//! Which layout is active, and how to select the other one.
//!
//! Sway and Hyprland publish this on a local socket. Everyone else goes through
//! the XKB state of the X server, which GNOME, KDE and X11 sessions keep updated.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{ConnectionExt as _, ModMask};
use x11rb::rust_connection::RustConnection;

use zyabra::Script;

pub enum Layout {
    Xkb { groups: Vec<Option<Script>> },
    Sway { path: PathBuf },
    Hypr { path: PathBuf },
}

struct Groups {
    groups: Vec<Option<Script>>,
    active: usize,
}

pub fn detect(x11: Option<(&RustConnection, u32)>) -> Result<Layout, String> {
    if let Some(path) = env_path("SWAYSOCK") {
        let layout = Layout::Sway { path };
        layout.require_both(None)?;
        return Ok(layout);
    }
    if let Some(path) = hypr_socket() {
        let layout = Layout::Hypr { path };
        layout.require_both(None)?;
        return Ok(layout);
    }
    let Some((conn, root)) = x11 else {
        return Err(
            "нет источника раскладки. Нужен DISPLAY с XKB, сокет Sway или сокет Hyprland."
                .into(),
        );
    };
    let layout = Layout::Xkb {
        groups: xkb_groups(conn, root)?,
    };
    layout.require_both(Some(conn))?;
    Ok(layout)
}

impl Layout {
    pub fn current(&self, conn: Option<&RustConnection>) -> Result<Option<Script>, String> {
        let groups = self.groups(conn)?;
        Ok(groups.groups.get(groups.active).copied().flatten())
    }

    pub fn switch(&self, conn: Option<&RustConnection>, target: Script) -> Result<(), String> {
        let groups = self.groups(conn)?;
        if groups.groups.get(groups.active).copied().flatten() == Some(target) {
            return Ok(());
        }
        let index = groups
            .groups
            .iter()
            .position(|script| *script == Some(target))
            .ok_or_else(|| "в сессии нет нужной раскладки".to_string())?;
        self.set_index(conn, index)?;
        std::thread::sleep(Duration::from_millis(20));
        Ok(())
    }

    fn require_both(&self, conn: Option<&RustConnection>) -> Result<(), String> {
        let groups = self.groups(conn)?;
        let has = |script| groups.groups.iter().any(|item| *item == Some(script));
        if has(Script::En) && has(Script::Ru) {
            Ok(())
        } else {
            Err("нужны обе раскладки: английская (us) и русская ЙЦУКЕН (ru).".into())
        }
    }

    fn groups(&self, conn: Option<&RustConnection>) -> Result<Groups, String> {
        match self {
            Self::Xkb { groups } => {
                let conn = conn.ok_or("XKB нужен DISPLAY")?;
                let reply = conn
                    .xkb_get_state(xkb::ID::USE_CORE_KBD.into())
                    .map_err(xerr)?
                    .reply()
                    .map_err(xerr)?;
                Ok(Groups {
                    groups: groups.clone(),
                    active: usize::from(u8::from(reply.group)),
                })
            }
            Self::Sway { path } => sway_groups(path),
            Self::Hypr { path } => hypr_groups(path),
        }
    }

    fn set_index(&self, conn: Option<&RustConnection>, index: usize) -> Result<(), String> {
        match self {
            Self::Xkb { .. } => {
                let conn = conn.ok_or("XKB нужен DISPLAY")?;
                conn.xkb_latch_lock_state(
                    xkb::ID::USE_CORE_KBD.into(),
                    ModMask::default(),
                    ModMask::default(),
                    true,
                    u8::try_from(index).unwrap_or(0).into(),
                    ModMask::default(),
                    false,
                    0,
                )
                .map_err(xerr)?
                .check()
                .map_err(xerr)?;
                conn.flush().map_err(xerr)?;
                Ok(())
            }
            Self::Sway { path } => {
                let payload = format!("input \"*\" xkb_switch_layout {index}");
                let _ = sway_call(path, 0, payload.as_bytes())?;
                Ok(())
            }
            Self::Hypr { path } => {
                let _ = hypr_call(path, &format!("switchxkblayout all {index}"))?;
                Ok(())
            }
        }
    }
}

