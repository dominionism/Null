//! Verifying an answer: a second model looks at it before it is relied on.
//!
//! A model that works fast can be wrong and give no sign of it. It takes a
//! shortcut, or carries a number over from an earlier step, and says what it
//! found as plainly as if it had done what was asked. So the person can name a
//! second model (`/verify`). From then on, when a reply that took steps has
//! ended well, Null moves the conversation to that model, asks it to check the
//! answer against what was seen and done, and puts the conversation back. It
//! is the same conversation, so the second model has before it everything the
//! first saw. The answer is not held for this: it is on the page before the
//! second model is asked.
//!
//! While a backup is answering in place of the person's model, the
//! conversation is not moved: the model that answered goes over its own
//! answer. A move would have to be undone, and there is nowhere good to undo
//! it to (`harness.rs`, `on_a_backup`).
//!
//! The harness has a checker of its own, a second model that reads along.
//! Through the protocol it keeps what that model finds and never passes it on
//! (asked of Oh-my-pi 18.8.7 with the stand-in, and seen with a real model), so
//! a mistake it found reached nobody. That is why Null does this itself.
//!
//! It is the one message Null sends that nobody typed. It is sent on the
//! person's standing word, the box says each time that it is sent, and it is
//! the words of `ASK` and no others.

use tauri::AppHandle;

use crate::log::log;
use crate::settings;

/// What the second model is asked. It is to judge the answer by what the
/// conversation holds, not by what it would have answered itself, and to
/// change nothing on the Mac while it does.
pub const ASK: &str = "Before the answer above is relied on, verify it. Go back over what was seen and done in this conversation and compare the last answer with it, fact by fact: every number, every name, every choice made, and every claim about what something offers or does not offer. You may look again at a page or a window to settle a doubt, but change nothing and take no new action. If everything in the answer is supported by what was seen, reply with the one word: stands. If something in it is wrong, is not supported by what was seen, or was chosen without the person asking for it and not said, name each in one line, and then give the corrected answer.";

/// What is known of a reply that has just ended.
pub struct Reply {
    /// It ended as an answer: not with a refusal or an error, and not stopped.
    pub ended_well: bool,
    /// How many steps it took.
    pub steps: u32,
    /// It was itself the second model's reply.
    pub verification: bool,
    /// Its conversation works in the box's own folder.
    pub own_folder: bool,
    /// The model named to verify is one the harness offers.
    pub offered: bool,
}

/// Whether the second model is asked about this reply. An answer that took no
/// step saw nothing to be checked against. The second model's own reply is
/// not verified in its turn. And work in a folder of the person's choosing is
/// left alone: a long job in a project is not an errand with one answer.
pub fn due(reply: &Reply) -> bool {
    reply.ended_well && reply.steps > 0 && !reply.verification && reply.own_folder && reply.offered
}

/// The model that verifies answers, when the person named one.
#[tauri::command]
pub fn verifier(app: AppHandle) -> Option<String> {
    settings::get(&app).verifier
}

/// Name the model that verifies answers, or none to have no answer verified.
#[tauri::command]
pub fn set_verifier(app: AppHandle, id: Option<String>) -> Option<String> {
    let id = id.filter(|id| !id.trim().is_empty());
    match &id {
        Some(id) => log!("answers are verified by {id} from now on"),
        None => log!("answers are not verified from now on"),
    }
    settings::update(&app, |settings| settings.verifier = id.clone());
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply() -> Reply {
        Reply { ended_well: true, steps: 3, verification: false, own_folder: true, offered: true }
    }

    #[test]
    fn an_answer_that_took_steps_and_ended_well_is_verified() {
        assert!(due(&reply()));
    }

    #[test]
    fn an_answer_that_took_no_step_saw_nothing_to_be_checked_against() {
        assert!(!due(&Reply { steps: 0, ..reply() }));
    }

    #[test]
    fn a_reply_that_did_not_end_as_an_answer_is_not_verified() {
        assert!(!due(&Reply { ended_well: false, ..reply() }), "a refusal, an error, or a reply the person stopped");
    }

    #[test]
    fn the_second_model_s_own_reply_is_not_verified_in_its_turn() {
        assert!(!due(&Reply { verification: true, ..reply() }));
    }

    #[test]
    fn work_in_a_folder_of_the_person_s_choosing_is_left_alone() {
        assert!(!due(&Reply { own_folder: false, ..reply() }));
    }

    #[test]
    fn a_model_the_harness_no_longer_offers_verifies_nothing() {
        assert!(!due(&Reply { offered: false, ..reply() }));
    }

    #[test]
    fn what_is_asked_changes_nothing_and_has_one_word_for_an_answer_that_stands() {
        assert!(ASK.contains("change nothing") && ASK.contains("the one word: stands"));
    }
}
