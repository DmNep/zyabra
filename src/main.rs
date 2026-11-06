//! Tray program on Windows, evdev program on Linux, event tap on macOS.
//! The library crate decides; this file only talks to the system.
//! Typed text is never written anywhere.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod win;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;

fn main() {
    let force_key = match zyabra::parse_args(std::env::args().skip(1)) {
        Ok(zyabra::Launch::Help) => {
            explain(&zyabra::help_text(&zyabra::installed_path()), false);
            return;
        }
        Ok(zyabra::Launch::Run(key)) => key,
        Err(err) => {
            explain(&err, true);
            std::process::exit(1);
        }
    };
    #[cfg(windows)]
    {
        if let Err(err) = win::run(force_key) {
            win::report_error(&err);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Err(err) = linux::run(force_key) {
            eprintln!("Зябра: {err}");
            std::process::exit(1);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Err(err) = mac::run(force_key) {
            eprintln!("Зябра: {err}");
            std::process::exit(1);
        }
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        eprintln!("Зябра работает только в Windows, Linux и macOS.");
        std::process::exit(1);
    }
}

fn explain(message: &str, error: bool) {
    #[cfg(windows)]
    {
        if win::write_console(message, error) {
            return;
        }
        if error {
            win::report_error(message);
        } else {
            win::report_info(message);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = error;
        eprintln!("Зябра: {message}");
    }
}