fn xkb_groups(conn: &RustConnection, root: u32) -> Result<Vec<Option<Script>>, String> {
    let reply = conn
        .xkb_use_extension(1, 0)
        .map_err(xerr)?
        .reply()
        .map_err(xerr)?;
    if !reply.supported {
        return Err("сервер X не поддерживает XKB".into());
    }
    let atom = conn
        .intern_atom(false, b"_XKB_RULES_NAMES")
        .map_err(xerr)?
        .reply()
        .map_err(xerr)?
        .atom;
    let prop = conn
        .get_property(false, root, atom, 0u32, 0, 1024)
        .map_err(xerr)?
        .reply()
        .map_err(xerr)?;
    let fields: Vec<&[u8]> = prop.value.split(|byte| *byte == 0).collect();
    let layouts = fields
        .get(2)
        .map(|bytes| String::from_utf8_lossy(bytes))
        .ok_or("у окна X нет списка раскладок _XKB_RULES_NAMES")?;
    let groups: Vec<Option<Script>> = layout_codes(&layouts)
        .into_iter()
        .map(script_of_token)
        .collect();
    if groups.is_empty() {
        return Err("список раскладок XKB пуст".into());
    }
    Ok(groups)
}

fn sway_groups(path: &Path) -> Result<Groups, String> {
    let body = sway_call(path, 100, b"")?;
    let text = String::from_utf8_lossy(&body);
    let names = json_array_strings(&text, "xkb_layout_names")
        .ok_or("Sway не сообщил xkb_layout_names")?;
    let active = json_u32_after(&text, "xkb_active_layout_index").unwrap_or(0) as usize;
    Ok(Groups {
        groups: names.iter().map(|name| script_of_token(name)).collect(),
        active,
    })
}

fn hypr_groups(path: &Path) -> Result<Groups, String> {
    let text = hypr_call(path, "j/devices")?;
    let layout = json_string(&text, "layout").ok_or("Hyprland не сообщил layout")?;
    let active_name = json_string(&text, "active_keymap").unwrap_or_default();
    let groups: Vec<Option<Script>> = layout_codes(&layout)
        .into_iter()
        .map(script_of_token)
        .collect();
    let active_script = script_of_token(&active_name);
    let active = groups
        .iter()
        .position(|script| *script == active_script && active_script.is_some())
        .unwrap_or(0);
    Ok(Groups { groups, active })
}

fn sway_call(path: &Path, kind: u32, payload: &[u8]) -> Result<Vec<u8>, String> {
    let mut sock = UnixStream::connect(path).map_err(|err| format!("сокет Sway: {err}"))?;
    sock.set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|err| format!("сокет Sway: {err}"))?;
    let mut message = Vec::with_capacity(14 + payload.len());
    message.extend_from_slice(b"i3-ipc");
    message.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    message.extend_from_slice(&kind.to_ne_bytes());
    message.extend_from_slice(payload);
    sock.write_all(&message)
        .map_err(|err| format!("сокет Sway: {err}"))?;
    let mut header = [0u8; 14];
    sock.read_exact(&mut header)
        .map_err(|err| format!("ответ Sway: {err}"))?;
    if &header[..6] != b"i3-ipc" {
        return Err("ответ Sway не похож на i3-ipc".into());
    }
    let len = u32::from_ne_bytes(header[6..10].try_into().unwrap()) as usize;
    if len > 1_000_000 {
        return Err("ответ Sway слишком большой".into());
    }
    let mut body = vec![0u8; len];
    sock.read_exact(&mut body)
        .map_err(|err| format!("ответ Sway: {err}"))?;
    Ok(body)
}

