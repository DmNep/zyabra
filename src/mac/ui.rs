//! AppKit window. Close quits and the event tap goes with the process.
//! There is no Pause key: "Исправить" does that job, and so does --key.

#![allow(unexpected_cfgs)]

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

#[link(name = "AppKit", kind = "framework")]
#[link(name = "Foundation", kind = "framework")]
extern "C" {}

type Id = *mut Object;

#[cfg(target_arch = "aarch64")]
type ObjcBool = bool;
#[cfg(target_arch = "aarch64")]
const YES: ObjcBool = true;
#[cfg(target_arch = "aarch64")]
const NO: ObjcBool = false;

#[cfg(not(target_arch = "aarch64"))]
type ObjcBool = i8;
#[cfg(not(target_arch = "aarch64"))]
const YES: ObjcBool = 1;
#[cfg(not(target_arch = "aarch64"))]
const NO: ObjcBool = 0;

#[repr(C)]
#[derive(Clone, Copy)]
struct NsPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NsRect {
    origin: NsPoint,
    size: NsSize,
}

struct Widgets {
    status: usize,
    toggle: usize,
}

unsafe impl Send for Widgets {}

static WIDGETS: Mutex<Widgets> = Mutex::new(Widgets {
    status: 0,
    toggle: 0,
});
static DELEGATE: AtomicPtr<Object> = AtomicPtr::new(std::ptr::null_mut());

const TITLE: &str = concat!("Зябра ", env!("CARGO_PKG_VERSION"));
const REPO: &str = "https://github.com/DmNep/zyabra";
const LISTENING: &str = "Зябра слушает клавиатуру.";
const PAUSED: &str = "Пауза: клавиши проходят мимо.";

pub(super) fn start() {
    unsafe {
        let _: Id = msg_send![class!(NSAutoreleasePool), new];
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setActivationPolicy: 0isize];
    }
}

pub(super) fn open(force: Option<zyabra::Force>) -> Result<(), String> {
    unsafe {
        let delegate = controller();
        DELEGATE.store(delegate, Ordering::Relaxed);
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setDelegate: delegate];

        let window: Id = msg_send![class!(NSWindow), alloc];
        let window: Id = msg_send![
            window,
            initWithContentRect: frame(0.0, 0.0, 560.0, 400.0)
            styleMask: 7usize
            backing: 2usize
            defer: NO
        ];
        if window.is_null() {
            return Err("не удалось открыть окно".into());
        }
        let _: () = msg_send![window, setTitle: ns(TITLE)];
        let content: Id = msg_send![window, contentView];
        let intro = label(&intro_text(force), frame(20.0, 332.0, 520.0, 52.0));
        let explain = label(
            "Если слово набрано не в той раскладке, Зябра стирает его и пишет заново в правильной, затем переключает раскладку.",
            frame(20.0, 260.0, 520.0, 64.0),
        );
        let safety = label(
            "Безопасность:\nВ памяти только текущее слово и одна последняя замена. На диск набранное не пишется. В интернет программа не ходит: словарь уже внутри файла. Поля пароля система закрывает, эти нажатия не читаются. \"Пауза\" останавливает обработку. Крестик завершает программу.",
            frame(20.0, 148.0, 520.0, 104.0),
        );
        let status = label(LISTENING, frame(20.0, 116.0, 520.0, 24.0));
        let help = button("Справка", frame(20.0, 72.0, 110.0, 32.0), sel!(help:), delegate);
        let fix = button(
            "Исправить",
            frame(142.0, 72.0, 120.0, 32.0),
            sel!(fix:),
            delegate,
        );
        let toggle = button("Пауза", frame(274.0, 72.0, 120.0, 32.0), sel!(toggle:), delegate);
        let quit = button("Выход", frame(406.0, 72.0, 110.0, 32.0), sel!(quit:), delegate);
        let link = button(REPO, frame(20.0, 24.0, 360.0, 32.0), sel!(link:), delegate);
        for view in [intro, explain, safety, status, help, fix, toggle, quit, link] {
            let _: () = msg_send![content, addSubview: view];
        }
        {
            let mut widgets = WIDGETS.lock().unwrap_or_else(|err| err.into_inner());
            widgets.status = status as usize;
            widgets.toggle = toggle as usize;
        }
        let _: () = msg_send![window, center];
        let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
    }
    Ok(())
}

pub(super) fn run_loop() {
    unsafe {
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, run];
    }
}

pub(super) fn set_status(text: &str) {
    let status = WIDGETS.lock().unwrap_or_else(|err| err.into_inner()).status as Id;
    unsafe { set_string(status, text) };
}

pub(super) fn app_is_active() -> bool {
    unsafe {
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let active: ObjcBool = msg_send![app, isActive];
        active == YES
    }
}

pub(super) fn alert(title: &str, text: &str) {
    unsafe {
        let alert: Id = msg_send![class!(NSAlert), new];
        let _: () = msg_send![alert, setMessageText: ns(title)];
        let _: () = msg_send![alert, setInformativeText: ns(text)];
        let _: isize = msg_send![alert, runModal];
    }
}

