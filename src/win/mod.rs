mod hook;
mod send;
mod ui;

use std::sync::{Mutex, OnceLock};

use windows::core::w;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, LoadKeyboardLayoutW, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, KLF_NOTELLSHELL,
    KLF_SUBSTITUTE_OK,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, MessageBoxW, PostMessageW,
    GUITHREADINFO, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, WM_INPUTLANGCHANGEREQUEST,
};

use zyabra::{Engine, StaticDict};

use hook::Hooks;

pub(super) const EXTRA: usize = 0x5A59_4142;

struct Runtime {
    engine: Engine,
    enabled: bool,
    pause_down: bool,
}

struct UiState {
    window: HWND,
    intro: HWND,
    explain: HWND,
    safety_title: HWND,
    safety: HWND,
    status: HWND,
    toggle: HWND,
    quit: HWND,
    link: HWND,
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
    font: windows::Win32::Graphics::Gdi::HFONT,
    link_font: windows::Win32::Graphics::Gdi::HFONT,
}

// Window handles are process-wide. `Send` lets the UI state live in a mutex.
unsafe impl Send for UiState {}

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    engine: Engine::new(),
    enabled: true,
    pause_down: false,
});

static UI: Mutex<UiState> = Mutex::new(UiState {
    window: HWND(std::ptr::null_mut()),
    intro: HWND(std::ptr::null_mut()),
    explain: HWND(std::ptr::null_mut()),
    safety_title: HWND(std::ptr::null_mut()),
    safety: HWND(std::ptr::null_mut()),
    status: HWND(std::ptr::null_mut()),
    toggle: HWND(std::ptr::null_mut()),
    quit: HWND(std::ptr::null_mut()),
    link: HWND(std::ptr::null_mut()),
    icon: windows::Win32::UI::WindowsAndMessaging::HICON(std::ptr::null_mut()),
    font: windows::Win32::Graphics::Gdi::HFONT(std::ptr::null_mut()),
    link_font: windows::Win32::Graphics::Gdi::HFONT(std::ptr::null_mut()),
});

static DICT: OnceLock<StaticDict> = OnceLock::new();
static HOOKS: Mutex<Option<Hooks>> = Mutex::new(None);

fn runtime() -> std::sync::MutexGuard<'static, Runtime> {
    RUNTIME.lock().unwrap_or_else(|err| err.into_inner())
}

fn ui() -> std::sync::MutexGuard<'static, UiState> {
    UI.lock().unwrap_or_else(|err| err.into_inner())
}

pub fn run(force_key: Option<u32>) -> Result<(), String> {
    if let Some(code) = force_key {
        hook::set_force_key(code);
    }
    let dict = StaticDict::load();
    DICT.set(dict)
        .map_err(|_| "словарь уже загружен".to_string())?;
    ui::run_window()
}

pub fn report_error(message: &str) {
    report(message, true);
}

pub fn report_info(message: &str) {
    report(message, false);
}

fn report(message: &str, error: bool) {
    let wide = utf16(message);
    let title = utf16("Зябра");
    let icon = if error {
        MB_ICONERROR
    } else {
        MB_ICONINFORMATION
    };
    unsafe {
        let _ = MessageBoxW(None, PCWSTR(wide.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | icon);
    }
}

pub(super) fn dict() -> &'static StaticDict {
    DICT.get().expect("dictionary loaded before the hook")
}

pub(super) fn foreground_script() -> Option<zyabra::Script> {
    unsafe {
        let hwnd = input_hwnd();
        if hwnd.0.is_null() {
            return None;
        }
        let thread = GetWindowThreadProcessId(hwnd, None);
        let hkl = GetKeyboardLayout(thread);
        script_from_hkl(hkl.0 as usize as u32)
    }
}

/// Window whose thread interprets keystrokes.
///
/// WebView2 (Tauri, WinForms) keeps focus on a child whose thread is not the
/// top-level window's thread. A layout change updates that child and leaves
/// the top-level thread on the previous layout. Reading `GetForegroundWindow`
/// then classifies English input as Russian, so the typed word is never rewritten.
fn input_hwnd() -> HWND {
    unsafe {
        let mut info = GUITHREADINFO::default();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(0, &mut info).is_ok() && !info.hwndFocus.0.is_null() {
            return info.hwndFocus;
        }
        GetForegroundWindow()
    }
}

fn script_from_hkl(hkl: u32) -> Option<zyabra::Script> {
    match hkl & 0x3FF {
        0x19 => Some(zyabra::Script::Ru),
        0x09 => Some(zyabra::Script::En),
        _ => None,
    }
}

pub(super) fn switch_layout(script: zyabra::Script) {
    let id = match script {
        zyabra::Script::Ru => w!("00000419"),
        zyabra::Script::En => w!("00000409"),
    };
    unsafe {
        let flags = ACTIVATE_KEYBOARD_LAYOUT_FLAGS(KLF_NOTELLSHELL.0 | KLF_SUBSTITUTE_OK.0);
        let Ok(hkl) = LoadKeyboardLayoutW(id, flags) else {
            return;
        };
        let hwnd = input_hwnd();
        if hwnd.0.is_null() {
            return;
        }
        let _ = PostMessageW(
            Some(hwnd),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(hkl.0 as isize),
        );
    }
}

pub(super) fn set_enabled(enabled: bool) {
    let mut rt = runtime();
    if rt.enabled != enabled {
        rt.engine.forget();
    }
    rt.enabled = enabled;
    rt.pause_down = false;
    drop(rt);
    ui::refresh_status();
}

pub(super) fn is_enabled() -> bool {
    runtime().enabled
}

pub(super) fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
