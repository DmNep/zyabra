use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_RETURN, VK_TAB,
};

use zyabra::Term;

use super::EXTRA;

pub(super) fn apply(rewrite: zyabra::Rewrite) {
    send_backspaces(rewrite.delete);
    send_unicode(&rewrite.insert);
    if let Some(term) = rewrite.term {
        send_term(term);
    }
    super::switch_layout(rewrite.switch_to);
}

fn send_term(term: Term) {
    match term {
        Term::Space => send_unicode(" "),
        Term::Enter => send_vk(VK_RETURN),
        Term::Tab => send_vk(VK_TAB),
        Term::Mark(ch) => {
            let mut buf = [0u8; 4];
            send_unicode(ch.encode_utf8(&mut buf));
        }
    }
}

fn send_unicode(text: &str) {
    if text.is_empty() {
        return;
    }
    let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
    for unit in text.encode_utf16() {
        inputs.push(key_input(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
        inputs.push(key_input(
            VIRTUAL_KEY(0),
            unit,
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        ));
    }
    send(&inputs);
}

fn send_backspaces(count: usize) {
    if count == 0 {
        return;
    }
    let mut inputs = Vec::with_capacity(count * 2);
    for _ in 0..count {
        inputs.push(key_input(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK,
            0,
            Default::default(),
        ));
        inputs.push(key_input(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK,
            0,
            KEYEVENTF_KEYUP,
        ));
    }
    send(&inputs);
}

fn send_vk(vk: VIRTUAL_KEY) {
    let inputs = [
        key_input(vk, 0, Default::default()),
        key_input(vk, 0, KEYEVENTF_KEYUP),
    ];
    send(&inputs);
}

fn key_input(
    vk: VIRTUAL_KEY,
    scan: u16,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: EXTRA,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        let _ = SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
