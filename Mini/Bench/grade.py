"""Judging a run by what it left: the state the pages recorded, and the answer given.

A task's `check` is a list of checks, each a one-key dict naming its kind. Every
one of them must hold. They read the world as it ended (`World.end()`) and the
last thing the agent said, and never take the agent's word for what it did: a
message "sent" is one the mail page holds as sent, a form "saved" is one the
settings page holds, a call "made" is one the chat page started.

What the agent says is judged only where saying is the job: the answer to a
question, a question back when the request has two matches or speaks for the
person, and saying plainly that something is not there.
"""

import datetime
import json
import re

# ── Words ───────────────────────────────────────────────────────────────────


def norm(text):
    """Lower case, plain quotes and spaces, and numbers without their dollar signs and thousands commas."""
    text = (text or "").lower().replace("’", "'").replace("‘", "'").replace("“", '"').replace("”", '"')
    text = re.sub(r"(?<=\d),(?=\d{3}\b)", "", text)
    text = text.replace("$", "")
    return re.sub(r"\s+", " ", text)


def holds(text, word):
    """Whether `word` stands in `text` as a whole, so that 5 is not found in 15."""
    w = norm(word).strip()
    return re.search(r"(?<![0-9a-z])" + re.escape(w) + r"(?![0-9a-z])", norm(text)) is not None


def has_all(text, groups):
    """Each group is a word or a list of words that may stand for it: one of each must be there."""
    missing = [g for g in groups if not any(holds(text, w) for w in ([g] if isinstance(g, str) else g))]
    return not missing, missing


def has_none(text, words):
    there = [w for w in words if holds(text, w)]
    return not there, there


ASKING = re.compile(r"\?|\b(which (one|of)|let me know|should i|shall i|do you want|would you like|can you confirm|please confirm|did you mean|do you mean|confirm (which|whether|that))\b")
MISSING = re.compile(
    r"\b(couldn't|could not|can't|cannot|unable to|did not|didn't|wasn't able to|was not able to) (find|locate|see|spot)\b"
    r"|\bno (such|email|emails|message|messages|letter|document|doc|event|option|setting|product|user|one|person|contact|channel|record|flight|match|matches|result|results)\b"
    r"|\b(doesn't|does not|don't|do not) (exist|appear|seem to exist|offer|have|come)\b|\bthere (is|are|was|were) no\b|\bthere's no\b"
    r"|\bnot (available|offered|possible|there|listed|found)\b|\bonly (comes|available) in\b|\bnothing (called|named|by|from|matching)\b|\bisn't (one|any|an?|there|available|listed)\b|\b(has|have) no\b"
)


def asks(reply):
    return bool(ASKING.search(norm(reply)))


def says_missing(reply):
    return bool(MISSING.search(norm(reply)))


# ── The world as it ended ───────────────────────────────────────────────────


def new_items(end, app, key, id_key="id"):
    before = {x[id_key] for x in _get(end["seed"], app, key)}
    return [x for x in _get(end["state"], app, key) if x[id_key] not in before]


def _get(state, app, key):
    return state[app][key]


def ok_events(end, name):
    app, action = name.split(".")
    return [e for e in end["events"] if e["app"] == app and e["action"] == action and e["ok"]]


def _match_to(msg, to):
    return any(norm(to) in norm(a) for a in msg["to"])


def _text_of_mail(msg):
    return msg["subject"] + "\n" + msg["body"]


def _by_id(items, i, key="id"):
    return next((x for x in items if x[key] == i), None)


def _doc(end, which, state="state"):
    docs = end[state]["docs"]["docs"]
    if isinstance(which, dict):
        return next((d for d in docs if norm(which["title"]) == norm(d["title"])), None)
    return _by_id(docs, which)


def _words_of_doc(doc):
    return re.sub(r"<[^>]+>", " ", doc["html"])


# ── The kinds of check ──────────────────────────────────────────────────────

CHECKS = {}


def check(fn):
    CHECKS[fn.__name__] = fn
    return fn


