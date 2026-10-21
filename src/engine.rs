use crate::decide::{autocorrect, force_convert};
use crate::dict::HasWords;
use crate::keymap::{Script, Term};

const MAX_WORD: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    /// How many characters, already on screen, to delete before inserting.
    pub delete: usize,
    pub insert: String,
    pub switch_to: Script,
    /// Terminator to type after the inserted text, already in `switch_to`.
    /// A mark is the other layout's character of the same key (`&` → `?`).
    /// `None` for a manual conversion that had no terminator.
    pub term: Option<Term>,
}

struct Saved {
    original: String,
    inserted: String,
    switched_to: Script,
    /// Mark as it was typed, in `term_script`. Space, Enter and Tab ignore it.
    term: Option<Term>,
    term_script: Script,
}

pub struct Engine {
    buf: Vec<char>,
    overflow: bool,
    /// The word currently on screen after the last Pause or autocorrect,
    /// paired with its other-layout form. Each Pause swaps them.
    /// Cleared on pause, navigation, password fields, and the next word.
    last: Option<Saved>,
}

impl Engine {
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            overflow: false,
            last: None,
        }
    }

    pub fn forget(&mut self) {
        self.buf.clear();
        self.overflow = false;
        self.last = None;
    }

    pub fn push_glyph(&mut self, ch: char) {
        if self.overflow {
            return;
        }
        if self.buf.len() >= MAX_WORD {
            self.buf.clear();
            self.overflow = true;
            return;
        }
        self.buf.push(ch);
    }

    pub fn backspace(&mut self) {
        if self.overflow {
            return;
        }
        if self.buf.pop().is_none() {
            // The user is deleting already committed text. Undo would target
            // the wrong place, so drop the saved replacement.
            self.last = None;
        }
    }

    pub fn cancel(&mut self) {
        self.buf.clear();
        self.overflow = false;
        self.last = None;
    }

    pub fn end_word(&mut self, term: Term, dict: &impl HasWords) -> Option<Rewrite> {
        let original: String = self.buf.iter().collect();
        let overflow = self.overflow;
        self.buf.clear();
        self.overflow = false;
        self.last = None;
        if overflow || original.is_empty() {
            return None;
        }
        let change = autocorrect(&original, dict)?;
        let typed_in = change.switch_to.other();
        let rewrite = Rewrite {
            delete: original.chars().count(),
            insert: change.text.clone(),
            switch_to: change.switch_to,
            term: Some(present_term(term, typed_in, change.switch_to)),
        };
        self.last = Some(Saved {
            original,
            inserted: change.text,
            switched_to: change.switch_to,
            term: Some(term),
            term_script: typed_in,
        });
        Some(rewrite)
    }

    /// Pause: convert the unfinished word, or swap the last replacement
    /// again when the buffer is empty. Repeated presses keep swapping.
    pub fn force(&mut self) -> Option<Rewrite> {
        if self.buf.is_empty() || self.overflow {
            self.overflow = false;
            self.buf.clear();
            return self.undo();
        }
        let original: String = self.buf.iter().collect();
        self.buf.clear();
        let change = force_convert(&original)?;
        let rewrite = Rewrite {
            delete: original.chars().count(),
            insert: change.text.clone(),
            switch_to: change.switch_to,
            term: None,
        };
        self.last = Some(Saved {
            original,
            inserted: change.text,
            switched_to: change.switch_to,
            term: None,
            term_script: change.switch_to,
        });
        Some(rewrite)
    }

    fn undo(&mut self) -> Option<Rewrite> {
        let saved = self.last.take()?;
        let term_len = usize::from(saved.term.is_some());
        let switch_to = saved.switched_to.other();
        let rewrite = Rewrite {
            delete: saved.inserted.chars().count() + term_len,
            insert: saved.original.clone(),
            switch_to,
            term: saved
                .term
                .map(|term| present_term(term, saved.term_script, switch_to)),
        };
        self.last = Some(Saved {
            original: saved.inserted,
            inserted: saved.original,
            switched_to: rewrite.switch_to,
            term: saved.term,
            term_script: saved.term_script,
        });
        Some(rewrite)
    }
}

