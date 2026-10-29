use crate::dict::{normalize, HasWords};
use crate::keymap::{script_of, transpose, Script};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub text: String,
    /// Layout the word was typed in.
    pub typed_in: Script,
    /// Layout to leave active. Same as `typed_in` when only the case changes.
    pub switch_to: Script,
}

/// Correct `word` when it is not a word in its own script but the same
/// keystrokes are a word in the other one.
///
/// A dictionary word typed as one lowercase letter and then only capitals
/// (`пРИВЕТ`, `hELLO`) is recased to `Привет` / `Hello` and stays on its
/// layout. All capitals and a capital run after the first letter stay.
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
            return fix_caps_lock(script, &stem, &chars[end..]);
        }
        if let Some(converted) = transpose(script, &stem) {
            if dict.has(script.other(), &normalize(&converted)) {
                let mut text = converted;
                text.extend(chars[end..].iter().copied());
                return Some(Change {
                    text,
                    typed_in: script,
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
        typed_in: script,
        switch_to: script.other(),
    })
}

/// Caps Lock with Shift only on the first letter: `пРИВЕТ` → `Привет`.
/// `None` leaves a dictionary word as it was typed.
fn fix_caps_lock(script: Script, stem: &str, tail: &[char]) -> Option<Change> {
    let fixed = invert_caps_lock(stem)?;
    let mut text = fixed;
    text.extend(tail.iter().copied());
    Some(Change {
        text,
        typed_in: script,
        switch_to: script,
    })
}

fn invert_caps_lock(stem: &str) -> Option<String> {
    let mut chars = stem.chars();
    let first = chars.next()?;
    if !first.is_alphabetic() || !first.is_lowercase() {
        return None;
    }
    let mut out = String::new();
    out.extend(first.to_uppercase());
    let mut rest = false;
    for ch in chars {
        if !ch.is_alphabetic() || !ch.is_uppercase() {
            return None;
        }
        rest = true;
        out.extend(ch.to_lowercase());
    }
    rest.then_some(out)
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
    fn caps_lock_on_the_first_letter_flips_case() {
        let dict = dict();
        let ru = autocorrect("пРИВЕТ", &dict).unwrap();
        assert_eq!(ru.text, "Привет");
        assert_eq!(ru.typed_in, Script::Ru);
        assert_eq!(ru.switch_to, Script::Ru);
        assert_eq!(autocorrect("пРИВЕТ,", &dict).unwrap().text, "Привет,");

        let en = autocorrect("hELLO", &dict).unwrap();
        assert_eq!(en.text, "Hello");
        assert_eq!(en.typed_in, Script::En);
        assert_eq!(en.switch_to, Script::En);
        assert_eq!(autocorrect("hELLO,", &dict).unwrap().text, "Hello,");

        let yo = MapDict::new(&[], &["ежик"]);
        assert_eq!(autocorrect("ёЖИК", &yo).unwrap().text, "Ёжик");
        let short = MapDict::new(&[], &["на"]);
        assert_eq!(autocorrect("нА", &short).unwrap().text, "На");
        let abbr = MapDict::new(&["ios"], &["сша"]);
        assert_eq!(autocorrect("iOS", &abbr).unwrap().text, "Ios");
        assert_eq!(autocorrect("сША", &abbr).unwrap().text, "Сша");
    }

    #[test]
    fn other_case_patterns_stay() {
        let dict = dict();
        assert!(autocorrect("ПРИВЕТ", &dict).is_none());
        assert!(autocorrect("ПРивет", &dict).is_none());
        assert!(autocorrect("пРивет", &dict).is_none());
        assert!(autocorrect("Привет", &dict).is_none());
        assert!(autocorrect("привет", &dict).is_none());
        assert!(autocorrect("HELLO", &dict).is_none());
        assert!(autocorrect("HEllo", &dict).is_none());
        assert!(autocorrect("Hello", &dict).is_none());
        let unknown = MapDict::new(&[], &[]);
        assert!(autocorrect("пРИВЕТ", &unknown).is_none());
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
        assert_eq!(autocorrect("пРИВЕТ", &dict).unwrap().text, "Привет");
        assert_eq!(autocorrect("hELLO", &dict).unwrap().text, "Hello");
        assert!(autocorrect("ПРИВЕТ", &dict).is_none());
        assert!(autocorrect("ПРивет", &dict).is_none());
        assert!(autocorrect("the", &dict).is_none());
        assert!(autocorrect("собака", &dict).is_none());
        assert!(dict.has(Script::En, "..."));
        let dots = autocorrect("ююю", &dict).unwrap();
        assert_eq!(dots.text, "...");
        assert_eq!(dots.switch_to, Script::En);
        assert_eq!(autocorrect("ЮЮЮ", &dict).unwrap().text, "...");
        assert!(autocorrect("юю", &dict).is_none());
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