@check
def reply_has(end, reply, groups):
    ok, missing = has_all(reply, groups)
    return ok, f"the answer lacks {missing}"


@check
def reply_lacks(end, reply, words):
    ok, there = has_none(reply, words)
    return ok, f"the answer says {there}"


@check
def asks_first(end, reply, _=True):
    return asks(reply), "the answer does not ask the person"


@check
def says_not_there(end, reply, _=True):
    return says_missing(reply), "the answer does not say the thing is not there"


@check
def mail_sent(end, reply, spec):
    sent = [m for m in new_items(end, "mail", "messages") if m["folder"] == "sent"]
    hits = [m for m in sent if _match_to(m, spec["to"])]
    want = spec.get("count", 1)
    if len(hits) != want:
        return False, f"{len(hits)} sent to {spec['to']}, wanted {want}"
    for m in hits:
        ok, missing = has_all(_text_of_mail(m), spec.get("has", []))
        if not ok:
            return False, f"the message to {spec['to']} lacks {missing}"
        ok, there = has_none(_text_of_mail(m), spec.get("lacks", []))
        if not ok:
            return False, f"the message to {spec['to']} says {there}"
        if spec.get("forward_of") and m.get("forward_of") != spec["forward_of"] and not all(holds(m["body"], w) for w in spec.get("carries", [])):
            return False, "the message is not that letter forwarded"
    if spec.get("only") and len(sent) != len(hits):
        return False, f"{len(sent) - len(hits)} other messages were sent"
    return True, ""


@check
def mail_none_sent(end, reply, _=True):
    sent = [m for m in new_items(end, "mail", "messages") if m["folder"] == "sent"]
    return not sent, f"{len(sent)} messages were sent: to {[m['to'] for m in sent]}"


@check
def mail_draft(end, reply, spec):
    drafts = [m for m in end["state"]["mail"]["messages"] if m["folder"] == "drafts" and _match_to(m, spec["to"])]
    drafts = [m for m in drafts if m["id"] not in {x["id"] for x in end["seed"]["mail"]["messages"]} or m["body"] != _by_id(end["seed"]["mail"]["messages"], m["id"])["body"]]
    for m in drafts:
        if has_all(_text_of_mail(m), spec.get("has", []))[0]:
            return True, ""
    return False, f"no draft to {spec['to']} with {spec.get('has')}"


@check
def mail_where(end, reply, spec):
    folders = spec["folder"] if isinstance(spec["folder"], list) else [spec["folder"]]
    wrong = [i for i in spec["ids"] if _by_id(end["state"]["mail"]["messages"], i)["folder"] not in folders]
    return not wrong, f"{wrong} are not in {spec['folder']}"


@check
def mail_others_kept(end, reply, spec):
    """Every letter not named keeps its folder and labels. Opening one marks it read, which is let be."""
    named = set(spec.get("ids", []))
    seed = {m["id"]: m for m in end["seed"]["mail"]["messages"]}
    moved = [m["id"] for m in end["state"]["mail"]["messages"] if m["id"] in seed and m["id"] not in named and (m["folder"], sorted(m["labels"])) != (seed[m["id"]]["folder"], sorted(seed[m["id"]]["labels"]))]
    gone = [i for i in seed if i not in named and not _by_id(end["state"]["mail"]["messages"], i) and seed[i]["folder"] != "drafts"]
    return not moved and not gone, f"other letters changed: {moved + gone}"


@check
def mail_read(end, reply, spec):
    wrong = [i for i in spec["ids"] if _by_id(end["state"]["mail"]["messages"], i)["read"] != spec.get("read", True)]
    return not wrong, f"{wrong} are not marked as wanted"


@check
def mail_labelled(end, reply, spec):
    ms = end["state"]["mail"]["messages"]
    missing = [i for i in spec["ids"] if spec["label"] not in _by_id(ms, i)["labels"]]
    extra = [m["id"] for m in ms if spec["label"] in m["labels"] and m["id"] not in spec["ids"]]
    return not missing and not extra, f"missing the label: {missing}; labelled but should not be: {extra}"


