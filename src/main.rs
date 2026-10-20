//! Tray program on Windows, evdev program on Linux. The library crate decides;
//! this file only talks to the system. Typed text is never written anywhere.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod win;
#[cfg(unix)]
mod linux;

fn main() {
    #[cfg(windows)]
    {
        if let Err(err) = win::run() {
            win::report_error(&err);
        }
    }
    #[cfg(unix)]
    {
        if let Err(err) = linux::run() {
            eprintln!("Зябра: {err}");
            std::process::exit(1);
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        eprintln!("Зябра работает только в Windows и Linux.");
        std::process::exit(1);
    }
}
