//! A reply's Markdown, read into its parts.
//!
//! The agent writes Markdown. The box shows it as what it is: a heading as a
//! heading, a list as a list, a command as something to copy. This module turns
//! the text into a tree of those parts. The page builds its elements from the
//! tree and gives them text; nothing a model writes ever reaches the page as
//! HTML, and raw HTML in a reply comes out as the plain words it is.
//!
//! The text is read again as it arrives, so half a reply has to read sensibly:
//! an open code fence is already a code block, and anything not understood stays
//! as plain text. Nothing vanishes.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag};
use serde_json::{json, Value};

/// The parts of a reply, in order.
///
/// Blocks: `paragraph` and `heading` (with `level`) hold `inlines`; `list` (with
/// `ordered` and `start`) holds `items`, each with `blocks` and, for a ticked
/// list, `checked`; `code` holds `text` and its `language`; `quote` holds
/// `blocks`; `table` holds `align`, `head` and `rows` of cells, each a list of
/// inlines; `rule` holds nothing.
///
/// Inlines: `text` and `code` hold `text`; `strong`, `emphasis` and `strike`
/// hold `inlines`; `link` holds a `url` and `inlines`; `break` ends a line.
pub fn tree(text: &str) -> Value {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut reader = Reader { events: Parser::new_ext(text, options), checked: None };
    Value::Array(reader.blocks())
}

struct Reader<'a> {
    events: Parser<'a>,
    /// The tick or empty box the list item being read began with.
    checked: Option<bool>,
}

impl Reader<'_> {
    /// Blocks up to the end of what holds them. That end is taken.
    fn blocks(&mut self) -> Vec<Value> {
        let mut blocks = Vec::new();
        // The words of a list item with no blank lines around it come without a paragraph.
        let mut loose: Vec<Value> = Vec::new();
        while let Some(event) = self.events.next() {
            let block = match event {
                Event::End(_) => break,
                Event::Start(Tag::Paragraph) => json!({ "kind": "paragraph", "inlines": self.inlines() }),
                Event::Start(Tag::Heading { level, .. }) => json!({ "kind": "heading", "level": level as u8, "inlines": self.inlines() }),
                Event::Start(Tag::BlockQuote(_)) => json!({ "kind": "quote", "blocks": self.blocks() }),
                Event::Start(Tag::CodeBlock(kind)) => {
                    let language = match kind {
                        CodeBlockKind::Fenced(info) => info.split_whitespace().next().map(str::to_string),
                        CodeBlockKind::Indented => None,
                    };
                    let text = self.words();
                    json!({ "kind": "code", "language": language, "text": text.strip_suffix('\n').unwrap_or(&text) })
                }
                Event::Start(Tag::HtmlBlock) => {
                    let text = self.words();
                    json!({ "kind": "paragraph", "inlines": [{ "kind": "text", "text": text.trim_end() }] })
                }
                Event::Start(Tag::List(start)) => self.list(start),
                Event::Start(Tag::Table(align)) => self.table(&align),
                Event::Rule => json!({ "kind": "rule" }),
                Event::TaskListMarker(checked) => {
                    self.checked = Some(checked);
                    continue;
                }
                // A block this box does not draw as its own kind: its parts are kept, in order.
                Event::Start(Tag::FootnoteDefinition(_) | Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition | Tag::Item) => {
                    settle(&mut loose, &mut blocks);
                    blocks.extend(self.blocks());
                    continue;
                }
                Event::Start(Tag::MetadataBlock(_)) => {
                    self.words();
                    continue;
                }
                other => {
                    if let Some(inline) = self.inline(other) {
                        add(&mut loose, inline);
                    }
                    continue;
                }
            };
            settle(&mut loose, &mut blocks);
            blocks.push(block);
        }
        settle(&mut loose, &mut blocks);
        blocks
    }

    /// Inlines up to the end of what holds them. That end is taken.
    fn inlines(&mut self) -> Vec<Value> {
        let mut inlines = Vec::new();
        while let Some(event) = self.events.next() {
            if matches!(event, Event::End(_)) {
                break;
            }
            if let Some(inline) = self.inline(event) {
                add(&mut inlines, inline);
            }
        }
        inlines
    }

    fn inline(&mut self, event: Event) -> Option<Value> {
        Some(match event {
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                json!({ "kind": "text", "text": text.as_ref() })
            }
            Event::FootnoteReference(name) => json!({ "kind": "text", "text": format!("[{name}]") }),
            Event::Code(text) => json!({ "kind": "code", "text": text.as_ref() }),
            // A line end inside a paragraph is a space; two spaces before it, or a backslash, is a new line.
            Event::SoftBreak => json!({ "kind": "text", "text": " " }),
            Event::HardBreak => json!({ "kind": "break" }),
            Event::Start(Tag::Strong) => json!({ "kind": "strong", "inlines": self.inlines() }),
            Event::Start(Tag::Emphasis) => json!({ "kind": "emphasis", "inlines": self.inlines() }),
            Event::Start(Tag::Strikethrough) => json!({ "kind": "strike", "inlines": self.inlines() }),
            Event::Start(Tag::Link { dest_url, .. }) => json!({ "kind": "link", "url": dest_url.as_ref(), "inlines": self.inlines() }),
            // A picture is not drawn; the words that stand for it are kept.
            Event::Start(Tag::Image { .. }) => {
                let words = self.words();
                json!({ "kind": "text", "text": words })
            }
            Event::Start(_) => {
                let words = self.words();
                json!({ "kind": "text", "text": words })
            }
            Event::TaskListMarker(checked) => {
                self.checked = Some(checked);
                return None;
            }
            Event::Rule | Event::End(_) => return None,
        })
    }

    /// Every word up to the end of what holds them, as plain text. That end is taken.
    fn words(&mut self) -> String {
        let mut words = String::new();
        let mut depth = 0usize;
        for event in self.events.by_ref() {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) if depth == 0 => break,
                Event::End(_) => depth -= 1,
                Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                    words.push_str(&text)
                }
                Event::SoftBreak | Event::HardBreak => words.push(' '),
                _ => {}
            }
        }
        words
    }

    fn list(&mut self, start: Option<u64>) -> Value {
        let mut items = Vec::new();
        while let Some(event) = self.events.next() {
            match event {
                Event::Start(Tag::Item) => {
                    // A list inside an item must not lose the tick of the item it is in.
                    let outer = self.checked.take();
                    let blocks = self.blocks();
                    items.push(json!({ "checked": self.checked.take(), "blocks": blocks }));
                    self.checked = outer;
                }
                Event::End(_) => break,
                _ => {}
            }
        }
        json!({ "kind": "list", "ordered": start.is_some(), "start": start.unwrap_or(1), "items": items })
    }

    fn table(&mut self, align: &[Alignment]) -> Value {
        let align: Vec<Value> = align
            .iter()
            .map(|side| match side {
                Alignment::Left => json!("left"),
                Alignment::Center => json!("center"),
                Alignment::Right => json!("right"),
                Alignment::None => Value::Null,
            })
            .collect();
        let mut head = Vec::new();
        let mut rows = Vec::new();
        while let Some(event) = self.events.next() {
            match event {
                Event::Start(Tag::TableHead) => head = self.cells(),
                Event::Start(Tag::TableRow) => rows.push(Value::Array(self.cells())),
                Event::End(_) => break,
                _ => {}
            }
        }
        json!({ "kind": "table", "align": align, "head": head, "rows": rows })
    }

    /// The cells of one row of a table. The row's end is taken.
    fn cells(&mut self) -> Vec<Value> {
        let mut cells = Vec::new();
        while let Some(event) = self.events.next() {
            match event {
                Event::Start(Tag::TableCell) => cells.push(Value::Array(self.inlines())),
                Event::End(_) => break,
                _ => {}
            }
        }
        cells
    }
}

