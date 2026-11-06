//! macOS shell around the shared engine.
//!
//! A session event tap reads keys and a virtual keyboard puts the correction
//! back. Layouts come from enabled input sources. Secure Input (password
//! fields) hides the keystrokes, same as a password box on Windows.

mod ui;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use zyabra::{Engine, Force, KeyClass, Rewrite, Script, ShiftPair, StaticDict, Term};

const MAGIC: i64 = 0x5A59_4142;
const FIELD_AUTOREPEAT: u32 = 8;
const FIELD_KEYCODE: u32 = 9;
const FIELD_SOURCE_PID: u32 = 41;
const FIELD_USER_DATA: u32 = 42;
const EVENT_LEFT_MOUSE: u32 = 1;
const EVENT_RIGHT_MOUSE: u32 = 3;
const EVENT_KEY_DOWN: u32 = 10;
const EVENT_KEY_UP: u32 = 11;
const EVENT_FLAGS: u32 = 12;
const EVENT_OTHER_MOUSE: u32 = 25;
const TAP_DISABLED_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_USER: u32 = 0xFFFF_FFFF;
const FLAG_CAPS: u64 = 0x0001_0000;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;
const KEY_RETURN: u16 = 0x24;
const KEY_TAB: u16 = 0x30;
const KEY_SPACE: u16 = 0x31;
const KEY_BACKSPACE: u16 = 0x33;
const KEY_SHIFT: u16 = 0x38;
const KEY_CAPS: u16 = 0x39;
const KEY_RIGHT_SHIFT: u16 = 0x3C;

const PERMISSION: &str = "\
Нет доступа к клавиатуре. Системные настройки → Конфиденциальность и безопасность → \
Универсальный доступ и Мониторинг ввода: включите Зябру и запустите её снова.";

struct Runtime {
    engine: Engine,
    enabled: bool,
    pause_down: bool,
    shift_left: bool,
    shift_right: bool,
    shift_pair: ShiftPair,
    double_shift: bool,
    force_key: Option<u16>,
}

struct Layouts {
    en: *mut c_void,
    ru: *mut c_void,
}

unsafe impl Send for Layouts {}
unsafe impl Sync for Layouts {}

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    engine: Engine::new(),
    enabled: true,
    pause_down: false,
    shift_left: false,
    shift_right: false,
    shift_pair: ShiftPair::new(),
    double_shift: false,
    force_key: None,
});

static LAYOUTS: OnceLock<Layouts> = OnceLock::new();
static DICT: OnceLock<StaticDict> = OnceLock::new();
static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static EVENT_SOURCE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static INJECTING: AtomicBool = AtomicBool::new(false);

#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "Carbon", kind = "framework")]
#[link(name = "ApplicationServices", kind = "framework")]
#[allow(non_snake_case)]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
    fn CGEventSourceCreate(state: i32) -> *mut c_void;
    fn CGEventSourceSetUserData(source: *mut c_void, data: i64);
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventKeyboardSetUnicodeString(event: *mut c_void, length: usize, text: *const u16);
    fn CGEventPost(tap: u32, event: *mut c_void);
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRelease(value: *const c_void);
    fn CFRetain(value: *const c_void) -> *const c_void;
    fn CFGetTypeID(value: *const c_void) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    fn CFStringGetCString(
        value: *const c_void,
        buffer: *mut i8,
        size: isize,
        encoding: u32,
    ) -> u8;
    fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *mut c_void;
    fn TISCopyCurrentKeyboardInputSource() -> *mut c_void;
    fn TISGetInputSourceProperty(source: *mut c_void, key: *const c_void) -> *const c_void;
    fn TISCreateInputSourceList(properties: *const c_void, include_all: u8) -> *mut c_void;
    fn TISSelectInputSource(source: *mut c_void) -> i32;
    fn IsSecureEventInputEnabled() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;

    #[link_name = "kCFRunLoopCommonModes"]
    static CF_RUN_LOOP_COMMON_MODES: *const c_void;
    #[link_name = "kCFBooleanTrue"]
    static CF_BOOLEAN_TRUE: *const c_void;
    #[link_name = "kCFTypeDictionaryKeyCallBacks"]
    static CF_TYPE_KEY_CALLBACKS: u8;
    #[link_name = "kCFTypeDictionaryValueCallBacks"]
    static CF_TYPE_VALUE_CALLBACKS: u8;
    #[link_name = "kAXTrustedCheckOptionPrompt"]
    static AX_PROMPT: *const c_void;
    #[link_name = "kTISPropertyInputSourceID"]
    static TIS_ID: *const c_void;
    #[link_name = "kTISPropertyInputSourceLanguages"]
    static TIS_LANGUAGES: *const c_void;
    #[link_name = "kTISPropertyInputSourceCategory"]
    static TIS_CATEGORY: *const c_void;
    #[link_name = "kTISCategoryKeyboardInputSource"]
    static TIS_KEYBOARD: *const c_void;
}