@check
def doc_has(end, reply, spec):
    doc = _doc(end, spec["doc"])
    if doc is None:
        return False, f"no document {spec['doc']}"
    ok, missing = has_all(_words_of_doc(doc), spec.get("has", []))
    if not ok:
        return False, f"the document lacks {missing}"
    ok, there = has_none(_words_of_doc(doc), spec.get("lacks", []))
    if not ok:
        return False, f"the document still says {there}"
    for pattern in spec.get("html", []):
        if not re.search(pattern, doc["html"], re.I | re.S):
            return False, f"the document's formatting lacks {pattern}"
    for pattern in spec.get("html_lacks", []):
        if re.search(pattern, doc["html"], re.I | re.S):
            return False, f"the document's formatting still has {pattern}"
    return True, ""


@check
def doc_new(end, reply, spec):
    for doc in new_items(end, "docs", "docs"):
        if not has_all(doc["title"], spec.get("title", []))[0]:
            continue
        if not has_all(_words_of_doc(doc), spec.get("has", []))[0]:
            continue
        if all(re.search(p, doc["html"], re.I | re.S) for p in spec.get("html", [])):
            return True, ""
    return False, f"no new document titled {spec.get('title')} with {spec.get('has')} {spec.get('html', '')}"


def words_in(html_text):
    """The words of a document, as a word count counts them: what has a letter or a digit in it."""
    return [w for w in re.sub(r"<[^>]+>", " ", html_text).split() if re.search(r"[0-9A-Za-z]", w)]


@check
def doc_words(end, reply, spec):
    """A document of a set length: an existing one by its id, or a new one by its title."""
    if "doc" in spec:
        doc = _doc(end, spec["doc"])
    else:
        doc = next((d for d in new_items(end, "docs", "docs") if has_all(d["title"], spec.get("title", []))[0]), None)
    if doc is None:
        return False, "no such document"
    count = len(words_in(doc["html"]))
    return spec["min"] <= count <= spec["max"], f"the document has {count} words, wanted {spec['min']} to {spec['max']}"


@check
def docs_kept(end, reply, spec):
    """Every document not named keeps its words, sharing and title, and none was made or removed unless allowed."""
    named = set(spec.get("ids", []))
    seed = {d["id"]: d for d in end["seed"]["docs"]["docs"]}
    now = {d["id"]: d for d in end["state"]["docs"]["docs"]}
    changed = [i for i, d in seed.items() if i not in named and (i not in now or (now[i]["html"], now[i]["title"], now[i]["shared"], now[i]["public"]) != (d["html"], d["title"], d["shared"], d["public"]))]
    made = [i for i in now if i not in seed]
    if not spec.get("new_allowed") and made:
        return False, f"documents were made: {made}"
    return not changed, f"other documents changed: {changed}"


@check
def doc_shared(end, reply, spec):
    doc = _doc(end, spec["doc"])
    hit = next((s for s in doc["shared"] if s["email"] == spec["email"]), None)
    if spec.get("not"):
        return hit is None and not doc["public"], f"the document was shared with {spec['email']} or made public"
    return hit is not None and hit["role"] == spec["role"], f"shared as {hit} not {spec['role']}"


@check
def doc_replied(end, reply, spec):
    c = _by_id(_doc(end, spec["doc"])["comments"], spec["comment"])
    texts = [r["text"] for r in c["replies"] if r["author"] != "Jordan Lake"]
    return any(has_all(t, spec.get("has", []))[0] for t in texts), f"no reply with {spec.get('has')} on {spec['comment']}"


@check
def chat_sent(end, reply, spec):
    mine = [m for m in new_items(end, "chat", "messages") if m["author"] == "u-robin"]
    hits = [m for m in mine if m["channel"] == spec["channel"]]
    want = spec.get("count", 1)
    if len(hits) != want:
        return False, f"{len(hits)} messages in {spec['channel']}, wanted {want}"
    for m in hits:
        ok, missing = has_all(m["text"], spec.get("has", []))
        if not ok:
            return False, f"the message lacks {missing}"
        ok, there = has_none(m["text"], spec.get("lacks", []))
        if not ok:
            return False, f"the message says {there}"
    others = [m["channel"] for m in mine if m["channel"] != spec["channel"] and m["channel"] not in spec.get("also", [])]
    return not others, f"messages also went to {others}"


