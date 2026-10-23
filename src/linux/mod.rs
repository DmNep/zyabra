//! Linux shell around the shared engine.
//!
//! Keys are taken from evdev and put back with a virtual keyboard, so the same
//! binary serves X11 and Wayland. The layout itself comes from XKB, Sway, or
//! Hyprland. Password fields are not marked on the evdev stream, so those
//! keystrokes are read like any others and still stay in memory only.

mod input;
mod layout;
mod ui;

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use evdev::KeyCode;

use zyabra::{Engine, Force, KeyClass, Rewrite, Script, ShiftPair, StaticDict, Term};

use input::{Incoming, Keys};
use layout::Layout;
use ui::{Display, UiEvent};

pub fn run(force: Option<Force>) -> Result<(), String> {
    let (force_key, double_shift) = match force {
        Some(Force::Key(code)) if code > u32::from(u16::MAX) => {
            return Err(format!("код клавиши {code} не помещается в evdev"));
        }
        Some(Force::Key(code)) => (Some(code as u16), false),
        Some(Force::DoubleShift) => (None, true),
        None => (None, false),
    };
    let lock = single_instance()?;
    let extra_key = if double_shift {
        Some("Shift-Shift".to_string())
    } else {
        force_key.and_then(force_key_label)
    };
    let display = ui::open(extra_key.as_deref())?;
    let layout = layout::detect(display.as_ref().map(|display| (display.conn(), display.root())))?;
    let keys = Keys::open()?;
    let caps = keys.caps_lit();
    if display.is_some() {
        eprintln!("Зябра слушает клавиатуру. Крестик или \"Выход\" отпускают её.");
    } else {
        eprintln!("Зябра слушает клавиатуру без окна. Ctrl+C отпускает её.");
    }
    let mut app = App {
        display,
        layout,
        keys,
        engine: Engine::new(),
        enabled: true,
        pause_down: false,
        force_key,
        double_shift,
        shift_left: false,
        shift_right: false,
        shift_pair: ShiftPair::new(),
        caps,
        swallowed: HashSet::new(),
        _lock: lock,
    };
    loop {
        let enabled = app.enabled;
        let event = match app.display.as_mut() {
            Some(display) => display.poll(enabled)?,
            None => None,
        };
        if let Some(event) = event {
            match event {
                UiEvent::Quit => return Ok(()),
                UiEvent::Toggle => {
                    app.enabled = !app.enabled;
                    app.engine.forget();
                    app.pause_down = false;
                    app.shift_pair.clear();
                    app.swallowed.clear();
                }
                UiEvent::Link => open_repo(),
                UiEvent::Help => {}
            }
            let enabled = app.enabled;
            if let Some(display) = app.display.as_mut() {
                display.redraw(enabled)?;
            }
        }
        for incoming in app.keys.poll()? {
            if let Err(err) = on_input(&mut app, incoming) {
                eprintln!("Зябра: {err}");
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

struct App {
    display: Option<Display>,
    layout: Layout,
    keys: Keys,
    engine: Engine,
    enabled: bool,
    pause_down: bool,
    force_key: Option<u16>,
    double_shift: bool,
    shift_left: bool,
    shift_right: bool,
    shift_pair: ShiftPair,
    caps: bool,
    swallowed: HashSet<u16>,
    _lock: std::fs::File,
}

fn on_input(app: &mut App, incoming: Incoming) -> Result<(), String> {
    match incoming {
        Incoming::Click => {
            app.engine.cancel();
            app.shift_pair.clear();
            Ok(())
        }
        Incoming::Key { code, value } => on_key(app, code, value),
    }
}

fn on_key(app: &mut App, code: u16, value: i32) -> Result<(), String> {
    if app.double_shift && is_shift(code) {
        return on_double_shift(app, code, value);
    }
    if app.double_shift && value == 1 {
        app.shift_pair.clear();
    }
    if value == 0 {
        if is_force_key(app, code) {
            app.pause_down = false;
        }
        if app.swallowed.remove(&code) {
            return Ok(());
        }
        return app.keys.forward(code, value);
    }
    if app.swallowed.contains(&code) {
        return Ok(());
    }
    if app.enabled && !focus_is_ours(app) && is_force_key(app, code) {
        app.swallowed.insert(code);
        if value != 1 || app.pause_down {
            return Ok(());
        }
        app.pause_down = true;
        if read_layout(app).is_none() {
            app.engine.cancel();
            return Ok(());
        }
        if let Some(rewrite) = app.engine.force() {
            apply(app, rewrite)?;
        }
        return Ok(());
    }
    if code == KeyCode::KEY_CAPSLOCK.0 && value == 1 {
        app.caps = !app.caps;
    }
    if is_modifier(code) {
        if value == 1 && is_chord_key(code) && app.enabled {
            app.engine.cancel();
        }
        return app.keys.forward(code, value);
    }
    if !app.enabled || focus_is_ours(app) {
        if focus_is_ours(app) {
            app.engine.forget();
        }
        return app.keys.forward(code, value);
    }
    if app.keys.chord_down() {
        app.engine.cancel();
        return app.keys.forward(code, value);
    }
    if code == KeyCode::KEY_BACKSPACE.0 {
        app.engine.backspace();
        return app.keys.forward(code, value);
    }
    if input::is_nav(code) {
        app.engine.cancel();
        return app.keys.forward(code, value);
    }

    let Some(old) = read_layout(app) else {
        app.engine.cancel();
        return app.keys.forward(code, value);
    };
    if let Some(term) = fixed_term(code) {
        return finish(app, code, value, term);
    }
    match zyabra::classify(code, app.keys.shift_down(), app.caps, old) {
        KeyClass::Glyph(ch) => {
            app.engine.push_glyph(ch);
            app.keys.forward(code, value)
        }
        KeyClass::End(term) => finish(app, code, value, term),
        KeyClass::Other => {
            app.engine.cancel();
            app.keys.forward(code, value)
        }
    }
}

fn is_force_key(app: &App, code: u16) -> bool {
    code == KeyCode::KEY_PAUSE.0 || app.force_key == Some(code)
}

fn force_key_label(code: u16) -> Option<String> {
    let name = match code {
        1 => "Esc",
        14 => "Backspace",
        15 => "Tab",
        16 => "Q",
        17 => "W",
        18 => "E",
        19 => "R",
        20 => "T",
        21 => "Y",
        22 => "U",
        23 => "I",
        24 => "O",
        25 => "P",
        28 => "Enter",
        29 => "Ctrl",
        30 => "A",
        31 => "S",
        32 => "D",
        33 => "F",
        34 => "G",
        35 => "H",
        36 => "J",
        37 => "K",
        38 => "L",
        42 | 54 => "Shift",
        44 => "Z",
        45 => "X",
        46 => "C",
        47 => "V",
        48 => "B",
        49 => "N",
        50 => "M",
        56 => "Alt",
        57 => "Space",
        58 => "Caps Lock",
        59 => "F1",
        60 => "F2",
        61 => "F3",
        62 => "F4",
        63 => "F5",
        64 => "F6",
        65 => "F7",
        66 => "F8",
        67 => "F9",
        68 => "F10",
        69 => "Num Lock",
        70 => "Scroll Lock",
        87 => "F11",
        88 => "F12",
        97 => "Right Ctrl",
        99 => "Print Screen",
        100 => "Right Alt",
        102 => "Home",
        103 => "Up",
        104 => "Page Up",
        105 => "Left",
        106 => "Right",
        107 => "End",
        108 => "Down",
        109 => "Page Down",
        110 => "Insert",
        111 => "Delete",
        119 => "Pause",
        125 => "Left Windows",
        126 => "Right Windows",
        183..=194 => return Some(format!("F{}", code - 183 + 13)),
        _ => return Some(format!("0x{code:X}")),
    };
    if name == "Pause" {
        None
    } else {
        Some(name.to_string())
    }
}

fn finish(app: &mut App, code: u16, value: i32, term: Term) -> Result<(), String> {
    let rewrite = app.engine.end_word(term, dict());
    if let Some(rewrite) = rewrite {
        app.swallowed.insert(code);
        apply(app, rewrite)?;
        Ok(())
    } else {
        app.keys.forward(code, value)
    }
}

fn apply(app: &mut App, rewrite: Rewrite) -> Result<(), String> {
    let held = app.keys.pause_mods()?;
    let typed = type_rewrite(app, &rewrite);
    let resumed = app.keys.resume_mods(&held);
    typed.and(resumed)
}

fn type_rewrite(app: &mut App, rewrite: &Rewrite) -> Result<(), String> {
    for _ in 0..rewrite.delete {
        app.keys.tap(KeyCode::KEY_BACKSPACE.0, false)?;
    }
    switch_to(app, rewrite.switch_to)?;
    type_text(&mut app.keys, rewrite.switch_to, &rewrite.insert)?;
    if let Some(term) = rewrite.term {
        type_term(&mut app.keys, rewrite.switch_to, term)?;
    }
    Ok(())
}

fn switch_to(app: &mut App, script: Script) -> Result<(), String> {
    let App { layout, display, .. } = app;
    layout.switch(display.as_ref().map(|display| display.conn()), script)
}

fn type_text(keys: &mut Keys, script: Script, text: &str) -> Result<(), String> {
    for ch in text.chars() {
        let chord = zyabra::chord_for(script, ch)
            .ok_or_else(|| format!("нет клавиши для символа {ch}"))?;
        keys.tap(chord.code, chord.shift)?;
    }
    Ok(())
}

fn type_term(keys: &mut Keys, script: Script, term: Term) -> Result<(), String> {
    match term {
        Term::Space => keys.tap(KeyCode::KEY_SPACE.0, false),
        Term::Enter => keys.tap(KeyCode::KEY_ENTER.0, false),
        Term::Tab => keys.tap(KeyCode::KEY_TAB.0, false),
        Term::Mark(ch) => {
            let chord = zyabra::chord_for(script, ch)
                .ok_or_else(|| format!("нет клавиши для символа {ch}"))?;
            keys.tap(chord.code, chord.shift)
        }
    }
}

fn read_layout(app: &App) -> Option<Script> {
    match app
        .layout
        .current(app.display.as_ref().map(|display| display.conn()))
    {
        Ok(script) => script,
        Err(err) => {
            eprintln!("Зябра: {err}");
            None
        }
    }
}

fn focus_is_ours(app: &App) -> bool {
    app.display
        .as_ref()
        .and_then(|display| display.focused().ok())
        .unwrap_or(false)
}

fn fixed_term(code: u16) -> Option<Term> {
    if code == KeyCode::KEY_SPACE.0 {
        return Some(Term::Space);
    }
    if code == KeyCode::KEY_ENTER.0 || code == KeyCode::KEY_KPENTER.0 {
        return Some(Term::Enter);
    }
    if code == KeyCode::KEY_TAB.0 {
        return Some(Term::Tab);
    }
    numpad(code).map(Term::Mark)
}

fn numpad(code: u16) -> Option<char> {
    Some(match code {
        c if c == KeyCode::KEY_KP0.0 => '0',
        c if c == KeyCode::KEY_KP1.0 => '1',
        c if c == KeyCode::KEY_KP2.0 => '2',
        c if c == KeyCode::KEY_KP3.0 => '3',
        c if c == KeyCode::KEY_KP4.0 => '4',
        c if c == KeyCode::KEY_KP5.0 => '5',
        c if c == KeyCode::KEY_KP6.0 => '6',
        c if c == KeyCode::KEY_KP7.0 => '7',
        c if c == KeyCode::KEY_KP8.0 => '8',
        c if c == KeyCode::KEY_KP9.0 => '9',
        c if c == KeyCode::KEY_KPDOT.0 => '.',
        _ => return None,
    })
}

fn on_double_shift(app: &mut App, code: u16, value: i32) -> Result<(), String> {
    if value == 0 {
        if code == KeyCode::KEY_LEFTSHIFT.0 {
            app.shift_left = false;
        } else {
            app.shift_right = false;
        }
        return app.keys.forward(code, value);
    }
    if value != 1 {
        return app.keys.forward(code, value);
    }
    let already = if code == KeyCode::KEY_LEFTSHIFT.0 {
        app.shift_left
    } else {
        app.shift_right
    };
    if already {
        return app.keys.forward(code, value);
    }
    let other = if code == KeyCode::KEY_LEFTSHIFT.0 {
        app.shift_right
    } else {
        app.shift_left
    };
    if code == KeyCode::KEY_LEFTSHIFT.0 {
        app.shift_left = true;
    } else {
        app.shift_right = true;
    }
    if !app.enabled || focus_is_ours(app) {
        app.shift_pair.clear();
        if focus_is_ours(app) {
            app.engine.forget();
        }
        return app.keys.forward(code, value);
    }
    if other {
        app.shift_pair.clear();
        return app.keys.forward(code, value);
    }
    if app.shift_pair.press(Instant::now()) {
        if read_layout(app).is_none() {
            app.engine.cancel();
        } else if let Some(rewrite) = app.engine.force() {
            apply(app, rewrite)?;
        }
    }
    app.keys.forward(code, value)
}

fn is_modifier(code: u16) -> bool {
    is_shift(code) || is_chord_key(code) || code == KeyCode::KEY_CAPSLOCK.0
}

fn is_shift(code: u16) -> bool {
    code == KeyCode::KEY_LEFTSHIFT.0 || code == KeyCode::KEY_RIGHTSHIFT.0
}

fn is_chord_key(code: u16) -> bool {
    code == KeyCode::KEY_LEFTCTRL.0
        || code == KeyCode::KEY_RIGHTCTRL.0
        || code == KeyCode::KEY_LEFTALT.0
        || code == KeyCode::KEY_RIGHTALT.0
        || code == KeyCode::KEY_LEFTMETA.0
        || code == KeyCode::KEY_RIGHTMETA.0
}

fn dict() -> &'static StaticDict {
    static DICT: OnceLock<StaticDict> = OnceLock::new();
    DICT.get_or_init(StaticDict::load)
}

fn open_repo() {
    let _ = std::process::Command::new("xdg-open")
        .arg("https://github.com/DmNep/zyabra")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn single_instance() -> Result<std::fs::File, String> {
    let path = lock_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|err| format!("файл блокировки {}: {err}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => {
            let _ = ui::raise_existing();
            eprintln!("Зябра уже запущена.");
            std::process::exit(0);
        }
        Err(std::fs::TryLockError::Error(err)) => Err(format!("файл блокировки: {err}")),
    }
}

fn lock_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("zyabra.lock");
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    PathBuf::from("/tmp").join(format!("zyabra-{user}.lock"))
}