/// Add an inline, joining text to the text before it.
fn add(inlines: &mut Vec<Value>, inline: Value) {
    if inline["kind"] == "text" {
        if let Some(last) = inlines.last_mut().filter(|last| last["kind"] == "text") {
            let joined = format!("{}{}", last["text"].as_str().unwrap_or_default(), inline["text"].as_str().unwrap_or_default());
            last["text"] = Value::String(joined);
            return;
        }
    }
    inlines.push(inline);
}

/// Words that came without a paragraph become one.
fn settle(loose: &mut Vec<Value>, blocks: &mut Vec<Value>) {
    if !loose.is_empty() {
        blocks.push(json!({ "kind": "paragraph", "inlines": std::mem::take(loose) }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(words: &str) -> Value {
        json!({ "kind": "text", "text": words })
    }

    fn paragraph(inlines: Value) -> Value {
        json!({ "kind": "paragraph", "inlines": inlines })
    }

    #[test]
    fn a_paragraph_keeps_its_emphasis_code_and_links() {
        let read = tree("The lockfile is **out of date**. Run `cargo update`, *then* see [the log](https://example.com/log) ~~later~~.");
        assert_eq!(
            read,
            json!([paragraph(json!([
                text("The lockfile is "),
                { "kind": "strong", "inlines": [text("out of date")] },
                text(". Run "),
                { "kind": "code", "text": "cargo update" },
                text(", "),
                { "kind": "emphasis", "inlines": [text("then")] },
                text(" see "),
                { "kind": "link", "url": "https://example.com/log", "inlines": [text("the log")] },
                text(" "),
                { "kind": "strike", "inlines": [text("later")] },
                text("."),
            ]))])
        );
    }

    #[test]
    fn a_line_end_in_a_paragraph_is_a_space_and_a_hard_one_is_a_break() {
        assert_eq!(tree("one\ntwo"), json!([paragraph(json!([text("one two")]))]));
        assert_eq!(tree("one  \ntwo"), json!([paragraph(json!([text("one"), { "kind": "break" }, text("two")]))]));
    }

    #[test]
    fn headings_carry_their_level() {
        assert_eq!(
            tree("## Why the build fails\n\n#### Detail"),
            json!([
                { "kind": "heading", "level": 2, "inlines": [text("Why the build fails")] },
                { "kind": "heading", "level": 4, "inlines": [text("Detail")] },
            ])
        );
    }

    #[test]
    fn lists_are_numbered_or_not_and_may_hold_lists() {
        let read = tree("3. Run `cargo update`\n4. Commit\n   - the lockfile\n   - nothing else\n");
        assert_eq!(read[0]["kind"], "list");
        assert_eq!(read[0]["ordered"], true);
        assert_eq!(read[0]["start"], 3);
        let items = read[0]["items"].as_array().unwrap();
        assert_eq!(items[0]["blocks"], json!([paragraph(json!([text("Run "), { "kind": "code", "text": "cargo update" }]))]));
        assert_eq!(items[1]["blocks"][0], paragraph(json!([text("Commit")])));
        let inner = &items[1]["blocks"][1];
        assert_eq!(inner["ordered"], false);
        assert_eq!(inner["items"][1]["blocks"], json!([paragraph(json!([text("nothing else")]))]));
    }

    #[test]
    fn a_ticked_list_says_which_items_are_done() {
        let read = tree("- [x] build\n- [ ] sign\n- plain");
        let checked: Vec<&Value> = read[0]["items"].as_array().unwrap().iter().map(|item| &item["checked"]).collect();
        assert_eq!(checked, [&json!(true), &json!(false), &Value::Null]);
        assert_eq!(read[0]["items"][0]["blocks"], json!([paragraph(json!([text("build")]))]));
    }

    #[test]
    fn a_code_block_keeps_its_lines_as_they_are() {
        let read = tree("```sh\ncargo update -p x\ngit commit -am \"y\"\n```\n\n    indented <b>too</b>\n");
        assert_eq!(
            read,
            json!([
                { "kind": "code", "language": "sh", "text": "cargo update -p x\ngit commit -am \"y\"" },
                { "kind": "code", "language": null, "text": "indented <b>too</b>" },
            ])
        );
        let diagram = "┌──┐   ┌──┐\n│a │──▶│b │\n└──┘   └──┘";
        assert_eq!(tree(&format!("```\n{diagram}\n```"))[0]["text"], diagram);
    }

    #[test]
    fn a_table_has_a_head_rows_and_sides() {
        let read = tree("| Crate | Locked | Wanted |\n|:--|--:|---|\n| `tauri` | 2.3.1 | **2.4.0** |\n| acp | 3.1.0 | 3.2.0 |\n");
        assert_eq!(read[0]["kind"], "table");
        assert_eq!(read[0]["align"], json!(["left", "right", null]));
        assert_eq!(read[0]["head"], json!([[text("Crate")], [text("Locked")], [text("Wanted")]]));
        assert_eq!(read[0]["rows"][0], json!([[{ "kind": "code", "text": "tauri" }], [text("2.3.1")], [{ "kind": "strong", "inlines": [text("2.4.0")] }]]));
        assert_eq!(read[0]["rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_quote_and_a_rule_are_their_own_parts() {
        assert_eq!(
            tree("> Nothing else changes.\n\n---\n"),
            json!([{ "kind": "quote", "blocks": [paragraph(json!([text("Nothing else changes.")]))] }, { "kind": "rule" }])
        );
    }

    #[test]
    fn html_a_model_writes_stays_plain_words() {
        let read = tree("<script>invoke('quit')</script>\n\nand <img src=x onerror=alert(1)> inline");
        assert_eq!(read[0], paragraph(json!([text("<script>invoke('quit')</script>")])));
        assert_eq!(read[1], paragraph(json!([text("and <img src=x onerror=alert(1)> inline")])));
        // A picture is not drawn: the words that stand for it are kept.
        assert_eq!(tree("![a chart](https://example.com/c.png)"), json!([paragraph(json!([text("a chart")]))]));
    }

    #[test]
    fn half_a_reply_already_reads_sensibly() {
        // A code fence that is still open is a code block.
        assert_eq!(tree("Run this:\n\n```sh\ncargo upd")[1], json!({ "kind": "code", "language": "sh", "text": "cargo upd" }));
        // Emphasis that is still open shows as typed, and nothing is lost.
        assert_eq!(tree("The lockfile is **out of"), json!([paragraph(json!([text("The lockfile is **out of")]))]));
        // A table grows row by row; before its second line arrives it is still a paragraph.
        assert_eq!(tree("| Crate | Locked |")[0]["kind"], "paragraph");
        let growing = tree("| Crate | Locked |\n|---|---|\n| tauri | 2.3");
        assert_eq!(growing[0]["kind"], "table");
        assert_eq!(growing[0]["rows"][0][0], json!([text("tauri")]));
        // Nothing at all is nothing.
        assert_eq!(tree(""), json!([]));
    }
}