fn intro_text(force: Option<zyabra::Force>) -> String {
    match force {
        Some(zyabra::Force::DoubleShift) => "Shift-Shift - исправить текущее слово.".to_string(),
        Some(zyabra::Force::Key(code)) => {
            format!("Клавиша {code:#X} - исправить текущее слово.")
        }
        None => "Клавиши Pause на Маке нет. Кнопка \"Исправить\" или запуск с --key 2sh."
            .to_string(),
    }
}

unsafe fn controller() -> Id {
    let mut decl = ClassDecl::new("ZyabraController", class!(NSObject))
        .expect("класс окна уже зарегистрирован");
    decl.add_method(sel!(help:), help as extern "C" fn(&Object, Sel, Id));
    decl.add_method(sel!(fix:), fix as extern "C" fn(&Object, Sel, Id));
    decl.add_method(sel!(toggle:), toggle as extern "C" fn(&Object, Sel, Id));
    decl.add_method(sel!(quit:), quit as extern "C" fn(&Object, Sel, Id));
    decl.add_method(sel!(link:), link as extern "C" fn(&Object, Sel, Id));
    decl.add_method(
        sel!(applicationShouldTerminateAfterLastWindowClosed:),
        should_terminate as extern "C" fn(&Object, Sel, Id) -> ObjcBool,
    );
    let class = decl.register();
    let object: Id = msg_send![class, alloc];
    msg_send![object, init]
}

extern "C" fn help(_this: &Object, _cmd: Sel, _sender: Id) {
    alert("Зябра", &zyabra::help_text(&zyabra::installed_path()));
}

extern "C" fn fix(_this: &Object, _cmd: Sel, _sender: Id) {
    super::fix_current();
}

extern "C" fn toggle(_this: &Object, _cmd: Sel, _sender: Id) {
    let enabled = super::toggle_pause();
    let (name, status) = if enabled {
        ("Пауза", LISTENING)
    } else {
        ("Продолжить", PAUSED)
    };
    let toggle = WIDGETS.lock().unwrap_or_else(|err| err.into_inner()).toggle as Id;
    unsafe { set_title(toggle, name) };
    set_status(status);
}

extern "C" fn quit(_this: &Object, _cmd: Sel, _sender: Id) {
    unsafe {
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, terminate: std::ptr::null_mut::<Object>()];
    }
}

extern "C" fn link(_this: &Object, _cmd: Sel, _sender: Id) {
    unsafe {
        let workspace: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let url: Id = msg_send![class!(NSURL), URLWithString: ns(REPO)];
        let _: ObjcBool = msg_send![workspace, openURL: url];
    }
}

extern "C" fn should_terminate(_this: &Object, _cmd: Sel, _sender: Id) -> ObjcBool {
    YES
}

unsafe fn label(text: &str, rect: NsRect) -> Id {
    let field: Id = msg_send![class!(NSTextField), alloc];
    let field: Id = msg_send![field, initWithFrame: rect];
    let _: () = msg_send![field, setStringValue: ns(text)];
    let _: () = msg_send![field, setEditable: NO];
    let _: () = msg_send![field, setSelectable: NO];
    let _: () = msg_send![field, setBezeled: NO];
    let _: () = msg_send![field, setBordered: NO];
    let _: () = msg_send![field, setDrawsBackground: NO];
    let cell: Id = msg_send![field, cell];
    if !cell.is_null() {
        let _: () = msg_send![cell, setWraps: YES];
        let _: () = msg_send![cell, setScrollable: NO];
    }
    field
}

unsafe fn button(title: &str, rect: NsRect, action: Sel, target: Id) -> Id {
    let button: Id = msg_send![class!(NSButton), alloc];
    let button: Id = msg_send![button, initWithFrame: rect];
    let _: () = msg_send![button, setTitle: ns(title)];
    let _: () = msg_send![button, setBezelStyle: 1usize];
    let _: () = msg_send![button, setButtonType: 7usize];
    let _: () = msg_send![button, setTarget: target];
    let _: () = msg_send![button, setAction: action];
    button
}

unsafe fn set_string(field: Id, text: &str) {
    if field.is_null() {
        return;
    }
    let _: () = msg_send![field, setStringValue: ns(text)];
}

unsafe fn set_title(button: Id, text: &str) {
    if button.is_null() {
        return;
    }
    let _: () = msg_send![button, setTitle: ns(text)];
}

unsafe fn ns(text: &str) -> Id {
    let object: Id = msg_send![class!(NSString), alloc];
    msg_send![object, initWithBytes: text.as_ptr() length: text.len() encoding: 4usize]
}

fn frame(x: f64, y: f64, width: f64, height: f64) -> NsRect {
    NsRect {
        origin: NsPoint { x, y },
        size: NsSize { width, height },
    }
}