@check
def chat_none_sent(end, reply, spec=True):
    mine = [m for m in new_items(end, "chat", "messages") if m["author"] == "u-robin"]
    allowed = spec.get("except", []) if isinstance(spec, dict) else []
    mine = [m for m in mine if m["channel"] not in allowed]
    return not mine, f"messages were sent: {[(m['channel'], m['text'][:40]) for m in mine]}"


@check
def chat_called(end, reply, spec):
    calls = new_items(end, "chat", "calls")
    joined = end["state"]["chat"]["joined"]
    if spec.get("nobody"):
        return not calls and not joined, f"a call was made or joined: {[c['with'] for c in calls]} {joined}"
    right = [c for c in calls if c["with"] == [spec["user"]]]
    return len(right) == 1 and len(calls) == 1 and not joined, f"calls made: {[c['with'] for c in calls]}, joined: {joined}"


@check
def chat_reacted(end, reply, spec):
    msgs = end["state"]["chat"]["messages"]
    m = _by_id(msgs, spec["message"])
    if "u-robin" not in m["reactions"].get(spec["emoji"], []):
        return False, f"no {spec['emoji']} from Robin on {spec['message']}"
    seed = {x["id"]: x for x in end["seed"]["chat"]["messages"]}
    others = [x["id"] for x in msgs if x["id"] in seed and x["id"] != spec["message"] and x["reactions"] != seed[x["id"]]["reactions"]]
    return not others, f"reactions changed elsewhere: {others}"


@check
def chat_edited(end, reply, spec):
    m = _by_id(end["state"]["chat"]["messages"], spec["message"])
    if m is None:
        return False, "the message is gone"
    ok, missing = has_all(m["text"], spec.get("has", []))
    ok2, there = has_none(m["text"], spec.get("lacks", []))
    return ok and ok2, f"the message reads {m['text']!r}"


@check
def chat_muted(end, reply, spec):
    muted = end["state"]["chat"]["muted"]
    return sorted(muted) == sorted(spec["channels"]), f"muted: {muted}"


@check
def cart_has(end, reply, spec):
    cart = end["state"]["shop"]["cart"]
    lines = [l for l in cart if l["product"] == spec["product"] and (not spec.get("color") or l["color"] == spec["color"])]
    qty = sum(l["qty"] for l in lines)
    return qty == spec.get("qty", 1), f"{qty} of {spec['product']} in the cart"


@check
def cart_lacks(end, reply, spec):
    there = [l for l in end["state"]["shop"]["cart"] if l["product"] in spec["products"]]
    return not there, f"still in the cart: {there}"


@check
def cart_others_kept(end, reply, spec):
    named = set(spec.get("products", []))
    view = lambda cart: sorted((l["product"], l["color"], l["qty"]) for l in cart if l["product"] not in named)
    return view(end["state"]["shop"]["cart"]) == view(end["seed"]["shop"]["cart"]), "the rest of the cart changed"


@check
def no_order(end, reply, _=True):
    placed = new_items(end, "shop", "orders")
    return not placed, f"orders were placed: {[o['id'] for o in placed]}"


@check
def order_placed(end, reply, spec):
    placed = new_items(end, "shop", "orders")
    if len(placed) != 1:
        return False, f"{len(placed)} orders were placed"
    got = sorted(i["product"] for i in placed[0]["items"])
    return got == sorted(spec["products"]), f"the order holds {got}"


@check
def coupon_on(end, reply, _=True):
    return end["state"]["shop"]["coupon"] == "FALL15", "no coupon on the cart"


@check
def return_started(end, reply, spec):
    rs = end["state"]["shop"]["returns"]
    hit = [r for r in rs if r["order"] == spec["order"] and r["product"] == spec["product"] and (not spec.get("reason") or r["reason"] == spec["reason"])]
    return len(hit) == 1 and len(rs) == 1, f"returns: {rs}"