pub fn run(force: Option<Force>) -> Result<(), String> {
    let (force_key, double_shift) = match force {
        Some(Force::Key(code)) if code > u32::from(u16::MAX) => {
            return Err(format!("код клавиши {code} не помещается в код macOS"));
        }
        Some(Force::Key(code)) => (Some(code as u16), false),
        Some(Force::DoubleShift) => (None, true),
        None => (None, false),
    };
    ui::start();
    if !single_instance()? {
        ui::alert("Зябра", "Зябра уже запущена.");
        return Ok(());
    }
    if let Err(err) = load_layouts() {
        ui::alert("Зябра", &err);
        return Err(err);
    }
    {
        let mut rt = runtime();
        rt.force_key = force_key;
        rt.double_shift = double_shift;
    }
    let _ = dict();
    ui::open(force)?;
    if install_tap() {
        eprintln!("Зябра слушает клавиатуру. Крестик или \"Выход\" отпускают её.");
    } else {
        eprintln!("Зябра: {PERMISSION}");
        ui::set_status(PERMISSION);
    }
    ui::run_loop();
    Ok(())
}

pub(super) fn toggle_pause() -> bool {
    let mut rt = runtime();
    rt.enabled = !rt.enabled;
    rt.engine.forget();
    rt.pause_down = false;
    rt.shift_pair.clear();
    rt.enabled
}

pub(super) fn fix_current() {
    let rewrite = runtime().engine.force();
    if let Some(rewrite) = rewrite {
        apply(rewrite);
    }
}

fn runtime() -> std::sync::MutexGuard<'static, Runtime> {
    RUNTIME.lock().unwrap_or_else(|err| err.into_inner())
}

fn dict() -> &'static StaticDict {
    DICT.get_or_init(StaticDict::load)
}

fn single_instance() -> Result<bool, String> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    let path = format!("/tmp/zyabra-{}.lock", unsafe { libc::getuid() });
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .open(&path)
        .map_err(|err| format!("файл блокировки {path}: {err}"))?;
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if locked != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            return Ok(false);
        }
        return Err(format!("файл блокировки: {err}"));
    }
    std::mem::forget(file);
    Ok(true)
}

fn load_layouts() -> Result<(), String> {
    let (en, ru, en_id, ru_id) = unsafe { enabled_sources()? };
    eprintln!("Зябра: английская {en_id}, русская {ru_id}.");
    LAYOUTS
        .set(Layouts { en, ru })
        .map_err(|_| "раскладки уже выбраны".to_string())
}

