//! Windows tray program. The library crate decides; this file only talks to Win32.
//! Release builds have no console. Typed text is never written anywhere.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

fn main() {
    #[cfg(windows)]
    {
        if let Err(err) = win::run() {
            win::report_error(&err);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("Зябра работает только в Windows.");
        std::process::exit(1);
    }
}
