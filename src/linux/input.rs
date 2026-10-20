//! Read the physical keyboard and write the replacement through a virtual one.
//!
//! The physical device is grabbed, so the compositor does not see those keys
//! until we emit them again. The kernel drops the grab when this process exits.

use std::collections::HashSet;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, EventType, InputEvent, KeyCode, LedCode};

pub struct Keys {
    boards: Vec<Device>,
    mice: Vec<Device>,
    virt: VirtualDevice,
    mods: HashSet<u16>,
}

pub enum Incoming {
    Key { code: u16, value: i32 },
    Click,
}

impl Keys {
    pub fn open() -> Result<Self, String> {
        let mut virt = VirtualDevice::builder()
            .and_then(|builder| builder.name("Zyabra").with_keys(&all_keys())?.build())
            .map_err(uinput_error)?;
        // udev needs a moment to publish the new event node. Grabbing it would
        // swallow the keys we ourselves emit.
        thread::sleep(Duration::from_millis(150));
        let own = own_nodes(&mut virt);

        let mut boards = Vec::new();
        let mut mice = Vec::new();
        for (path, mut device) in evdev::enumerate() {
            if own.iter().any(|node| node == &path) || is_ours(&device) {
                continue;
            }
            let Some(keys) = device.supported_keys() else {
                continue;
            };
            if keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_SPACE) {
                if let Err(err) = device.set_nonblocking(true) {
                    return Err(format!("{}: {err}", path.display()));
                }
                if device.grab().is_ok() {
                    boards.push(device);
                }
            } else if keys.contains(KeyCode::BTN_LEFT) && device.set_nonblocking(true).is_ok() {
                mice.push(device);
            }
        }
        if boards.is_empty() {
            return Err(
                "не вижу клавиатуру в /dev/input. Нужна группа input, затем новый вход в сессию."
                    .into(),
            );
        }
        Ok(Self {
            boards,
            mice,
            virt,
            mods: HashSet::new(),
        })
    }

    pub fn poll(&mut self) -> Result<Vec<Incoming>, String> {
        let mut out = Vec::new();
        let mut dead_boards = Vec::new();
        for (index, device) in self.boards.iter_mut().enumerate() {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if event.event_type() == EventType::KEY {
                            out.push(Incoming::Key {
                                code: event.code(),
                                value: event.value(),
                            });
                        }
                    }
                }
                Err(err) if is_idle(&err) => {}
                Err(_) => dead_boards.push(index),
            }
        }
        for index in dead_boards.into_iter().rev() {
            let mut device = self.boards.remove(index);
            let _ = device.ungrab();
        }
        if self.boards.is_empty() {
            return Err("клавиатура отключилась, хват снят".into());
        }

        let mut dead_mice = Vec::new();
        for (index, device) in self.mice.iter_mut().enumerate() {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if event.event_type() == EventType::KEY
                            && event.value() == 1
                            && is_mouse_button(event.code())
                        {
                            out.push(Incoming::Click);
                        }
                    }
                }
                Err(err) if is_idle(&err) => {}
                Err(_) => dead_mice.push(index),
            }
        }
        for index in dead_mice.into_iter().rev() {
            self.mice.remove(index);
        }
        Ok(out)
    }

    pub fn forward(&mut self, code: u16, value: i32) -> Result<(), String> {
        self.emit(code, value)?;
        if is_modifier(code) {
            if value == 0 {
                self.mods.remove(&code);
            } else if value == 1 {
                self.mods.insert(code);
            }
        }
        Ok(())
    }

    pub fn tap(&mut self, code: u16, shift: bool) -> Result<(), String> {
        if shift {
            self.emit(KeyCode::KEY_LEFTSHIFT.0, 1)?;
        }
        self.emit(code, 1)?;
        self.emit(code, 0)?;
        if shift {
            self.emit(KeyCode::KEY_LEFTSHIFT.0, 0)?;
        }
        Ok(())
    }

    /// Release modifiers on the virtual keyboard so the injected word is not
    /// shifted by a key the user is still holding. The physical state stays.
    pub fn pause_mods(&mut self) -> Result<Vec<u16>, String> {
        let held: Vec<u16> = self.mods.iter().copied().collect();
        for code in &held {
            self.emit(*code, 0)?;
        }
        Ok(held)
    }

    pub fn resume_mods(&mut self, held: &[u16]) -> Result<(), String> {
        for code in held {
            if self.mods.contains(code) {
                self.emit(*code, 1)?;
            }
        }
        Ok(())
    }

    pub fn shift_down(&self) -> bool {
        self.mods.contains(&KeyCode::KEY_LEFTSHIFT.0)
            || self.mods.contains(&KeyCode::KEY_RIGHTSHIFT.0)
    }

    pub fn chord_down(&self) -> bool {
        const CODES: &[u16] = &[
            KeyCode::KEY_LEFTCTRL.0,
            KeyCode::KEY_RIGHTCTRL.0,
            KeyCode::KEY_LEFTALT.0,
            KeyCode::KEY_RIGHTALT.0,
            KeyCode::KEY_LEFTMETA.0,
            KeyCode::KEY_RIGHTMETA.0,
        ];
        CODES.iter().any(|code| self.mods.contains(code))
    }

    pub fn caps_lit(&self) -> bool {
        self.boards.iter().any(|device| {
            device
                .get_led_state()
                .ok()
                .is_some_and(|leds| leds.contains(LedCode::LED_CAPSL))
        })
    }

    fn emit(&mut self, code: u16, value: i32) -> Result<(), String> {
        self.virt
            .emit(&[InputEvent::new(EventType::KEY.0, code, value)])
            .map_err(|err| format!("не удалось отправить клавишу: {err}"))
    }
}

