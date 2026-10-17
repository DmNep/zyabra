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
    /// Replay this terminator after the inserted text, while the old layout
    /// is still active. `None` for a manual conversion that had no terminator.
    pub term: Option<Term>,
}

struct Saved {
    original: String,
    inserted: String,
    switched_to: Script,
    term: Option<Term>,
}

pub struct Engine {
    buf: Vec<char>,
    overflow: bool,
    /// The single previous replacement, so Pause can undo it.
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
        let rewrite = Rewrite {
            delete: original.chars().count(),
            insert: change.text.clone(),
            switch_to: change.switch_to,
            term: Some(term),
        };
        self.last = Some(Saved {
            original,
            inserted: change.text,
            switched_to: change.switch_to,
            term: Some(term),
        });
        Some(rewrite)
    }

    /// Pause: convert the unfinished word, or undo the last replacement
    /// when the buffer is empty.
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
        });
        Some(rewrite)
    }

    fn undo(&mut self) -> Option<Rewrite> {
        let saved = self.last.take()?;
        let term_len = usize::from(saved.term.is_some());
        Some(Rewrite {
            delete: saved.inserted.chars().count() + term_len,
            insert: saved.original,
            switch_to: saved.switched_to.other(),
            term: saved.term,
        })
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
        assert!(engine.force().is_none());
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
