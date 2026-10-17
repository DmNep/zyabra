//! Core of Zyabra. This crate has no I/O: it does not read the keyboard,
//! write files, or open sockets. The Windows program in `main.rs` feeds it
//! one current word and applies the result.

#![forbid(unsafe_code)]

mod decide;
mod dict;
mod engine;
mod keymap;

pub use decide::{autocorrect, force_convert, Change};
pub use dict::{HasWords, MapDict, StaticDict};
pub use engine::{Engine, Rewrite};
pub use keymap::{classify, KeyClass, Script, Term};
