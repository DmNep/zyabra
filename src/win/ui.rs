use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, CreateFontW, DeleteObject, GetDC, GetDeviceCaps,
    GetTextExtentPoint32W, ReleaseDC, SelectObject, SetBkMode, SetTextColor, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_BTNFACE,
    DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, FW_NORMAL, HBITMAP, HBRUSH, HDC, HFONT,
    LOGPIXELSY, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyNameTextW, MapVirtualKeyW, MAPVK_VK_TO_VSC};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadCursorW, LoadIconW,
    GetClientRect, GetSystemMetrics, MoveWindow, PostMessageW, PostQuitMessage, RegisterClassExW,
    SendMessageW, SetForegroundWindow, SetWindowTextW, ShowWindow, TrackPopupMenuEx,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, HICON, HMENU, IDC_ARROW, IDI_APPLICATION, MF_STRING,
    MINMAXINFO, MSG, SC_MINIMIZE, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, SW_SHOWNORMAL,
    TPM_BOTTOMALIGN, TPM_RIGHTALIGN, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_GETMINMAXINFO, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
    WM_NULL, WM_RBUTTONUP, WM_SETFONT, WM_SIZE, WM_SYSCOMMAND, WNDCLASSEXW, WS_CAPTION, WS_CHILD,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME,
    WS_VISIBLE,
};

use super::{set_enabled, utf16};

const WM_TRAY: u32 = 0x8000 + 1;
const SS_NOTIFY: u32 = 256;
const IDC_TOGGLE: u16 = 1;
const IDC_EXIT: u16 = 2;
const IDC_LINK: u16 = 3;
const CLASS: windows::core::PCWSTR = w!("ZyabraWindow");
const TITLE: &str = concat!("Зябра ", env!("CARGO_PKG_VERSION"));
const REPO: &str = "https://github.com/DmNep/zyabra";