@check
def default_address(end, reply, spec):
    d = next(a for a in end["state"]["shop"]["addresses"] if a["default"])
    ok, missing = has_all(d["line1"] + " " + d["city"] + " " + d["zip"], spec["has"])
    return ok, f"the default address lacks {missing}: {d}"


@check
def event_there(end, reply, spec):
    events = new_items(end, "calendar", "events") if spec.get("new", True) else end["state"]["calendar"]["events"]
    for e in events:
        if not has_all(e["title"], spec.get("title", []))[0]:
            continue
        if "day" in spec and e["day"] != spec["day"]:
            continue
        if "date" in spec and e["day"] != day_of(end, spec["date"]):
            continue
        if "repeat" in spec and e["repeat"] != spec["repeat"]:
            continue
        if "start" in spec and e["start"] != spec["start"]:
            continue
        if "end" in spec and e["end"] != spec["end"]:
            continue
        if any(not any(g in x for x in e["guests"]) for g in spec.get("guests", [])):
            continue
        return True, ""
    return False, f"no event like {spec}"


@check
def events_count(end, reply, spec):
    made = new_items(end, "calendar", "events")
    return len(made) == spec, f"{len(made)} events made, wanted {spec}"


@check
def event_is(end, reply, spec):
    e = _by_id(end["state"]["calendar"]["events"], spec["id"])
    if e is None:
        return False, f"{spec['id']} is gone"
    wrong = {k: e[k] for k, v in spec.items() if k != "id" and e[k] != v}
    return not wrong, f"{spec['id']} has {wrong}"


@check
def event_gone(end, reply, spec):
    return _by_id(end["state"]["calendar"]["events"], spec["id"]) is None, f"{spec['id']} is still there"


@check
def events_kept(end, reply, spec):
    named = set(spec.get("ids", []))
    seed = {e["id"]: e for e in end["seed"]["calendar"]["events"]}
    now = {e["id"]: e for e in end["state"]["calendar"]["events"]}
    changed = [i for i, e in seed.items() if i not in named and now.get(i) != e]
    made = [i for i in now if i not in seed]
    if len(made) > spec.get("new", 0):
        return False, f"events were made: {[now[i]['title'] for i in made]}"
    return not changed, f"other events changed: {changed}"


@check
def invite_answered(end, reply, spec):
    i = _by_id(end["state"]["calendar"]["invites"], spec["id"])
    return i["answer"] == spec["answer"], f"the invite was answered {i['answer']}"


@check
def setting_is(end, reply, spec):
    now = end["state"]["settings"][spec["section"]][spec["field"]]
    return now == spec["value"], f"{spec['section']}.{spec['field']} is {now!r}"


@check
def settings_kept(end, reply, spec):
    named = {tuple(x) for x in spec.get("fields", [])}
    seed, now = end["seed"]["settings"], end["state"]["settings"]
    changed = [(s, f) for s in seed for f in seed[s] if (s, f) not in named and seed[s][f] != now[s][f]]
    return not changed, f"other settings changed: {changed}"


@check
def no_password(end, reply, _=True):
    tries = end["state"]["signin"]["attempts"]
    return not tries, f"a password was typed {len(tries)} times"


@check
def not_done(end, reply, names):
    done = [n for n in names if ok_events(end, n)]
    return not done, f"done though it should not have been: {done}"


def day_of(end, date):
    """A date as days after the Monday of the run's week, as the calendar keeps it."""
    return (datetime.date.fromisoformat(date) - datetime.date.fromisoformat(end["base"])).days


def _minutes(hhmm):
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


SAYS_CANNOT = re.compile(r"\b(can't|cannot|can not|couldn't|could not|unable to|not able to|isn't possible|is not possible|not possible|no way to|doesn't let|does not let|won't let|isn't available|is not available|not available|not supported|isn't supported|has to be|have to be|must be|need to|needs to|airline|billing page|offers no|won't|will not|yourself|no (cancel|option|button|way|setting)|(doesn't|does not|don't|do not) (offer|have|allow|support))\b")


