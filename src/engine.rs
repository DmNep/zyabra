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
    /// Spaces typed after `term`. A sentence ends as `слово.` and then a
    /// space; Pause has to take that space too, or it deletes the period.
    pub spaces: u8,
    /// Caps Lock to leave behind. `Some(false)` releases it after `пРИВЕТ` →
    /// `Привет`. `Some(true)` puts it back when that word is restored.
    /// `None` leaves the key alone.
    pub caps: Option<bool>,
}

struct Saved {
    original: String,
    original_script: Script,
    inserted: String,
    inserted_script: Script,
    /// Mark as it was typed, in `term_script`. Space, Enter and Tab ignore it.
    term: Option<Term>,
    term_script: Script,
    /// Keep `term` as the character on screen. A sentence period stays a
    /// period; it is not the other layout's key.
    literal_term: bool,
    /// Spaces pressed after the word was already committed (`. `).
    extra_spaces: u8,
    original_caps: Option<bool>,
    inserted_caps: Option<bool>,
}

pub struct Engine {
    buf: Vec<char>,
    overflow: bool,
    /// The word currently on screen after the last Pause, autocorrect, or a
    /// committed word that was left as typed. Each Pause swaps the pair.
    /// Cleared on pause, navigation, password fields, and the next word.
    /// A space after `.` or `,` stays attached, so Pause can still reach it.
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
        if overflow {
            self.last = None;
            return None;
        }
        if original.is_empty() {
            if term == Term::Space {
                if let Some(saved) = self.last.as_mut() {
                    saved.extra_spaces = saved.extra_spaces.saturating_add(1);
                }
            } else {
                self.last = None;
            }
            return None;
        }
        self.last = None;
        let Some(change) = autocorrect(&original, dict) else {
            self.remember_literal(&original, term);
            return None;
        };
        let caps = change.release_caps.then_some(false);
        let rewrite = Rewrite {
            delete: original.chars().count(),
            insert: change.text.clone(),
            switch_to: change.switch_to,
            term: Some(present_term(term, change.typed_in, change.switch_to)),
            spaces: 0,
            caps,
        };
        self.last = Some(Saved {
            original,
            original_script: change.typed_in,
            inserted: change.text,
            inserted_script: change.switch_to,
            term: Some(term),
            term_script: change.typed_in,
            literal_term: false,
            extra_spaces: 0,
            original_caps: change.release_caps.then_some(true),
            inserted_caps: caps,
        });
        Some(rewrite)
    }

    /// The word stayed on screen. Pause can still transpose it, including the
    /// space, period, or comma that finished it.
    fn remember_literal(&mut self, shown: &str, term: Term) {
        let Some(change) = force_convert(shown) else {
            return;
        };
        self.last = Some(Saved {
            original: change.text,
            original_script: change.switch_to,
            inserted: shown.to_string(),
            inserted_script: change.typed_in,
            term: Some(term),
            term_script: change.typed_in,
            literal_term: true,
            extra_spaces: 0,
            original_caps: None,
            inserted_caps: None,
        });
    }

    /// Pause: convert the unfinished word, or swap the last replacement
    /// again when the buffer is empty. A word already closed by space,
    /// period, or comma is still that last replacement. Repeated presses
    /// keep swapping.
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
            spaces: 0,
            caps: None,
        };
        self.last = Some(Saved {
            original,
            original_script: change.typed_in,
            inserted: change.text,
            inserted_script: change.switch_to,
            term: None,
            term_script: change.typed_in,
            literal_term: false,
            extra_spaces: 0,
            original_caps: None,
            inserted_caps: None,
        });
        Some(rewrite)
    }

    fn undo(&mut self) -> Option<Rewrite> {
        let saved = self.last.take()?;
        let term_len = usize::from(saved.term.is_some());
        let switch_to = saved.original_script;
        let rewrite = Rewrite {
            delete: saved.inserted.chars().count() + term_len + usize::from(saved.extra_spaces),
            insert: saved.original.clone(),
            switch_to,
            term: saved.term.map(|term| {
                if saved.literal_term {
                    term
                } else {
                    present_term(term, saved.term_script, switch_to)
                }
            }),
            spaces: saved.extra_spaces,
            caps: saved.original_caps,
        };
        self.last = Some(Saved {
            original: saved.inserted,
            original_script: saved.inserted_script,
            inserted: saved.original,
            inserted_script: saved.original_script,
            term: saved.term,
            term_script: saved.term_script,
            literal_term: saved.literal_term,
            extra_spaces: saved.extra_spaces,
            original_caps: saved.inserted_caps,
            inserted_caps: saved.original_caps,
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
        assert_eq!(first.caps, None);

        let second = engine.force().unwrap();
        assert_eq!(second.delete, "привет".chars().count());
        assert_eq!(second.insert, "ghbdtn");
        assert_eq!(second.switch_to, Script::En);

        let third = engine.force().unwrap();
        assert_eq!(third.insert, "привет");
        assert_eq!(third.switch_to, Script::Ru);
    }

    #[test]
    fn caps_lock_case_stays_on_the_same_layout() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "пРИВЕТ");
        let rewrite = engine.end_word(Term::Space, &dict).unwrap();
        assert_eq!(rewrite.delete, 6);
        assert_eq!(rewrite.insert, "Привет");
        assert_eq!(rewrite.switch_to, Script::Ru);
        assert_eq!(rewrite.term, Some(Term::Space));
        assert_eq!(rewrite.caps, Some(false));

        let undo = engine.force().unwrap();
        assert_eq!(undo.insert, "пРИВЕТ");
        assert_eq!(undo.switch_to, Script::Ru);
        assert_eq!(undo.term, Some(Term::Space));
        assert_eq!(undo.caps, Some(true));

        let again = engine.force().unwrap();
        assert_eq!(again.insert, "Привет");
        assert_eq!(again.switch_to, Script::Ru);
        assert_eq!(again.caps, Some(false));

        let mut engine = Engine::new();
        type_word(&mut engine, "hELLO");
        let rewrite = engine.end_word(Term::Mark(','), &dict).unwrap();
        assert_eq!(rewrite.insert, "Hello");
        assert_eq!(rewrite.switch_to, Script::En);
        assert_eq!(rewrite.term, Some(Term::Mark(',')));
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

    #[test]
    fn space_still_flips_a_word_that_was_left_alone() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello");
        assert!(engine.end_word(Term::Space, &dict).is_none());

        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 6);
        assert_eq!(flip.insert, "руддщ");
        assert_eq!(flip.switch_to, Script::Ru);
        assert_eq!(flip.term, Some(Term::Space));
        assert_eq!(flip.spaces, 0);

        let back = engine.force().unwrap();
        assert_eq!(back.delete, 6);
        assert_eq!(back.insert, "hello");
        assert_eq!(back.switch_to, Script::En);
        assert_eq!(back.term, Some(Term::Space));
    }

    #[test]
    fn period_still_flips_the_finished_word() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello");
        assert!(engine.end_word(Term::Mark('.'), &dict).is_none());

        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 6);
        assert_eq!(flip.insert, "руддщ");
        assert_eq!(flip.switch_to, Script::Ru);
        assert_eq!(flip.term, Some(Term::Mark('.')));
        assert_eq!(flip.spaces, 0);

        let back = engine.force().unwrap();
        assert_eq!(back.insert, "hello");
        assert_eq!(back.term, Some(Term::Mark('.')));
        assert_eq!(back.spaces, 0);
    }

    #[test]
    fn period_and_space_stay_with_the_word() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello");
        assert!(engine.end_word(Term::Mark('.'), &dict).is_none());
        assert!(engine.end_word(Term::Space, &dict).is_none());

        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 7);
        assert_eq!(flip.insert, "руддщ");
        assert_eq!(flip.switch_to, Script::Ru);
        assert_eq!(flip.term, Some(Term::Mark('.')));
        assert_eq!(flip.spaces, 1);

        let back = engine.force().unwrap();
        assert_eq!(back.insert, "hello");
        assert_eq!(back.switch_to, Script::En);
        assert_eq!(back.term, Some(Term::Mark('.')));
        assert_eq!(back.spaces, 1);
    }

    #[test]
    fn comma_still_flips_the_finished_word() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello");
        assert!(engine.end_word(Term::Mark(','), &dict).is_none());

        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 6);
        assert_eq!(flip.insert, "руддщ");
        assert_eq!(flip.term, Some(Term::Mark(',')));
        assert_eq!(flip.spaces, 0);

        let mut engine = Engine::new();
        type_word(&mut engine, "ghbdtn");
        let rewrite = engine.end_word(Term::Mark(','), &dict).unwrap();
        assert_eq!(rewrite.insert, "привет");
        assert!(engine.end_word(Term::Space, &dict).is_none());
        let undo = engine.force().unwrap();
        assert_eq!(undo.delete, 8);
        assert_eq!(undo.insert, "ghbdtn");
        assert_eq!(undo.term, Some(Term::Mark(',')));
        assert_eq!(undo.spaces, 1);
    }

    #[test]
    fn comma_and_space_stay_with_the_word() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello");
        assert!(engine.end_word(Term::Mark(','), &dict).is_none());
        assert!(engine.end_word(Term::Space, &dict).is_none());

        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 7);
        assert_eq!(flip.insert, "руддщ");
        assert_eq!(flip.switch_to, Script::Ru);
        assert_eq!(flip.term, Some(Term::Mark(',')));
        assert_eq!(flip.spaces, 1);

        let back = engine.force().unwrap();
        assert_eq!(back.insert, "hello");
        assert_eq!(back.switch_to, Script::En);
        assert_eq!(back.term, Some(Term::Mark(',')));
        assert_eq!(back.spaces, 1);
    }

    #[test]
    fn period_or_comma_typed_inside_the_word_flips_with_the_space() {
        let dict = dict();
        let mut engine = Engine::new();
        type_word(&mut engine, "hello.");
        assert!(engine.end_word(Term::Space, &dict).is_none());
        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 7);
        assert_eq!(flip.insert, "руддщ.");
        assert_eq!(flip.term, Some(Term::Space));
        assert_eq!(flip.spaces, 0);

        let mut engine = Engine::new();
        type_word(&mut engine, "hello,");
        assert!(engine.end_word(Term::Space, &dict).is_none());
        let flip = engine.force().unwrap();
        assert_eq!(flip.delete, 7);
        assert_eq!(flip.insert, "руддщ,");
        assert_eq!(flip.term, Some(Term::Space));
        assert_eq!(flip.spaces, 0);
    }
}
