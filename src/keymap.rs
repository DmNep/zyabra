//! Physical US QWERTY ↔ Russian ЙЦУКЕН keys.
//!
//! Characters are paired by the key that produces them, not by alphabetical order.
//! Shifted letters follow Unicode case. Shifted punctuation is listed explicitly
//! because the two layouts disagree (`@` versus `"` and so on).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    En,
    Ru,
}

impl Script {
    pub fn other(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        }
    }
}

/// How a finished word was terminated. The platform replays this after a rewrite
/// so the character the user pressed still appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Term {
    Space,
    Enter,
    Tab,
    Mark(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyClass {
    /// Part of the current word. Passed through to the focused application.
    Glyph(char),
    /// Ends the word. May be swallowed and replayed after a correction.
    End(Term),
    /// Navigation and unknown keys. The unfinished word is dropped.
    Other,
}

struct KeyDef {
    scan: u16,
    en: char,
    ru: char,
    en_shift: Option<char>,
    ru_shift: Option<char>,
}

struct NumDef {
    scan: u16,
    plain: char,
    en_shift: char,
    ru_shift: char,
}

/// Set 1 make-codes for the keys whose unshifted characters differ, or that
/// carry a Russian letter on a US punctuation key.
const KEYS: &[KeyDef] = &[
    KeyDef {
        scan: 0x29,
        en: '`',
        ru: 'ё',
        en_shift: Some('~'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x10,
        en: 'q',
        ru: 'й',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x11,
        en: 'w',
        ru: 'ц',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x12,
        en: 'e',
        ru: 'у',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x13,
        en: 'r',
        ru: 'к',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x14,
        en: 't',
        ru: 'е',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x15,
        en: 'y',
        ru: 'н',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x16,
        en: 'u',
        ru: 'г',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x17,
        en: 'i',
        ru: 'ш',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x18,
        en: 'o',
        ru: 'щ',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x19,
        en: 'p',
        ru: 'з',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x1A,
        en: '[',
        ru: 'х',
        en_shift: Some('{'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x1B,
        en: ']',
        ru: 'ъ',
        en_shift: Some('}'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x1E,
        en: 'a',
        ru: 'ф',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x1F,
        en: 's',
        ru: 'ы',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x20,
        en: 'd',
        ru: 'в',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x21,
        en: 'f',
        ru: 'а',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x22,
        en: 'g',
        ru: 'п',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x23,
        en: 'h',
        ru: 'р',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x24,
        en: 'j',
        ru: 'о',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x25,
        en: 'k',
        ru: 'л',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x26,
        en: 'l',
        ru: 'д',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x27,
        en: ';',
        ru: 'ж',
        en_shift: Some(':'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x28,
        en: '\'',
        ru: 'э',
        en_shift: Some('"'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x2C,
        en: 'z',
        ru: 'я',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x2D,
        en: 'x',
        ru: 'ч',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x2E,
        en: 'c',
        ru: 'с',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x2F,
        en: 'v',
        ru: 'м',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x30,
        en: 'b',
        ru: 'и',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x31,
        en: 'n',
        ru: 'т',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x32,
        en: 'm',
        ru: 'ь',
        en_shift: None,
        ru_shift: None,
    },
    KeyDef {
        scan: 0x33,
        en: ',',
        ru: 'б',
        en_shift: Some('<'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x34,
        en: '.',
        ru: 'ю',
        en_shift: Some('>'),
        ru_shift: None,
    },
    KeyDef {
        scan: 0x35,
        en: '/',
        ru: '.',
        en_shift: Some('?'),
        ru_shift: Some(','),
    },
];

const NUMS: &[NumDef] = &[
    NumDef {
        scan: 0x02,
        plain: '1',
        en_shift: '!',
        ru_shift: '!',
    },
    NumDef {
        scan: 0x03,
        plain: '2',
        en_shift: '@',
        ru_shift: '"',
    },
    NumDef {
        scan: 0x04,
        plain: '3',
        en_shift: '#',
        ru_shift: '№',
    },
    NumDef {
        scan: 0x05,
        plain: '4',
        en_shift: '$',
        ru_shift: ';',
    },
    NumDef {
        scan: 0x06,
        plain: '5',
        en_shift: '%',
        ru_shift: '%',
    },
    NumDef {
        scan: 0x07,
        plain: '6',
        en_shift: '^',
        ru_shift: ':',
    },
    NumDef {
        scan: 0x08,
        plain: '7',
        en_shift: '&',
        ru_shift: '?',
    },
    NumDef {
        scan: 0x09,
        plain: '8',
        en_shift: '*',
        ru_shift: '*',
    },
    NumDef {
        scan: 0x0A,
        plain: '9',
        en_shift: '(',
        ru_shift: '(',
    },
    NumDef {
        scan: 0x0B,
        plain: '0',
        en_shift: ')',
        ru_shift: ')',
    },
    NumDef {
        scan: 0x0C,
        plain: '-',
        en_shift: '_',
        ru_shift: '_',
    },
    NumDef {
        scan: 0x0D,
        plain: '=',
        en_shift: '+',
        ru_shift: '+',
    },
];

/// A physical key and whether Shift must be held to produce a character.
/// `code` is the AT set-1 scan code, which is also the Linux evdev key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub code: u16,
    pub shift: bool,
}

pub fn chord_for(script: Script, ch: char) -> Option<Chord> {
    let lower = ch.to_lowercase().next()?;
    let letter = ch.is_alphabetic();
    for key in KEYS {
        let (base, shifted) = match script {
            Script::En => (key.en, key.en_shift),
            Script::Ru => (key.ru, key.ru_shift),
        };
        if letter && base == lower {
            return Some(Chord {
                code: key.scan,
                shift: ch.is_uppercase(),
            });
        }
        if !letter && base == ch {
            return Some(Chord {
                code: key.scan,
                shift: false,
            });
        }
        if !letter && shifted == Some(ch) {
            return Some(Chord {
                code: key.scan,
                shift: true,
            });
        }
    }
    for num in NUMS {
        if num.plain == ch {
            return Some(Chord {
                code: num.scan,
                shift: false,
            });
        }
        let shifted = match script {
            Script::En => num.en_shift,
            Script::Ru => num.ru_shift,
        };
        if shifted == ch {
            return Some(Chord {
                code: num.scan,
                shift: true,
            });
        }
    }
    None
}

pub fn classify(scan: u16, shift: bool, caps: bool, script: Script) -> KeyClass {
    let scan = scan & 0xFF;
    if let Some(def) = KEYS.iter().find(|k| k.scan == scan) {
        return class_letter_key(def, shift, caps, script);
    }
    if let Some(def) = NUMS.iter().find(|k| k.scan == scan) {
        let ch = if shift {
            match script {
                Script::En => def.en_shift,
                Script::Ru => def.ru_shift,
            }
        } else {
            def.plain
        };
        return KeyClass::End(Term::Mark(ch));
    }
    KeyClass::Other
}

fn class_letter_key(def: &KeyDef, shift: bool, caps: bool, script: Script) -> KeyClass {
    let base = match script {
        Script::En => def.en,
        Script::Ru => def.ru,
    };
    let produced = if base.is_alphabetic() {
        let upper = shift ^ caps;
        if upper {
            upper_char(base)
        } else {
            base
        }
    } else if shift {
        match match script {
            Script::En => def.en_shift,
            Script::Ru => def.ru_shift,
        } {
            Some(ch) => ch,
            None => return KeyClass::Other,
        }
    } else {
        base
    };

    let other_is_letter = other_char(script, produced)
        .map(|ch| ch.is_alphabetic())
        .unwrap_or(false);
    if produced.is_alphabetic() || other_is_letter {
        KeyClass::Glyph(produced)
    } else {
        KeyClass::End(Term::Mark(produced))
    }
}

/// What `ch`, typed in `from`, becomes on the same key in the other layout.
/// `!` stays `!`. A Russian comma (Shift+/) becomes `?`, and English `&` becomes `?`.
pub fn other_mark(from: Script, ch: char) -> Option<char> {
    let chord = chord_for(from, ch)?;
    match classify(chord.code, chord.shift, false, from.other()) {
        KeyClass::Glyph(out) | KeyClass::End(Term::Mark(out)) => Some(out),
        KeyClass::End(_) | KeyClass::Other => None,
    }
}

pub fn other_char(from: Script, ch: char) -> Option<char> {
    let lower = ch.to_lowercase().next()?;
    let upper = ch.is_uppercase();
    for key in KEYS {
        let src = match from {
            Script::En => key.en,
            Script::Ru => key.ru,
        };
        if src == lower {
            let dst = match from {
                Script::En => key.ru,
                Script::Ru => key.en,
            };
            return Some(if upper { upper_char(dst) } else { dst });
        }
    }
    None
}

pub fn transpose(from: Script, word: &str) -> Option<String> {
    let mut out = String::with_capacity(word.len());
    for ch in word.chars() {
        out.push(other_char(from, ch)?);
    }
    Some(out)
}

/// Script of the letters in `word`. Punctuation is ignored.
/// Mixed Cyrillic and Latin returns `None`.
pub fn script_of(word: &str) -> Option<Script> {
    let mut seen = None;
    for ch in word.chars() {
        let script = if ch.is_ascii_alphabetic() {
            Some(Script::En)
        } else if is_cyrillic(ch) {
            Some(Script::Ru)
        } else {
            None
        };
        if let Some(script) = script {
            match seen {
                None => seen = Some(script),
                Some(prev) if prev != script => return None,
                Some(_) => {}
            }
        }
    }
    seen
}

fn is_cyrillic(ch: char) -> bool {
    matches!(ch, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

fn upper_char(ch: char) -> char {
    ch.to_uppercase().next().unwrap_or(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_roundtrips() {
        for key in KEYS {
            let ru = other_char(Script::En, key.en).unwrap();
            let en = other_char(Script::Ru, key.ru).unwrap();
            assert_eq!(ru, key.ru, "en {} -> ru", key.en);
            assert_eq!(en, key.en, "ru {} -> en", key.ru);
            assert_eq!(other_char(Script::Ru, ru).unwrap(), key.en);
            assert_eq!(other_char(Script::En, en).unwrap(), key.ru);
        }
    }

    #[test]
    fn classic_words() {
        assert_eq!(transpose(Script::En, "ghbdtn").as_deref(), Some("привет"));
        assert_eq!(transpose(Script::Ru, "привет").as_deref(), Some("ghbdtn"));
        assert_eq!(transpose(Script::En, "cj,frf").as_deref(), Some("собака"));
        assert_eq!(transpose(Script::En, "[jhjij").as_deref(), Some("хорошо"));
        assert_eq!(transpose(Script::En, "Ghbdtn").as_deref(), Some("Привет"));
        assert_eq!(transpose(Script::Ru, "руддщ").as_deref(), Some("hello"));
        assert_eq!(other_mark(Script::Ru, ','), Some('?'));
        assert_eq!(other_mark(Script::En, '&'), Some('?'));
        assert_eq!(other_mark(Script::En, '!'), Some('!'));
        assert_eq!(other_mark(Script::Ru, '?'), Some('&'));
    }

    #[test]
    fn classify_letters_and_punctuation() {
        assert_eq!(
            classify(0x10, false, false, Script::En),
            KeyClass::Glyph('q')
        );
        assert_eq!(
            classify(0x10, false, false, Script::Ru),
            KeyClass::Glyph('й')
        );
        assert_eq!(
            classify(0x10, true, false, Script::En),
            KeyClass::Glyph('Q')
        );
        assert_eq!(classify(0x10, true, true, Script::En), KeyClass::Glyph('q'));
        assert_eq!(
            classify(0x33, false, false, Script::En),
            KeyClass::Glyph(',')
        );
        assert_eq!(
            classify(0x33, true, false, Script::En),
            KeyClass::End(Term::Mark('<'))
        );
        assert_eq!(
            classify(0x33, false, false, Script::Ru),
            KeyClass::Glyph('б')
        );
        assert_eq!(
            classify(0x33, true, false, Script::Ru),
            KeyClass::Glyph('Б')
        );
        assert_eq!(
            classify(0x35, false, false, Script::En),
            KeyClass::End(Term::Mark('/'))
        );
        assert_eq!(
            classify(0x35, false, false, Script::Ru),
            KeyClass::End(Term::Mark('.'))
        );
        assert_eq!(
            classify(0x35, true, false, Script::Ru),
            KeyClass::End(Term::Mark(','))
        );
        assert_eq!(
            classify(0x02, false, false, Script::En),
            KeyClass::End(Term::Mark('1'))
        );
        assert_eq!(
            classify(0x04, true, false, Script::Ru),
            KeyClass::End(Term::Mark('№'))
        );
        assert_eq!(
            classify(0x29, false, false, Script::Ru),
            KeyClass::Glyph('ё')
        );
        assert_eq!(classify(0x00, false, false, Script::En), KeyClass::Other);
    }

    #[test]
    fn chords_match_the_keymap() {
        assert_eq!(
            chord_for(Script::En, 'q'),
            Some(Chord { code: 0x10, shift: false })
        );
        assert_eq!(
            chord_for(Script::Ru, 'й'),
            Some(Chord { code: 0x10, shift: false })
        );
        assert_eq!(
            chord_for(Script::Ru, 'Й'),
            Some(Chord { code: 0x10, shift: true })
        );
        assert_eq!(
            chord_for(Script::En, ','),
            Some(Chord { code: 0x33, shift: false })
        );
        assert_eq!(
            chord_for(Script::Ru, 'б'),
            Some(Chord { code: 0x33, shift: false })
        );
        let word = "привет";
        for ch in word.chars() {
            let chord = chord_for(Script::Ru, ch).unwrap();
            assert_eq!(
                classify(chord.code, chord.shift, false, Script::Ru),
                KeyClass::Glyph(ch)
            );
        }
    }
}
