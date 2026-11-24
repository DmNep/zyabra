//! KDE system tray icon through the StatusNotifierItem protocol.

use std::env;
use std::sync::mpsc::Sender;

use ksni::blocking::{Handle, TrayMethods};
use ksni::{Category, ToolTip, Tray};
use libc::{geteuid, seteuid, uid_t};

use super::ui::UiEvent;

const ICON: &str = "input-keyboard";

pub(super) struct ZyabraTray {
    events: Sender<UiEvent>,
}

impl ZyabraTray {
    pub(super) fn new(events: Sender<UiEvent>) -> Self {
        Self { events }
    }
}

impl Tray for ZyabraTray {
    fn id(&self) -> String {
        "zyabra".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn title(&self) -> String {
        "Зябра".into()
    }

    fn icon_name(&self) -> String {
        ICON.into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: ICON.into(),
            title: "Зябра".into(),
            description: "Переключатель раскладки".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.events.send(UiEvent::TrayActivate);
    }
}

pub(super) fn spawn(
    events: Sender<UiEvent>,
) -> Result<Handle<ZyabraTray>, String> {
    let _user = SessionBusUser::enter()?;
    ZyabraTray::new(events).spawn().map_err(|err| err.to_string())
}

struct SessionBusUser {
    previous: uid_t,
}

impl SessionBusUser {
    fn enter() -> Result<Option<Self>, String> {
        if effective_uid() != 0 {
            return Ok(None);
        }
        let Ok(sudo_uid) = env::var("SUDO_UID") else {
            return Ok(None);
        };
        let uid = sudo_uid
            .parse::<uid_t>()
            .map_err(|err| format!("неверный SUDO_UID: {err}"))?;
        if uid == 0 {
            return Ok(None);
        }
        // ksni opens the session bus inside spawn(), so the D-Bus daemon sees
        // root's EXTERNAL identity unless we connect as the original user.
        let previous = effective_uid();
        if change_uid(uid) == -1 {
            return Err(format!(
                "не удалось переключиться на пользователя {uid}: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Some(Self { previous }))
    }
}

impl Drop for SessionBusUser {
    fn drop(&mut self) {
        if change_uid(self.previous) == -1 {
            eprintln!(
                "Зябра: не удалось вернуть UID {}: {}",
                self.previous,
                std::io::Error::last_os_error()
            );
        }
    }
}

fn effective_uid() -> uid_t {
    unsafe { geteuid() }
}

fn change_uid(uid: uid_t) -> i32 {
    unsafe { seteuid(uid) }
}