fn hypr_call(path: &Path, command: &str) -> Result<String, String> {
    let mut sock = UnixStream::connect(path).map_err(|err| format!("сокет Hyprland: {err}"))?;
    sock.set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|err| format!("сокет Hyprland: {err}"))?;
    sock.write_all(command.as_bytes())
        .map_err(|err| format!("сокет Hyprland: {err}"))?;
    let mut body = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match sock.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&buf[..n]),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(err) => return Err(format!("ответ Hyprland: {err}")),
        }
    }
    if body.is_empty() {
        return Err("Hyprland ничего не ответил".into());
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn env_path(key: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var(key).ok()?);
    path.exists().then_some(path)
}

fn hypr_socket() -> Option<PathBuf> {
    let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let base = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let path = PathBuf::from(base)
        .join("hypr")
        .join(signature)
        .join(".socket.sock");
    path.exists().then_some(path)
}

fn layout_codes(layouts: &str) -> Vec<&str> {
    layouts
        .split(',')
        .map(|item| item.trim().split('(').next().unwrap_or(item).trim())
        .filter(|item| !item.is_empty())
        .collect()
}

fn script_of_token(raw: &str) -> Option<Script> {
    let token = raw.trim().to_lowercase();
    let token = token
        .split(['(', ','])
        .next()
        .unwrap_or(token.as_str())
        .trim();
    if token == "ru"
        || token.starts_with("ru-")
        || token.starts_with("russian")
        || token.starts_with("рус")
    {
        Some(Script::Ru)
    } else if token == "us"
        || token == "en"
        || token == "gb"
        || token.starts_with("us-")
        || token.starts_with("en-")
        || token.starts_with("english")
        || token.starts_with("англ")
    {
        Some(Script::En)
    } else {
        None
    }
}

fn json_string(body: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let rest = &body[body.find(&pattern)? + pattern.len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else if ch == '"' {
            break;
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

fn json_array_strings(body: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("\"{key}\"");
    let rest = &body[body.find(&pattern)? + pattern.len()..];
    let start = rest.find('[')?;
    let end = rest[start..].find(']')?;
    let mut out = Vec::new();
    let mut chars = rest[start + 1..start + end].chars();
    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut item = String::new();
        while let Some(next) = chars.next() {
            if next == '\\' {
                if let Some(escaped) = chars.next() {
                    item.push(escaped);
                }
            } else if next == '"' {
                break;
            } else {
                item.push(next);
            }
        }
        out.push(item);
    }
    Some(out)
}

fn json_u32_after(body: &str, key: &str) -> Option<u32> {
    let pattern = format!("\"{key}\"");
    let rest = &body[body.find(&pattern)? + pattern.len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn xerr(err: impl std::fmt::Display) -> String {
    format!("ошибка XKB: {err}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_match_the_two_layouts() {
        assert_eq!(script_of_token("us"), Some(Script::En));
        assert_eq!(script_of_token("ru(winkeys)"), Some(Script::Ru));
        assert_eq!(script_of_token("English (US)"), Some(Script::En));
        assert_eq!(script_of_token("Russian"), Some(Script::Ru));
        assert_eq!(script_of_token("ua"), None);
        assert_eq!(layout_codes("us, ru(winkeys)"), vec!["us", "ru"]);
    }

    #[test]
    fn sway_keyboard_json() {
        let body = r#"[{"type":"keyboard","xkb_active_layout_index":1,"xkb_layout_names":["English (US)","Russian"]}]"#;
        assert_eq!(
            json_array_strings(body, "xkb_layout_names").as_deref(),
            Some(["English (US)".to_string(), "Russian".to_string()].as_slice())
        );
        assert_eq!(json_u32_after(body, "xkb_active_layout_index"), Some(1));
    }

    #[test]
    fn hypr_devices_json() {
        let body = r#"{"keyboards":[{"layout":"us,ru","active_keymap":"Russian"}]}"#;
        assert_eq!(json_string(body, "layout").as_deref(), Some("us,ru"));
        assert_eq!(json_string(body, "active_keymap").as_deref(), Some("Russian"));
    }
}