unsafe fn enabled_sources() -> Result<(*mut c_void, *mut c_void, String, String), String> {
    let list = TISCreateInputSourceList(std::ptr::null(), 0);
    if list.is_null() {
        return Err("не удалось прочитать раскладки".into());
    }
    let mut found = Vec::new();
    let count = CFArrayGetCount(list);
    for index in 0..count {
        let source = CFArrayGetValueAtIndex(list, index) as *mut c_void;
        if source.is_null() || !is_keyboard(source) {
            continue;
        }
        let Some(script) = script_of(source) else {
            continue;
        };
        let Some(id) = source_string(source, TIS_ID) else {
            continue;
        };
        found.push((id, script, source));
    }
    let view: Vec<(&str, Script)> = found
        .iter()
        .map(|(id, script, _)| (id.as_str(), *script))
        .collect();
    let Some(en_id) = zyabra::choose_layout_source(&view, Script::En) else {
        CFRelease(list);
        return Err("нужны обе раскладки: английская (us) и русская ЙЦУКЕН (ru).".into());
    };
    let Some(ru_id) = zyabra::choose_layout_source(&view, Script::Ru) else {
        CFRelease(list);
        return Err("нужны обе раскладки: английская (us) и русская ЙЦУКЕН (ru).".into());
    };
    let en_id = en_id.to_string();
    let ru_id = ru_id.to_string();
    let en = found
        .iter()
        .find(|(id, _, _)| *id == en_id)
        .map(|(_, _, source)| *source)
        .unwrap_or(std::ptr::null_mut());
    let ru = found
        .iter()
        .find(|(id, _, _)| *id == ru_id)
        .map(|(_, _, source)| *source)
        .unwrap_or(std::ptr::null_mut());
    if en.is_null() || ru.is_null() {
        CFRelease(list);
        return Err("нужны обе раскладки: английская (us) и русская ЙЦУКЕН (ru).".into());
    }
    CFRetain(en);
    CFRetain(ru);
    CFRelease(list);
    Ok((en, ru, en_id, ru_id))
}

unsafe fn is_keyboard(source: *mut c_void) -> bool {
    let category = TISGetInputSourceProperty(source, TIS_CATEGORY);
    !category.is_null() && CFEqual(category, TIS_KEYBOARD) != 0
}

unsafe fn script_of(source: *mut c_void) -> Option<Script> {
    let languages = TISGetInputSourceProperty(source, TIS_LANGUAGES);
    if languages.is_null() || CFGetTypeID(languages) != CFArrayGetTypeID() {
        return None;
    }
    let mut english = false;
    let count = CFArrayGetCount(languages);
    for index in 0..count {
        let Some(text) = cf_string(CFArrayGetValueAtIndex(languages, index)) else {
            continue;
        };
        if text.starts_with("ru") {
            return Some(Script::Ru);
        }
        if text.starts_with("en") {
            english = true;
        }
    }
    english.then_some(Script::En)
}

unsafe fn source_string(source: *mut c_void, key: *const c_void) -> Option<String> {
    cf_string(TISGetInputSourceProperty(source, key))
}

unsafe fn cf_string(value: *const c_void) -> Option<String> {
    if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
        return None;
    }
    let mut buffer = [0i8; 512];
    let ok = CFStringGetCString(
        value,
        buffer.as_mut_ptr(),
        buffer.len() as isize,
        0x0800_0100,
    );
    if ok == 0 {
        return None;
    }
    let bytes = buffer.iter().take_while(|byte| **byte != 0).count();
    let text = buffer[..bytes]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    String::from_utf8(text).ok()
}

fn current_script() -> Option<Script> {
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() {
            return None;
        }
        let script = script_of(source);
        CFRelease(source);
        script
    }
}

fn switch_to(script: Script) {
    if current_script() == Some(script) {
        return;
    }
    let Some(layouts) = LAYOUTS.get() else {
        return;
    };
    let source = match script {
        Script::En => layouts.en,
        Script::Ru => layouts.ru,
    };
    unsafe {
        let _ = TISSelectInputSource(source);
    }
    std::thread::sleep(Duration::from_millis(20));
}

fn install_tap() -> bool {
    prompt_trust();
    let _ = event_source();
    let mask = bit(EVENT_LEFT_MOUSE)
        | bit(EVENT_RIGHT_MOUSE)
        | bit(EVENT_OTHER_MOUSE)
        | bit(EVENT_KEY_DOWN)
        | bit(EVENT_KEY_UP)
        | bit(EVENT_FLAGS);
    let tap = unsafe { CGEventTapCreate(1, 0, 0, mask, tap_callback, std::ptr::null_mut()) };
    if tap.is_null() {
        return false;
    }
    let source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    if source.is_null() {
        return false;
    }
    unsafe {
        let mode = CF_RUN_LOOP_COMMON_MODES;
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, mode);
        CGEventTapEnable(tap, true);
    }
    TAP.store(tap, Ordering::Relaxed);
    true
}

