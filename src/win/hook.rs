use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PAUSE, VK_PRIOR,
    VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW, SetWindowsHookExW,
    UnhookWindowsHookEx, GUITHREADINFO, GWL_STYLE, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
    LLKHF_LOWER_IL_INJECTED, LLKHF_UP, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_RBUTTONDOWN, WM_XBUTTONDOWN,
};

use zyabra::{KeyClass, Term};

use super::{dict, foreground_script, runtime, ui, EXTRA};

pub(super) struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

// Win32 hook handles are process-wide. `Send` lets them live in a mutex.
unsafe impl Send for Hooks {}

const ES_PASSWORD: isize = 0x20;

pub(super) fn install() -> Result<(), String> {
    unsafe {
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
            .map_err(|err| format!("не удалось поставить хук клавиатуры: {err}"))?;
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
            .map_err(|err| format!("не удалось поставить хук мыши: {err}"))?;
        *super::HOOKS.lock().unwrap_or_else(|err| err.into_inner()) =
            Some(Hooks { keyboard, mouse });
    }
    Ok(())
}

pub(super) fn uninstall() {
    let hooks = super::HOOKS
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .take();
    if let Some(hooks) = hooks {
        unsafe {
            let _ = UnhookWindowsHookEx(hooks.keyboard);
            let _ = UnhookWindowsHookEx(hooks.mouse);
        }
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let injected =
        info.flags.contains(LLKHF_INJECTED) || info.flags.contains(LLKHF_LOWER_IL_INJECTED);
    if injected || info.dwExtraInfo == EXTRA {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let swallow = handle_key(
        info.vkCode,
        (info.scanCode & 0xFF) as u16,
        info.flags.contains(LLKHF_UP),
    );
    if swallow {
        LRESULT(1)
    } else {
        CallNextHookEx(None, code, wparam, lparam)
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        if msg == WM_LBUTTONDOWN
            || msg == WM_RBUTTONDOWN
            || msg == WM_MBUTTONDOWN
            || msg == WM_XBUTTONDOWN
        {
            if let Ok(mut rt) = super::RUNTIME.try_lock() {
                rt.engine.cancel();
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn handle_key(vk: u32, scan: u16, is_up: bool) -> bool {
    if is_up {
        if vk == VK_PAUSE.0 as u32 {
            let mut rt = runtime();
            rt.pause_down = false;
            return rt.enabled;
        }
        return false;
    }

    if focus_is_password() || foreground_is_self() {
        runtime().engine.forget();
        return false;
    }

    let words = dict();
    let mut rt = runtime();
    if !rt.enabled {
        return false;
    }

    if vk == VK_PAUSE.0 as u32 {
        if rt.pause_down {
            return true;
        }
        rt.pause_down = true;
        let rewrite = rt.engine.force();
        drop(rt);
        if let Some(rewrite) = rewrite {
            super::send::apply(rewrite);
        }
        return true;
    }

    if vk == VK_BACK.0 as u32 {
        rt.engine.backspace();
        return false;
    }

    // `!` and `?` are typed with Shift. Shift itself is not a character, and
    // dropping the word here would throw it away before the mark arrives.
    if is_shift_or_caps(vk) {
        return false;
    }

    if is_chord(vk) {
        rt.engine.cancel();
        return false;
    }

    if is_navigation(vk) {
        rt.engine.cancel();
        return false;
    }

    if let Some(term) = fixed_term(vk) {
        let rewrite = rt.engine.end_word(term, words);
        drop(rt);
        if let Some(rewrite) = rewrite {
            super::send::apply(rewrite);
            return true;
        }
        return false;
    }

    let Some(script) = foreground_script() else {
        rt.engine.cancel();
        return false;
    };
    match classify_key(scan, script) {
        KeyClass::Glyph(ch) => {
            rt.engine.push_glyph(ch);
            false
        }
        KeyClass::End(term) => {
            let rewrite = rt.engine.end_word(term, words);
            drop(rt);
            if let Some(rewrite) = rewrite {
                super::send::apply(rewrite);
                true
            } else {
                false
            }
        }
        KeyClass::Other => {
            rt.engine.cancel();
            false
        }
    }
}

fn classify_key(scan: u16, script: zyabra::Script) -> KeyClass {
    zyabra::classify(scan, shift_down(), caps_on(), script)
}

fn fixed_term(vk: u32) -> Option<Term> {
    if vk == VK_SPACE.0 as u32 {
        return Some(Term::Space);
    }
    if vk == VK_RETURN.0 as u32 {
        return Some(Term::Enter);
    }
    if vk == VK_TAB.0 as u32 {
        return Some(Term::Tab);
    }
    numpad_mark(vk).map(Term::Mark)
}

fn numpad_mark(vk: u32) -> Option<char> {
    match vk {
        0x60 => Some('0'),
        0x61 => Some('1'),
        0x62 => Some('2'),
        0x63 => Some('3'),
        0x64 => Some('4'),
        0x65 => Some('5'),
        0x66 => Some('6'),
        0x67 => Some('7'),
        0x68 => Some('8'),
        0x69 => Some('9'),
        0x6E => Some('.'),
        _ => None,
    }
}

fn is_navigation(vk: u32) -> bool {
    const NAV: &[u16] = &[
        VK_LEFT.0,
        VK_RIGHT.0,
        VK_UP.0,
        VK_DOWN.0,
        VK_HOME.0,
        VK_END.0,
        VK_DELETE.0,
        VK_PRIOR.0,
        VK_NEXT.0,
        VK_ESCAPE.0,
        VK_INSERT.0,
    ];
    NAV.contains(&(vk as u16)) || (0x70..=0x87).contains(&vk)
}

fn is_shift_or_caps(vk: u32) -> bool {
    vk == VK_SHIFT.0 as u32 || vk == 0xA0 || vk == 0xA1 || vk == VK_CAPITAL.0 as u32
}

fn is_chord(vk: u32) -> bool {
    if vk == VK_SHIFT.0 as u32 || vk == 0xA0 || vk == 0xA1 || vk == VK_CAPITAL.0 as u32 {
        return false;
    }
    if vk == VK_CONTROL.0 as u32
        || vk == VK_MENU.0 as u32
        || vk == VK_LWIN.0 as u32
        || vk == VK_RWIN.0 as u32
        || vk == 0xA2
        || vk == 0xA3
        || vk == 0xA4
        || vk == 0xA5
    {
        return true;
    }
    vk_down(VK_CONTROL) || vk_down(VK_MENU) || vk_down(VK_LWIN) || vk_down(VK_RWIN)
}

fn shift_down() -> bool {
    vk_down(VK_SHIFT)
        || vk_down(windows::Win32::UI::Input::KeyboardAndMouse::VK_LSHIFT)
        || vk_down(windows::Win32::UI::Input::KeyboardAndMouse::VK_RSHIFT)
}

fn caps_on() -> bool {
    unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0 }
}

fn vk_down(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

fn focus_is_password() -> bool {
    unsafe {
        let mut info = GUITHREADINFO::default();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(0, &mut info).is_err() {
            return false;
        }
        if info.hwndFocus.0.is_null() {
            return false;
        }
        let style = GetWindowLongPtrW(info.hwndFocus, GWL_STYLE);
        style & ES_PASSWORD != 0
    }
}

fn foreground_is_self() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    let window = ui().window;
    !fg.0.is_null() && fg.0 == window.0
}
