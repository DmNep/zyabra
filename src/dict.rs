use std::io::Read;

use flate2::read::GzDecoder;

use crate::keymap::Script;

pub trait HasWords {
    fn has(&self, script: Script, normalized: &str) -> bool;
}

pub struct MapDict {
    en: Vec<String>,
    ru: Vec<String>,
}

impl MapDict {
    pub fn new(en: &[&str], ru: &[&str]) -> Self {
        Self {
            en: prepare(en),
            ru: prepare(ru),
        }
    }
}

impl HasWords for MapDict {
    fn has(&self, script: Script, normalized: &str) -> bool {
        let list = match script {
            Script::En => &self.en,
            Script::Ru => &self.ru,
        };
        list.binary_search_by(|item| item.as_str().cmp(normalized))
            .is_ok()
    }
}

pub struct StaticDict {
    en: Vec<&'static str>,
    ru: Vec<&'static str>,
}

impl StaticDict {
    pub fn load() -> Self {
        let mut en = collect(inflate(include_bytes!(concat!(env!("OUT_DIR"), "/en.txt.gz"))), is_en_word);
        let mut ru = collect(inflate(include_bytes!(concat!(env!("OUT_DIR"), "/ru.txt.gz"))), is_ru_word);
        en.extend(load_extra(
            include_str!("../dictionaries/extra-en.txt"),
            is_extra_en,
        ));
        ru.extend(load_extra(
            include_str!("../dictionaries/extra-ru.txt"),
            is_ru_word,
        ));
        en.sort_unstable();
        en.dedup();
        ru.sort_unstable();
        ru.dedup();
        Self { en, ru }
    }
}

fn inflate(bytes: &[u8]) -> &'static str {
    let mut text = String::new();
    GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .expect("embedded dictionary");
    Box::leak(text.into_boxed_str())
}

impl HasWords for StaticDict {
    fn has(&self, script: Script, normalized: &str) -> bool {
        let list: &[&str] = match script {
            Script::En => &self.en,
            Script::Ru => &self.ru,
        };
        list.binary_search(&normalized).is_ok()
    }
}

fn prepare(words: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = words.iter().map(|word| normalize(word)).collect();
    out.sort();
    out.dedup();
    out
}

fn collect(text: &'static str, keep: fn(&str) -> bool) -> Vec<&'static str> {
    text.lines()
        .map(str::trim)
        .filter(|word| keep(word))
        .collect()
}

fn load_extra(text: &str, keep: fn(&str) -> bool) -> Vec<&'static str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let word = normalize(line);
            if keep(&word) {
                Some(&*Box::leak(word.into_boxed_str()))
            } else {
                None
            }
        })
        .collect()
}

fn is_en_word(word: &str) -> bool {
    (2..=20).contains(&word.len()) && word.bytes().all(|b| b.is_ascii_lowercase())
}

/// Letters, or a run of periods. A period is the English face of `ю`.
fn is_extra_en(word: &str) -> bool {
    let len = word.len();
    (2..=20).contains(&len)
        && (word.bytes().all(|b| b.is_ascii_lowercase()) || word.bytes().all(|b| b == b'.'))
}

fn is_ru_word(word: &str) -> bool {
    let len = word.chars().count();
    (2..=20).contains(&len) && word.chars().all(|ch| ('а'..='я').contains(&ch))
}

pub fn normalize(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for ch in word.chars() {
        for lower in ch.to_lowercase() {
            out.push(if lower == 'ё' { 'е' } else { lower });
        }
    }
    out
}