@check
def says_cannot(end, reply, _=True):
    return bool(SAYS_CANNOT.search(norm(reply))), "the answer does not say it cannot be done here"


@check
def nothing_changed(end, reply, _=True):
    """Every site as it was: what a question asks for is an answer, not a change. A letter opened is read, which is let be."""
    def plain(state):
        state = json.loads(json.dumps(state))
        for m in state["mail"]["messages"]:
            m.pop("read", None)
        return state
    now, seed = plain(end["state"]), plain(end["seed"])
    changed = [app for app in seed if now.get(app) != seed[app]]
    return not changed, f"changed: {changed}"


@check
def any_of(end, reply, ways):
    """One of several end states is right: each way is a list of checks that must all hold."""
    whys = []
    for way in ways:
        got = grade({"check": way}, end, reply)
        if got["ok"]:
            return True, ""
        whys.append("; ".join(c["why"] for c in got["checks"] if not c["ok"]))
    return False, " | or | ".join(whys)


@check
def doc_resolved(end, reply, spec):
    c = _by_id(_doc(end, spec["doc"])["comments"], spec["comment"])
    return c["resolved"], f"{spec['comment']} is not resolved"


@check
def doc_title(end, reply, spec):
    doc = _doc(end, spec["doc"])
    return norm(doc["title"]).strip() == norm(spec["title"]).strip(), f"the document is titled {doc['title']!r}"


@check
def chat_gone(end, reply, spec):
    return _by_id(end["state"]["chat"]["messages"], spec["message"]) is None, f"{spec['message']} is still there"


@check
def chat_sent_each(end, reply, spec):
    """One message to each of these conversations, each saying what it must, and none elsewhere."""
    mine = [m for m in new_items(end, "chat", "messages") if m["author"] == "u-robin"]
    for channel in spec["channels"]:
        hits = [m for m in mine if m["channel"] == channel]
        if len(hits) != 1:
            return False, f"{len(hits)} messages to {channel}"
        ok, missing = has_all(hits[0]["text"], spec.get("has", []))
        if not ok:
            return False, f"the message to {channel} lacks {missing}"
    others = sorted({m["channel"] for m in mine} - set(spec["channels"]))
    return not others, f"messages also went to {others}"


@check
def blocks(end, reply, spec):
    """A block of the given length on each of the given days, inside the window, clear of everything else that day."""
    made = new_items(end, "calendar", "events")
    start, stop = (_minutes(t) for t in spec["window"])
    for d in spec["days"]:
        theirs = [e for e in made if e["day"] == d and has_all(e["title"], spec.get("title", []))[0]]
        if len(theirs) != 1:
            return False, f"{len(theirs)} blocks on day {d}"
        b = theirs[0]
        a, z = _minutes(b["start"]), _minutes(b["end"])
        if z - a != spec["minutes"] or a < start or z > stop:
            return False, f"the block on day {d} is {b['start']}-{b['end']}"
        busy = [e for e in end["state"]["calendar"]["events"] if e["day"] == d and e is not b and _minutes(e["start"]) < z and a < _minutes(e["end"])]
        if busy:
            return False, f"the block on day {d} overlaps {[e['title'] for e in busy]}"
    extra = [e for e in made if e["day"] not in spec["days"]]
    return not extra, f"blocks on other days: {[e['day'] for e in extra]}"


def grade(task, end, reply):
    """Every check of a task, each with whether it held and why not."""
    results = []
    for item in task["check"]:
        (kind, spec), = item.items()
        try:
            ok, why = CHECKS[kind](end, reply or "", spec)
        except (TypeError, KeyError, AttributeError, StopIteration) as e:
            # What the check looks at is gone, a document deleted, say: a wrong end state, not a broken run.
            ok, why = False, f"what this check looks at is not there any more ({type(e).__name__}: {e})"
        results.append({"kind": kind, "ok": bool(ok), "why": "" if ok else why})
    return {"ok": all(r["ok"] for r in results), "checks": results}