fn bit(kind: u32) -> u64 {
    1u64 << kind
}

fn prompt_trust() {
    unsafe {
        let key = AX_PROMPT;
        let value = CF_BOOLEAN_TRUE;
        if key.is_null() || value.is_null() {
            return;
        }
        let keys = [key];
        let values = [value];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &CF_TYPE_KEY_CALLBACKS as *const u8 as *const c_void,
            &CF_TYPE_VALUE_CALLBACKS as *const u8 as *const c_void,
        );
        if dict.is_null() {
            return;
        }
        let _ = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict);
    }
}

fn event_source() -> *mut c_void {
    let existing = EVENT_SOURCE.load(Ordering::Relaxed);
    if !existing.is_null() {
        return existing;
    }
    let created = unsafe { CGEventSourceCreate(0) };
    if created.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { CGEventSourceSetUserData(created, MAGIC) };
    EVENT_SOURCE.store(created, Ordering::Relaxed);
    created
}

extern "C" fn tap_callback(
    _proxy: *mut c_void,
    kind: u32,
    event: *mut c_void,
    _user: *mut c_void,
) -> *mut c_void {
    if event.is_null() {
        return event;
    }
    if kind == TAP_DISABLED_TIMEOUT || kind == TAP_DISABLED_USER {
        let tap = TAP.load(Ordering::Relaxed);
        if !tap.is_null() {
            unsafe { CGEventTapEnable(tap, true) };
        }
        return event;
    }
    if INJECTING.load(Ordering::SeqCst) || from_us(event) {
        return event;
    }
    let swallow = match kind {
        EVENT_LEFT_MOUSE | EVENT_RIGHT_MOUSE | EVENT_OTHER_MOUSE => {
            on_click();
            false
        }
        EVENT_FLAGS => {
            on_flags(event);
            false
        }
        EVENT_KEY_DOWN => on_key(event, false),
        EVENT_KEY_UP => on_key(event, true),
        _ => false,
    };
    if swallow {
        std::ptr::null_mut()
    } else {
        event
    }
}

fn from_us(event: *mut c_void) -> bool {
    unsafe {
        if CGEventGetIntegerValueField(event, FIELD_USER_DATA) == MAGIC {
            return true;
        }
        let pid = CGEventGetIntegerValueField(event, FIELD_SOURCE_PID);
        pid != 0 && pid == i64::from(std::process::id())
    }
}

fn on_click() {
    let mut rt = runtime();
    rt.engine.cancel();
    rt.shift_pair.clear();
}

fn on_flags(event: *mut c_void) {
    let code = keycode(event);
    if code == KEY_SHIFT || code == KEY_RIGHT_SHIFT {
        if runtime().double_shift {
            on_shift(code, flags_of(event));
        }
        return;
    }
    if runtime().double_shift {
        runtime().shift_pair.clear();
    }
    if code == KEY_CAPS {
        return;
    }
    runtime().engine.cancel();
}

fn on_shift(code: u16, flags: u64) {
    let blocked = secure_input() || ui::app_is_active();
    let mut rt = runtime();
    let left = code == KEY_SHIFT;
    let was = if left { rt.shift_left } else { rt.shift_right };
    let now_down = (flags & FLAG_SHIFT) != 0 && !was;
    if left {
        rt.shift_left = now_down;
    } else {
        rt.shift_right = now_down;
    }
    if !now_down {
        return;
    }
    let other = if left { rt.shift_right } else { rt.shift_left };
    if blocked || !rt.enabled {
        rt.shift_pair.clear();
        if blocked {
            rt.engine.forget();
        }
        return;
    }
    if other {
        rt.shift_pair.clear();
        return;
    }
    if rt.shift_pair.press(Instant::now()) {
        let rewrite = rt.engine.force();
        drop(rt);
        if let Some(rewrite) = rewrite {
            apply(rewrite);
        }
    }
}

