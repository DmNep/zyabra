use crate::dict::{normalize, HasWords};
use crate::keymap::{script_of, transpose, Script};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub text: String,
    pub switch_to: Script,
}

/// Correct `word` when it is not a word in its own script but the same
/// keystrokes are a word in the other one.
///
/// Trailing punctuation stays as it appeared on screen (`ghbdtn,` → `привет,`).
/// A punctuation mark that is a letter on the other layout stays inside the
/// word (`cj,frf` → `собака`). Nothing is changed when the typed stem is
/// already a dictionary word, when both interpretations are words, or when
/// neither is.
pub fn autocorrect(word: &str, dict: &impl HasWords) -> Option<Change> {
    let script = script_of(word)?;
    let chars: Vec<char> = word.chars().collect();
    let mut end = chars.len();
    while end > 0 {
        if chars[end..].iter().any(|ch| ch.is_alphabetic()) {
            break;
        }
        let stem: String = chars[..end].iter().collect();
        if letter_count(&stem) < 2 {
            break;
        }
        if dict.has(script, &normalize(&stem)) {
            return None;
        }
        if let Some(converted) = transpose(script, &stem) {
            if dict.has(script.other(), &normalize(&converted)) {
                let mut text = converted;
                text.extend(chars[end..].iter().copied());
                return Some(Change {
                    text,
                    switch_to: script.other(),
                });
            }
        }
        if end == 0 {
            break;
        }
        let last = chars[end - 1];
        if last.is_alphabetic() {
            break;
        }
        end -= 1;
    }
    None
}

/// Transpose the current word without asking the dictionary.
/// Trailing punctuation stays as it appeared (`ghbdtn,` → `привет,`).
/// A mark inside the word is still a key of the other layout (`cj,frf` → `собака`).
pub fn force_convert(word: &str) -> Option<Change> {
    let script = script_of(word)?;
    let chars: Vec<char> = word.chars().collect();
    let mut end = chars.len();
    while end > 0 && !chars[end - 1].is_alphabetic() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let stem: String = chars[..end].iter().collect();
    let converted = transpose(script, &stem)?;
    if converted == stem {
        return None;
    }
    let mut text = converted;
    text.extend(chars[end..].iter().copied());
    Some(Change {
        text,
        switch_to: script.other(),
    })
}

fn letter_count(word: &str) -> usize {
    word.chars().filter(|ch| ch.is_alphabetic()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::MapDict;

    fn dict() -> MapDict {
        MapDict::new(
            &["hello", "world", "on", "dog", "the", "cat"],
            &[
                "привет",
                "собака",
                "хорошо",
                "еще",
                "он",
                "не",
                "что",
                "сфе",
            ],
        )
    }

    #[test]
    fn switches_wrong_layout_words() {
        let dict = dict();
        assert_eq!(autocorrect("ghbdtn", &dict).unwrap().text, "привет");
        assert_eq!(autocorrect("Ghbdtn", &dict).unwrap().text, "Привет");
        assert_eq!(autocorrect("cj,frf", &dict).unwrap().text, "собака");
        assert_eq!(autocorrect("[jhjij", &dict).unwrap().text, "хорошо");
        assert_eq!(autocorrect("руддщ", &dict).unwrap().text, "hello");
        assert_eq!(autocorrect("jy", &dict).unwrap().text, "он");
        assert_eq!(autocorrect("xnj", &dict).unwrap().text, "что");
    }

    #[test]
    fn keeps_real_words_and_unknowns() {
        let dict = dict();
        assert!(autocorrect("hello", &dict).is_none());
        assert!(autocorrect("привет", &dict).is_none());
        assert!(autocorrect("on", &dict).is_none());
        assert!(autocorrect("asdf", &dict).is_none());
        assert!(autocorrect("q", &dict).is_none());
        assert!(autocorrect("cat", &dict).is_none());
        assert!(autocorrect("сфе", &dict).is_none());
    }

    #[test]
    fn peels_trailing_punctuation() {
        let dict = dict();
        assert_eq!(autocorrect("ghbdtn,", &dict).unwrap().text, "привет,");
        assert_eq!(autocorrect("ghbdtn.", &dict).unwrap().text, "привет.");
        assert!(autocorrect("hello,", &dict).is_none());
    }

    #[test]
    fn yo_matches_dictionary_without_yo() {
        let dict = dict();
        assert!(autocorrect("ещё", &dict).is_none());
    }

    #[test]
    fn force_ignores_dictionary() {
        assert_eq!(force_convert("asdf").unwrap().text, "фыва");
        assert_eq!(force_convert("ghbdtn,").unwrap().text, "привет,");
        assert_eq!(force_convert("cj,frf").unwrap().text, "собака");
    }

    #[test]
    fn embedded_dictionary_handles_everyday_words() {
        use crate::dict::StaticDict;
        let dict = StaticDict::load();
        assert!(dict.has(Script::Ru, "привет"));
        assert!(dict.has(Script::En, "hello"));
        assert_eq!(autocorrect("ghbdtn", &dict).unwrap().text, "привет");
        assert_eq!(autocorrect("cj,frf", &dict).unwrap().text, "собака");
        assert_eq!(autocorrect("[jhjij", &dict).unwrap().text, "хорошо");
        assert_eq!(autocorrect("руддщ", &dict).unwrap().text, "hello");
        assert!(autocorrect("hello", &dict).is_none());
        assert!(autocorrect("привет", &dict).is_none());
        assert!(autocorrect("the", &dict).is_none());
        assert!(autocorrect("собака", &dict).is_none());
        for word in [
            "code", "file", "data", "true", "false", "name", "test", "main",
        ] {
            assert!(
                autocorrect(word, &dict).is_none(),
                "{word} should stay as typed"
            );
        }
    }
}
