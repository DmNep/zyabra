use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, CreateFontW, DeleteObject, GetDC, ReleaseDC, SetBkMode,
    SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    COLOR_WINDOW, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, FW_NORMAL, HBITMAP, HBRUSH,
    HDC, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadCursorW, LoadIconW,
    GetClientRect, MoveWindow, PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW,
    SetForegroundWindow, SetWindowTextW, ShowWindow, TrackPopupMenuEx, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, HICON, HMENU, IDC_ARROW, IDI_APPLICATION, MF_STRING, MINMAXINFO, MSG,
    SC_MINIMIZE, SW_HIDE, SW_SHOW, SW_SHOWNORMAL, TPM_BOTTOMALIGN, TPM_RIGHTALIGN,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY,
    WM_GETMINMAXINFO, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_SETFONT, WM_SIZE,
    WM_SYSCOMMAND,
    WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
    WS_THICKFRAME, WS_VISIBLE,
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
        let icon = make_icon();
        super::ui().icon = icon;

        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW).map_err(|err| err.to_string())?,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 as isize + 1) as *mut core::ffi::c_void),
            lpszClassName: CLASS,
            hIconSm: icon,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err("не удалось зарегистрировать окно".to_string());
        }

        let title = utf16(TITLE);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS,
            windows::core::PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_THICKFRAME,
            120,
            80,
            540,
            560,
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
            info.ptMinTrackSize.x = 460;
            info.ptMinTrackSize.y = 420;
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let child = HWND(lparam.0 as *mut core::ffi::c_void);
            let link = super::ui().link;
            if !link.0.is_null() && child == link {
                let dc = HDC(wparam.0 as *mut core::ffi::c_void);
                let _ = SetTextColor(dc, COLORREF(0x00C0_6000));
                let _ = SetBkMode(dc, TRANSPARENT);
                LRESULT((COLOR_WINDOW.0 as isize) + 1)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
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
            let icon = super::ui().icon;
            if !icon.0.is_null() {
                let _ = DestroyIcon(icon);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn create_children(parent: HWND) {
    let text_style = WS_CHILD | WS_VISIBLE;
    let button_style = WS_CHILD | WS_VISIBLE;
    unsafe {
        let intro = static_text(parent, "Pause - исправить текущее слово.", 24, 16, 480, 28, text_style);
        let explain = static_text(
            parent,
            "Если слово набрано не в той раскладке, Зябра стирает его и пишет заново в правильной, затем переключает раскладку.",
            24,
            48,
            480,
            64,
            text_style,
        );
        let safety_title = static_text(parent, "Безопасность:", 24, 120, 480, 24, text_style);
        let safety = static_text(
            parent,
            "В памяти только текущее слово и одна последняя замена. На диск набранное не пишется. В интернет программа не ходит: словарь уже внутри файла. Поля пароля не читаются. «Пауза» останавливает обработку.",
            24,
            148,
            480,
            96,
            text_style,
        );
        let status = static_text(
            parent,
            "Включена. Набранное никуда не сохраняется.",
            24,
            256,
            480,
            28,
            text_style,
        );
        let toggle = button(parent, "Пауза", 24, 296, 140, 32, IDC_TOGGLE, button_style);
        let quit = button(parent, "Выход", 176, 296, 140, 32, IDC_EXIT, button_style);
        let link = github_link(parent);
        let mut ui = super::ui();
        ui.intro = intro.unwrap_or(HWND(std::ptr::null_mut()));
        ui.explain = explain.unwrap_or(HWND(std::ptr::null_mut()));
        ui.safety_title = safety_title.unwrap_or(HWND(std::ptr::null_mut()));
        ui.safety = safety.unwrap_or(HWND(std::ptr::null_mut()));
        ui.status = status.unwrap_or(HWND(std::ptr::null_mut()));
        ui.toggle = toggle.unwrap_or(HWND(std::ptr::null_mut()));
        ui.quit = quit.unwrap_or(HWND(std::ptr::null_mut()));
        ui.link = link.unwrap_or(HWND(std::ptr::null_mut()));
    }
    layout(parent);
}

fn layout(parent: HWND) {
    let handles = {
        let ui = super::ui();
        (
            ui.intro,
            ui.explain,
            ui.safety_title,
            ui.safety,
            ui.status,
            ui.toggle,
            ui.quit,
            ui.link,
        )
    };
    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(parent, &mut rect).is_err() || rect.bottom < 80 {
            return;
        }
        let width = (rect.right - 48).max(120);
        let link_y = rect.bottom - 36;
        let buttons_y = link_y - 48;
        let status_y = buttons_y - 36;
        place(handles.0, 24, 16, width, 28);
        place(handles.1, 24, 48, width, 64);
        place(handles.2, 24, 120, width, 24);
        place(handles.3, 24, 148, width, 96);
        place(handles.4, 24, status_y, width, 28);
        place(handles.5, 24, buttons_y, 140, 32);
        place(handles.6, 176, buttons_y, 140, 32);
        place(handles.7, 24, link_y, width, 24);
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
    let mut rect = RECT::default();
    let _ = GetClientRect(parent, &mut rect);
    let y = (rect.bottom - 40).max(0);
    let wide = utf16(REPO);
    let link = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        windows::core::PCWSTR(wide.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_NOTIFY),
        24,
        y,
        (rect.right - 48).max(200),
        24,
        Some(parent),
        Some(HMENU(IDC_LINK as usize as *mut core::ffi::c_void)),
        None,
        None,
    )?;
    let font = CreateFontW(
        -16,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        1,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    );
    if !font.0.is_null() {
        let _ = SendMessageW(
            link,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
    Ok(link)
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