impl Drop for Keys {
    fn drop(&mut self) {
        for device in &mut self.boards {
            let _ = device.ungrab();
        }
    }
}

fn all_keys() -> AttributeSet<KeyCode> {
    let mut keys = AttributeSet::new();
    for code in 1..128 {
        keys.insert(KeyCode::new(code));
    }
    keys
}

fn own_nodes(virt: &mut VirtualDevice) -> Vec<PathBuf> {
    virt.enumerate_dev_nodes_blocking()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect()
}

fn is_ours(device: &Device) -> bool {
    device
        .name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Zyabra"))
}

fn is_modifier(code: u16) -> bool {
    matches!(
        code,
        x if x == KeyCode::KEY_LEFTSHIFT.0
            || x == KeyCode::KEY_RIGHTSHIFT.0
            || x == KeyCode::KEY_LEFTCTRL.0
            || x == KeyCode::KEY_RIGHTCTRL.0
            || x == KeyCode::KEY_LEFTALT.0
            || x == KeyCode::KEY_RIGHTALT.0
            || x == KeyCode::KEY_LEFTMETA.0
            || x == KeyCode::KEY_RIGHTMETA.0
    )
}

fn is_mouse_button(code: u16) -> bool {
    matches!(
        code,
        x if x == KeyCode::BTN_LEFT.0
            || x == KeyCode::BTN_RIGHT.0
            || x == KeyCode::BTN_MIDDLE.0
    )
}

fn is_idle(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut
    )
}

fn uinput_error(err: io::Error) -> String {
    format!(
        "не открыть /dev/uinput: {err}. Нужна группа input и правило udev KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\""
    )
}

pub fn is_nav(code: u16) -> bool {
    const CODES: &[u16] = &[
        KeyCode::KEY_ESC.0,
        KeyCode::KEY_HOME.0,
        KeyCode::KEY_END.0,
        KeyCode::KEY_UP.0,
        KeyCode::KEY_DOWN.0,
        KeyCode::KEY_LEFT.0,
        KeyCode::KEY_RIGHT.0,
        KeyCode::KEY_PAGEUP.0,
        KeyCode::KEY_PAGEDOWN.0,
        KeyCode::KEY_INSERT.0,
        KeyCode::KEY_DELETE.0,
        KeyCode::KEY_F1.0,
        KeyCode::KEY_F2.0,
        KeyCode::KEY_F3.0,
        KeyCode::KEY_F4.0,
        KeyCode::KEY_F5.0,
        KeyCode::KEY_F6.0,
        KeyCode::KEY_F7.0,
        KeyCode::KEY_F8.0,
        KeyCode::KEY_F9.0,
        KeyCode::KEY_F10.0,
        KeyCode::KEY_F11.0,
        KeyCode::KEY_F12.0,
    ];
    CODES.contains(&code)
}