fn present_term(term: Term, typed_in: Script, switch_to: Script) -> Term {
    match term {
        Term::Mark(ch) if typed_in != switch_to => {
            Term::Mark(crate::keymap::other_mark(typed_in, ch).unwrap_or(ch))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::MapDict;

    fn dict() -> MapDict {
        MapDict::new(&["hello"], &["привет"])
    }

    fn type_word(engine: &mut Engine, word: &str) {
        for ch in word.chars() {
            engine.push_glyph(ch);
        }
    }

    #[test]
    fn space_rewrites_and_pause_undoes() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "ghbdtn");
        let rewrite = engine.end_word(Term::Space, &dict).unwrap();
        assert_eq!(rewrite.delete, 6);
        assert_eq!(rewrite.insert, "привет");
        assert_eq!(rewrite.switch_to, Script::Ru);
        assert_eq!(rewrite.term, Some(Term::Space));

        let undo = engine.force().unwrap();
        assert_eq!(undo.delete, 7);
        assert_eq!(undo.insert, "ghbdtn");
        assert_eq!(undo.switch_to, Script::En);
        assert_eq!(undo.term, Some(Term::Space));

        let again = engine.force().unwrap();
        assert_eq!(again.delete, 7);
        assert_eq!(again.insert, "привет");
        assert_eq!(again.switch_to, Script::Ru);
        assert_eq!(again.term, Some(Term::Space));

        let back = engine.force().unwrap();
        assert_eq!(back.insert, "ghbdtn");
        assert_eq!(back.switch_to, Script::En);
    }

    #[test]
    fn mark_follows_the_layout() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "руддщ");
        let rewrite = engine.end_word(Term::Mark(','), &dict).unwrap();
        assert_eq!(rewrite.insert, "hello");
        assert_eq!(rewrite.switch_to, Script::En);
        assert_eq!(rewrite.term, Some(Term::Mark('?')));

        let undo = engine.force().unwrap();
        assert_eq!(undo.insert, "руддщ");
        assert_eq!(undo.switch_to, Script::Ru);
        assert_eq!(undo.term, Some(Term::Mark(',')));

        let again = engine.force().unwrap();
        assert_eq!(again.insert, "hello");
        assert_eq!(again.term, Some(Term::Mark('?')));

        let mut engine = Engine::new();
        type_word(&mut engine, "ghbdtn");
        let rewrite = engine.end_word(Term::Mark('&'), &dict).unwrap();
        assert_eq!(rewrite.insert, "привет");
        assert_eq!(rewrite.switch_to, Script::Ru);
        assert_eq!(rewrite.term, Some(Term::Mark('?')));
    }

    #[test]
    fn pause_toggles_the_same_word() {
        let mut engine = Engine::new();
        type_word(&mut engine, "ghbdtn");
        let first = engine.force().unwrap();
        assert_eq!(first.insert, "привет");
        assert_eq!(first.switch_to, Script::Ru);
        assert_eq!(first.term, None);

        let second = engine.force().unwrap();
        assert_eq!(second.delete, "привет".chars().count());
        assert_eq!(second.insert, "ghbdtn");
        assert_eq!(second.switch_to, Script::En);

        let third = engine.force().unwrap();
        assert_eq!(third.insert, "привет");
        assert_eq!(third.switch_to, Script::Ru);
    }

    #[test]
    fn real_word_passes_through() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello");
        assert!(engine.end_word(Term::Space, &dict).is_none());
    }

    #[test]
    fn backspace_edits_the_buffer() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "ghbdtn");
        engine.backspace();
        engine.push_glyph('n');
        assert!(engine.end_word(Term::Enter, &dict).unwrap().insert == "привет");
    }

    #[test]
    fn navigation_drops_undo() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "ghbdtn");
        engine.end_word(Term::Space, &dict).unwrap();
        engine.cancel();
        assert!(engine.force().is_none());
    }
}