fn on_key(event: *mut c_void, is_up: bool) -> bool {
    let code = keycode(event);
    if runtime().double_shift && !is_up {
        runtime().shift_pair.clear();
    }
    if is_up {
        if runtime().force_key == Some(code) {
            let mut rt = runtime();
            rt.pause_down = false;
            return rt.enabled;
        }
        return false;
    }
    if secure_input() || ui::app_is_active() {
        let mut rt = runtime();
        rt.shift_pair.clear();
        rt.engine.forget();
        return false;
    }
    let mut rt = runtime();
    if !rt.enabled {
        rt.shift_pair.clear();
        return false;
    }
    if rt.force_key == Some(code) {
        if rt.pause_down || autorepeat(event) {
            return true;
        }
        rt.pause_down = true;
        let rewrite = rt.engine.force();
        drop(rt);
        if let Some(rewrite) = rewrite {
            apply(rewrite);
        }
        return true;
    }
    if code == KEY_BACKSPACE {
        rt.engine.backspace();
        return false;
    }
    let flags = flags_of(event);
    if (flags & (FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND)) != 0 {
        rt.engine.cancel();
        return false;
    }
    if let Some(term) = fixed_term(code) {
        let rewrite = rt.engine.end_word(term, dict());
        drop(rt);
        if let Some(rewrite) = rewrite {
            apply(rewrite);
            return true;
        }
        return false;
    }
    let Some(script) = current_script() else {
        rt.engine.cancel();
        return false;
    };
    let Some(scan) = zyabra::mac_keycode_to_scan(code) else {
        rt.engine.cancel();
        return false;
    };
    match zyabra::classify(
        scan,
        (flags & FLAG_SHIFT) != 0,
        (flags & FLAG_CAPS) != 0,
        script,
    ) {
        KeyClass::Glyph(ch) => {
            rt.engine.push_glyph(ch);
            false
        }
        KeyClass::End(term) => {
            let rewrite = rt.engine.end_word(term, dict());
            drop(rt);
            if let Some(rewrite) = rewrite {
                apply(rewrite);
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

fn fixed_term(code: u16) -> Option<Term> {
    match code {
        KEY_SPACE => Some(Term::Space),
        KEY_RETURN => Some(Term::Enter),
        KEY_TAB => Some(Term::Tab),
        _ => None,
    }
}

fn keycode(event: *mut c_void) -> u16 {
    unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) as u16 }
}

fn flags_of(event: *mut c_void) -> u64 {
    unsafe { CGEventGetFlags(event) }
}

fn autorepeat(event: *mut c_void) -> bool {
    unsafe { CGEventGetIntegerValueField(event, FIELD_AUTOREPEAT) != 0 }
}

fn secure_input() -> bool {
    unsafe { IsSecureEventInputEnabled() != 0 }
}

fn apply(rewrite: Rewrite) {
    INJECTING.store(true, Ordering::SeqCst);
    for _ in 0..rewrite.delete {
        post_key(KEY_BACKSPACE, &[]);
    }
    for ch in rewrite.insert.chars() {
        post_char(ch);
    }
    if let Some(term) = rewrite.term {
        match term {
            Term::Space => post_key(KEY_SPACE, &[]),
            Term::Enter => post_key(KEY_RETURN, &[]),
            Term::Tab => post_key(KEY_TAB, &[]),
            Term::Mark(ch) => post_char(ch),
        }
    }
    INJECTING.store(false, Ordering::SeqCst);
    switch_to(rewrite.switch_to);
}

fn post_char(ch: char) {
    let mut units = [0u16; 2];
    let encoded = ch.encode_utf16(&mut units);
    let length = encoded.len();
    post_key(0, &units[..length]);
}

fn post_key(code: u16, units: &[u16]) {
    let source = event_source();
    if source.is_null() {
        return;
    }
    for down in [true, false] {
        unsafe {
            let event = CGEventCreateKeyboardEvent(source, code, down);
            if event.is_null() {
                continue;
            }
            if !units.is_empty() {
                CGEventKeyboardSetUnicodeString(event, units.len(), units.as_ptr());
            }
            CGEventSetFlags(event, 0);
            CGEventSetIntegerValueField(event, FIELD_USER_DATA, MAGIC);
            CGEventPost(0, event);
            CFRelease(event);
        }
    }
}
