//! Core of Zyabra. This crate has no I/O: it does not read the keyboard,
//! write files, or open sockets. The program in `main.rs` feeds it one
//! current word and applies the result.

#![forbid(unsafe_code)]

mod args;
mod decide;
mod dict;
mod engine;
mod keymap;

pub use args::{help_text, installed_path, parse_args, Launch, USAGE};
pub use decide::{autocorrect, force_convert, Change};
pub use dict::{HasWords, MapDict, StaticDict};
pub use engine::{Engine, Rewrite};
pub use keymap::{chord_for, classify, Chord, KeyClass, Script, Term};
