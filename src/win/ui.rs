use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, COLOR_WINDOW, DIB_RGB_COLORS, HBITMAP, HBRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadCursorW, LoadIconW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetWindowTextW,
    ShowWindow, TrackPopupMenuEx, TranslateMessage, CS_HREDRAW, CS_VREDRAW, HICON, HMENU,
    IDC_ARROW, IDI_APPLICATION, MF_STRING, MSG, SW_RESTORE, TPM_BOTTOMALIGN, TPM_RIGHTALIGN,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_LBUTTONUP,
    WM_NULL, WM_RBUTTONUP, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_SYSMENU, WS_VISIBLE,
};

use super::{set_enabled, utf16};

const WM_TRAY: u32 = 0x8000 + 1;
const IDC_TOGGLE: u16 = 1;
const IDC_EXIT: u16 = 2;
const CLASS: windows::core::PCWSTR = w!("ZyabraWindow");

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

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS,
            w!("Зябра"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            120,
            120,
            520,
            360,
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
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
                true
            }
            _ => false,
        }
    }
}

pub(super) fn refresh_status() {
    let enabled = super::is_enabled();
    let ui = super::ui();
    if ui.status.0.is_null() {
        return;
    }
    let status = if enabled {
        "Включена. Набранное никуда не сохраняется."
    } else {
        "На паузе. Клавиши не обрабатываются."
    };
    let button = if enabled {
        "Пауза"
    } else {
        "Продолжить"
    };
    let status = utf16(status);
    let button = utf16(button);
    unsafe {
        let _ = SetWindowTextW(ui.status, windows::core::PCWSTR(status.as_ptr()));
        let _ = SetWindowTextW(ui.toggle, windows::core::PCWSTR(button.as_ptr()));
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
        WM_COMMAND => {
            match (wparam.0 & 0xFFFF) as u16 {
                IDC_TOGGLE => {
                    set_enabled(!super::is_enabled());
                }
                IDC_EXIT => {
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TRAY => {
            match lparam.0 as u32 {
                WM_LBUTTONUP => {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                }
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
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
        let _ = static_text(
            parent,
            "Если слово набрано не в той раскладке, Зябра стирает его и пишет заново в правильной, затем переключает раскладку.",
            24,
            20,
            460,
            72,
            text_style,
        );
        let _ = static_text(
            parent,
            "В памяти только текущее слово и одна последняя замена. На диск текст не пишется. В интернет программа не ходит. Поля пароля не читаются.",
            24,
            96,
            460,
            72,
            text_style,
        );
        let _ = static_text(
            parent,
            "Pause — исправить текущее слово или отменить последнюю замену.",
            24,
            172,
            460,
            32,
            text_style,
        );
        let status = static_text(
            parent,
            "Включена. Набранное никуда не сохраняется.",
            24,
            214,
            460,
            28,
            text_style,
        );
        let toggle = button(parent, "Пауза", 24, 252, 140, 32, IDC_TOGGLE, button_style);
        let _ = button(parent, "Выход", 176, 252, 140, 32, IDC_EXIT, button_style);
        let mut ui = super::ui();
        ui.status = status.unwrap_or(HWND(std::ptr::null_mut()));
        ui.toggle = toggle.unwrap_or(HWND(std::ptr::null_mut()));
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
    for (i, unit) in "Зябра — переключатель раскладки"
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