pub(super) fn run_window() -> Result<(), String> {
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwareness(
            windows::Win32::UI::HiDpi::PROCESS_SYSTEM_DPI_AWARE,
        );
        if focus_existing() {
            return Ok(());
        }
        let instance = GetModuleHandleW(None).map_err(|err| err.to_string())?;
        let instance = windows::Win32::Foundation::HINSTANCE(instance.0);
        let icon = load_icon(instance);
        super::ui().icon = icon;

        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW).map_err(|err| err.to_string())?,
            hbrBackground: HBRUSH((COLOR_BTNFACE.0 as isize + 1) as *mut core::ffi::c_void),
            lpszClassName: CLASS,
            hIconSm: icon,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err("не удалось зарегистрировать окно".to_string());
        }

        let title = utf16(TITLE);
        let width = dip(440);
        let height = dip(304);
        let x = (GetSystemMetrics(SM_CXSCREEN) - width) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - height) / 2;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS,
            windows::core::PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_THICKFRAME,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )
        .map_err(|err| err.to_string())?;
        super::ui().window = hwnd;
        add_tray(hwnd, icon)?;
        super::hook::install()?;

        let mut msg = MSG::default();
        loop {
            let status = GetMessageW(&mut msg, None, 0, 0);
            if !status.as_bool() {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn focus_existing() -> bool {
    unsafe {
        match windows::Win32::UI::WindowsAndMessaging::FindWindowW(CLASS, None) {
            Ok(hwnd) if !hwnd.0.is_null() => {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
                true
            }
            _ => false,
        }
    }
}

pub(super) fn refresh_status() {
    let enabled = super::is_enabled();
    let (status_hwnd, toggle_hwnd) = {
        let ui = super::ui();
        (ui.status, ui.toggle)
    };
    if status_hwnd.0.is_null() {
        return;
    }
    let status = if enabled {
        "Включена. Набранное никуда не сохраняется."
    } else {
        "На паузе. Клавиши не обрабатываются."
    };
    let button = if enabled { "Пауза" } else { "Продолжить" };
    let status = utf16(status);
    let button = utf16(button);
    unsafe {
        let _ = SetWindowTextW(status_hwnd, windows::core::PCWSTR(status.as_ptr()));
        let _ = SetWindowTextW(toggle_hwnd, windows::core::PCWSTR(button.as_ptr()));
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_children(hwnd);
            LRESULT(0)
        }
        WM_SIZE => {
            layout(hwnd);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam.0 as *mut MINMAXINFO);
            info.ptMinTrackSize.x = dip(400);
            info.ptMinTrackSize.y = dip(280);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let child = HWND(lparam.0 as *mut core::ffi::c_void);
            let link = super::ui().link;
            let dc = HDC(wparam.0 as *mut core::ffi::c_void);
            let _ = SetBkMode(dc, TRANSPARENT);
            if !link.0.is_null() && child == link {
                let _ = SetTextColor(dc, COLORREF(0x00C0_6000));
            }
            LRESULT((COLOR_BTNFACE.0 as isize) + 1)
        }
        WM_COMMAND => {
            match (wparam.0 & 0xFFFF) as u16 {
                IDC_TOGGLE => {
                    set_enabled(!super::is_enabled());
                }
                IDC_EXIT => {
                    let _ = DestroyWindow(hwnd);
                }
                IDC_LINK => open_repo(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_TRAY => {
            match lparam.0 as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = SetForegroundWindow(hwnd);
                }
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_SYSCOMMAND => {
            if (wparam.0 & 0xFFF0) as u32 == SC_MINIMIZE {
                hide_to_tray(hwnd);
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_CLOSE => {
            hide_to_tray(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray(hwnd);
            super::hook::uninstall();
            let (font, link_font) = {
                let ui = super::ui();
                (ui.font, ui.link_font)
            };
            if !font.0.is_null() {
                let _ = DeleteObject(font.into());
            }
            if !link_font.0.is_null() {
                let _ = DeleteObject(link_font.into());
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn create_children(parent: HWND) {
    let text_style = WS_CHILD | WS_VISIBLE;
    let button_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP;
    unsafe {
        let intro_line = intro_text();
        let intro = static_text(
            parent,
            &intro_line,
            0,
            0,
            10,
            10,
            text_style | WINDOW_STYLE(0x0000_0080),
        );
        let explain = static_text(
            parent,
            "Если слово набрано не в той раскладке, Зябра стирает его и пишет заново в правильной, затем переключает раскладку.",
            0,
            0,
            10,
            10,
            text_style,
        );
        let safety_title = static_text(parent, "Безопасность:", 0, 0, 10, 10, text_style);
        let safety = static_text(
            parent,
            "В памяти только текущее слово и одна последняя замена. На диск набранное не пишется. В интернет программа не ходит: словарь уже внутри файла. Поля пароля не читаются. «Пауза» останавливает обработку.",
            0,
            0,
            10,
            10,
            text_style,
        );
        let status = static_text(
            parent,
            "Включена. Набранное никуда не сохраняется.",
            0,
            0,
            10,
            10,
            text_style,
        );
        let toggle = button(parent, "Пауза", 0, 0, 10, 10, IDC_TOGGLE, button_style);
        let quit = button(parent, "Выход", 0, 0, 10, 10, IDC_EXIT, button_style);
        let link = github_link(parent);
        let font = segoe(false);
        let link_font = segoe(true);
        let handles = [
            intro.unwrap_or(HWND(std::ptr::null_mut())),
            explain.unwrap_or(HWND(std::ptr::null_mut())),
            safety_title.unwrap_or(HWND(std::ptr::null_mut())),
            safety.unwrap_or(HWND(std::ptr::null_mut())),
            status.unwrap_or(HWND(std::ptr::null_mut())),
            toggle.unwrap_or(HWND(std::ptr::null_mut())),
            quit.unwrap_or(HWND(std::ptr::null_mut())),
        ];
        for hwnd in handles {
            use_font(hwnd, font);
        }
        let link = link.unwrap_or(HWND(std::ptr::null_mut()));
        use_font(link, link_font);
        let mut ui = super::ui();
        ui.intro = handles[0];
        ui.explain = handles[1];
        ui.safety_title = handles[2];
        ui.safety = handles[3];
        ui.status = handles[4];
        ui.toggle = handles[5];
        ui.quit = handles[6];
        ui.link = link;
        ui.font = font;
        ui.link_font = link_font;
    }
    layout(parent);
}

fn intro_text() -> String {
    match extra_key_name() {
        Some(name) => format!("Pause или {name} - исправить текущее слово."),
        None => "Pause - исправить текущее слово.".to_string(),
    }
}

fn extra_key_name() -> Option<String> {
    let vk = super::hook::extra_force_key()?;
    let name = virtual_key_name(vk);
    if name.eq_ignore_ascii_case("Pause") {
        None
    } else {
        Some(name)
    }
}

/// Windows names a key from its scan code. Print Screen and Pause are the two
/// codes `MapVirtualKey` does not describe: one comes back as Sys Req, the
/// other as Right Ctrl.
fn virtual_key_name(vk: u32) -> String {
    let (scan, extended) = match vk {
        0x2C => (0x37, true),
        0x13 => (0x45, false),
        code => {
            let scan = unsafe { MapVirtualKeyW(code, MAPVK_VK_TO_VSC) } & 0xFF;
            if scan == 0 {
                return format!("0x{code:X}");
            }
            (scan, extended_key(code))
        }
    };
    let mut param = (scan as i32) << 16;
    if extended {
        param |= 1 << 24;
    }
    let mut buf = [0u16; 64];
    let written = unsafe { GetKeyNameTextW(param, &mut buf) };
    if written <= 0 {
        return format!("0x{vk:X}");
    }
    let name = String::from_utf16_lossy(&buf[..written as usize]);
    let name = name.trim();
    if name.is_empty() || name.starts_with('<') {
        format!("0x{vk:X}")
    } else {
        name.to_string()
    }
}

fn extended_key(vk: u32) -> bool {
    matches!(
        vk,
        0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2D | 0x2E | 0x5B | 0x5C
            | 0x5D | 0x6F | 0x90 | 0xA3 | 0xA5
    )
}

fn layout(parent: HWND) {
    let (handles, font) = {
        let ui = super::ui();
        (
            (
                ui.intro,
                ui.explain,
                ui.safety_title,
                ui.safety,
                ui.status,
                ui.toggle,
                ui.quit,
                ui.link,
            ),
            ui.font,
        )
    };
    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(parent, &mut rect).is_err() || rect.bottom < 80 {
            return;
        }
        let margin = dip(12);
        let width = (rect.right - margin * 2).max(dip(160));
        let line = dip(18);
        let mut y = margin;
        let intro = intro_text();
        let intro_h = if text_wider_than(&intro, font, width - dip(4)) {
            line * 2
        } else {
            line
        };
        place(handles.0, margin, y, width, intro_h);
        y += intro_h + dip(2);
        let explain_h = dip(34);
        place(handles.1, margin, y, width, explain_h);
        y += explain_h + dip(8);
        place(handles.2, margin, y, width, line);
        y += line;
        let safety_h = dip(68);
        place(handles.3, margin, y, width, safety_h);
        y += safety_h + dip(6);
        place(handles.4, margin, y, width, line);

        let btn_h = dip(23);
        let btn_y = rect.bottom - margin - btn_h;
        let quit_w = dip(76);
        let toggle_w = dip(108);
        let gap = dip(8);
        let quit_x = rect.right - margin - quit_w;
        let toggle_x = quit_x - gap - toggle_w;
        place(handles.5, toggle_x, btn_y, toggle_w, btn_h);
        place(handles.6, quit_x, btn_y, quit_w, btn_h);
        let link_w = (toggle_x - margin - gap).max(dip(40));
        place(handles.7, margin, btn_y + dip(2), link_w, line);
    }
}

unsafe fn place(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    if hwnd.0.is_null() {
        return;
    }
    let _ = MoveWindow(hwnd, x, y, w, h, true);
}

fn hide_to_tray(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

unsafe fn static_text(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: WINDOW_STYLE,
) -> Result<HWND, windows::core::Error> {
    let wide = utf16(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        windows::core::PCWSTR(wide.as_ptr()),
        style,
        x,
        y,
        w,
        h,
        Some(parent),
        None,
        None,
        None,
    )
}

unsafe fn github_link(parent: HWND) -> Result<HWND, windows::core::Error> {
    let wide = utf16(REPO);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        windows::core::PCWSTR(wide.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_NOTIFY),
        0,
        0,
        10,
        10,
        Some(parent),
        Some(HMENU(IDC_LINK as usize as *mut core::ffi::c_void)),
        None,
        None,
    )
}

fn text_wider_than(text: &str, font: HFONT, width: i32) -> bool {
    if font.0.is_null() || width <= 0 {
        return false;
    }
    unsafe {
        let dc = GetDC(None);
        if dc.0.is_null() {
            return false;
        }
        let previous = SelectObject(dc, font.into());
        let wide = utf16(text);
        let chars = &wide[..wide.len().saturating_sub(1)];
        let mut size = SIZE::default();
        let measured = GetTextExtentPoint32W(dc, chars, &mut size).as_bool();
        if !previous.is_invalid() {
            let _ = SelectObject(dc, previous);
        }
        let _ = ReleaseDC(None, dc);
        measured && size.cx > width
    }
}

fn dip(px: i32) -> i32 {
    px * system_dpi() / 96
}

fn system_dpi() -> i32 {
    unsafe {
        let dc = GetDC(None);
        if dc.0.is_null() {
            return 96;
        }
        let dpi = GetDeviceCaps(Some(dc), LOGPIXELSY);
        let _ = ReleaseDC(None, dc);
        if dpi <= 0 { 96 } else { dpi }
    }
}

fn segoe(underline: bool) -> HFONT {
    unsafe {
        CreateFontW(
            -(dip(12).max(11)),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            u32::from(underline),
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    }
}

fn use_font(hwnd: HWND, font: HFONT) {
    if hwnd.0.is_null() || font.0.is_null() {
        return;
    }
    unsafe {
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(0)),
        );
    }
}

fn load_icon(instance: windows::Win32::Foundation::HINSTANCE) -> HICON {
    unsafe {
        if let Ok(icon) = LoadIconW(Some(instance), windows::core::PCWSTR(1 as *const u16)) {
            if !icon.0.is_null() {
                return icon;
            }
        }
        make_icon()
    }
}

fn open_repo() {
    let wide = utf16(REPO);
    unsafe {
        let _ = ShellExecuteW(
            None,
            w!("open"),
            windows::core::PCWSTR(wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

unsafe fn button(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: u16,
    style: WINDOW_STYLE,
) -> Result<HWND, windows::core::Error> {
    let wide = utf16(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        windows::core::PCWSTR(wide.as_ptr()),
        style,
        x,
        y,
        w,
        h,
        Some(parent),
        Some(HMENU(id as usize as *mut core::ffi::c_void)),
        None,
        None,
    )
}

fn show_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let pause = utf16(if super::is_enabled() {
            "Пауза"
        } else {
            "Продолжить"
        });
        let exit = utf16("Выход");
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDC_TOGGLE as usize,
            windows::core::PCWSTR(pause.as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDC_EXIT as usize,
            windows::core::PCWSTR(exit.as_ptr()),
        );
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenuEx(
            menu,
            TPM_RIGHTALIGN.0 | TPM_BOTTOMALIGN.0,
            pt.x,
            pt.y,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

fn add_tray(hwnd: HWND, icon: HICON) -> Result<(), String> {
    let mut tip = [0u16; 128];
    for (i, unit) in concat!("Зябра ", env!("CARGO_PKG_VERSION"), " — переключатель раскладки")
        .encode_utf16()
        .take(127)
        .enumerate()
    {
        tip[i] = unit;
    }
    let mut data = NOTIFYICONDATAW::default();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = icon;
    data.szTip = tip;
    let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
    if !ok.as_bool() {
        return Err("не удалось показать значок".to_string());
    }
    Ok(())
}

fn remove_tray(hwnd: HWND) {
    let mut data = NOTIFYICONDATAW::default();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn make_icon() -> HICON {
    unsafe {
        if let Some(icon) = draw_icon() {
            return icon;
        }
        LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON(std::ptr::null_mut()))
    }
}

fn draw_icon() -> Option<HICON> {
    unsafe {
        let screen = GetDC(None);
        if screen.0.is_null() {
            return None;
        }
        let pixels = icon_pixels();
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            bmiColors: Default::default(),
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color =
            CreateDIBSection(Some(screen), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(color.into());
            let _ = ReleaseDC(None, screen);
            return None;
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, pixels.len());
        let mask = CreateBitmap(32, 32, 1, 1, None);
        if mask.0.is_null() {
            let _ = DeleteObject(color.into());
            let _ = ReleaseDC(None, screen);
            return None;
        }
        let icon = windows::Win32::UI::WindowsAndMessaging::CreateIconIndirect(
            &windows::Win32::UI::WindowsAndMessaging::ICONINFO {
                fIcon: windows::core::BOOL(1),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            },
        )
        .ok();
        let _ = DeleteObject(HBITMAP(color.0).into());
        let _ = DeleteObject(mask.into());
        let _ = ReleaseDC(None, screen);
        let _ = pixels;
        icon
    }
}

fn icon_pixels() -> [u32; 32 * 32] {
    let mut px = [0u32; 32 * 32];
    for y in 0..32 {
        for x in 0..32 {
            let dx = x as i32 - 15;
            let dy = y as i32 - 15;
            if dx * dx + dy * dy <= 196 {
                px[y * 32 + x] = 0xFF3E8F6E;
            }
        }
    }
    for x in 9..23 {
        px[11 * 32 + x] = 0xFFFFFFFF;
        px[20 * 32 + x] = 0xFFFFFFFF;
    }
    for y in 11..21 {
        px[y * 32 + 9] = 0xFFFFFFFF;
        px[y * 32 + 22] = 0xFFFFFFFF;
    }
    px
}
