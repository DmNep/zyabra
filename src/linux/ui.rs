//! Small X11 window. The compositor draws the title; the body is a core font
//! so the binary does not need GTK, Cairo, or a bundled typeface.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const WIDTH: u16 = 560;
const HEIGHT: u16 = 320;

const LINES_RU: &[&str] = &[
    "Pause - исправить текущее слово.",
    "Если слово набрано не в той раскладке, Зябра стирает его и пишет",
    "заново в правильной, затем переключает раскладку.",
    "",
    "Безопасность:",
    "В памяти только текущее слово и одна последняя замена. На диск",
    "набранное не пишется. В интернет программа не ходит: словарь уже",
    "внутри файла. Поля пароля на Linux не отличить, клавиши читаются.",
    "«Пауза» останавливает обработку. Крестик завершает программу.",
];

const LINES_EN: &[&str] = &[
    "Pause - fix the current word.",
    "A word typed in the wrong layout is deleted and typed again.",
    "",
    "Safety:",
    "Only the current word and one last replacement stay in memory.",
    "Nothing is written to disk. The program does not open a network",
    "connection. Linux cannot tell password fields apart, so those",
    "keys are read too. Pause stops processing. Close quits.",
];

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        UTF8_STRING,
        _NET_ACTIVE_WINDOW,
    }
}

pub enum UiEvent {
    Quit,
    Toggle,
    Link,
}

pub struct Display {
    conn: RustConnection,
    atoms: Atoms,
    root: Window,
    window: Window,
    gc: Gcontext,
    ascii: bool,
    width: u16,
    height: u16,
    text: u32,
    button: u32,
    link: u32,
    bg: u32,
}

pub fn open() -> Result<Option<Display>, String> {
    if std::env::var_os("DISPLAY").is_none() {
        return Ok(None);
    }
    Ok(Some(Display::connect()?))
}

pub fn raise_existing() -> bool {
    if std::env::var_os("DISPLAY").is_none() {
        return false;
    }
    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return false;
    };
    let root = conn.setup().roots[screen_num].root;
    let Some(window) = find_zyabra(&conn, root) else {
        return false;
    };
    let _ = conn.map_window(window);
    let _ = conn.configure_window(
        window,
        &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
    );
    if let Ok(cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") {
        if let Ok(atom) = cookie.reply() {
            let event = ClientMessageEvent::new(32, window, atom.atom, [1u32, 0, 0, 0, 0]);
            let _ = conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
        }
    }
    let _ = conn.flush();
    true
}

impl Display {
    pub fn conn(&self) -> &RustConnection {
        &self.conn
    }

    pub fn root(&self) -> Window {
        self.root
    }

    pub fn focused(&self) -> Result<bool, String> {
        let reply = self
            .conn
            .get_input_focus()
            .map_err(xerr)?
            .reply()
            .map_err(xerr)?;
        Ok(reply.focus == self.window)
    }

    pub fn poll(&mut self, enabled: bool) -> Result<Option<UiEvent>, String> {
        let mut chosen = None;
        while let Some(event) = self.conn.poll_for_event().map_err(xerr)? {
            match self.on_event(event, enabled)? {
                Some(UiEvent::Quit) => return Ok(Some(UiEvent::Quit)),
                other @ Some(_) => chosen = other,
                None => {}
            }
        }
        Ok(chosen)
    }

    pub fn redraw(&mut self, enabled: bool) -> Result<(), String> {
        self.paint(enabled, true)
    }

    fn connect() -> Result<Self, String> {
        let (conn, screen_num) = x11rb::connect(None).map_err(xerr)?;
        let screen = &conn.setup().roots[screen_num];
        let atoms = Atoms::new(&conn).map_err(xerr)?.reply().map_err(xerr)?;
        let window = conn.generate_id().map_err(xerr)?;
        let gc = conn.generate_id().map_err(xerr)?;
        let root = screen.root;
        let depth = screen.root_depth;
        let colormap = screen.default_colormap;
        let text = screen.black_pixel;
        let x = (screen.width_in_pixels.saturating_sub(WIDTH) / 2) as i16;
        let y = (screen.height_in_pixels.saturating_sub(HEIGHT) / 2) as i16;
        let bg = color(&conn, colormap, 0xE6E6, 0xE6E6, 0xE6E6)?;
        let button = color(&conn, colormap, 0xF4F4, 0xF4F4, 0xF4F4)?;
        let link = color(&conn, colormap, 0x0000, 0x4A4A, 0xA8A8)?;
        conn.create_window(
            depth,
            window,
            root,
            x,
            y,
            WIDTH,
            HEIGHT,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new()
                .background_pixel(bg)
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::BUTTON_PRESS
                        | EventMask::STRUCTURE_NOTIFY,
                ),
        )
        .map_err(xerr)?;
        let title = format!("Зябра {}", env!("CARGO_PKG_VERSION"));
        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )
        .map_err(xerr)?;
        conn.change_property8(
            PropMode::REPLACE,
            window,
            atoms._NET_WM_NAME,
            atoms.UTF8_STRING,
            title.as_bytes(),
        )
        .map_err(xerr)?;
        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"zyabra\0Zyabra\0",
        )
        .map_err(xerr)?;
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[atoms.WM_DELETE_WINDOW],
        )
        .map_err(xerr)?;
        let ascii = !open_font(&conn, gc, window, true, true)?;
        if ascii && !open_font(&conn, gc, window, false, true)? {
            return Err("в X11 нет шрифта fixed".into());
        }
        conn.change_gc(gc, &ChangeGCAux::new().foreground(text))
            .map_err(xerr)?;
        conn.map_window(window).map_err(xerr)?;
        conn.flush().map_err(xerr)?;
        let mut display = Self {
            conn,
            atoms,
            root,
            window,
            gc,
            ascii,
            width: WIDTH,
            height: HEIGHT,
            text,
            button,
            link,
            bg,
        };
        display.redraw(true)?;
        Ok(display)
    }

    fn on_event(&mut self, event: Event, enabled: bool) -> Result<Option<UiEvent>, String> {
        match event {
            Event::Expose(expose) if expose.count == 0 => {
                self.redraw(enabled)?;
            }
            Event::ConfigureNotify(configure) if configure.window == self.window => {
                self.width = configure.width;
                self.height = configure.height;
                self.redraw(enabled)?;
            }
            Event::ButtonPress(press) if press.event == self.window => {
                return Ok(self.hit(press.event_x, press.event_y));
            }
            Event::ClientMessage(message)
                if message.format == 32
                    && message.window == self.window
                    && message.data.as_data32()[0] == self.atoms.WM_DELETE_WINDOW =>
            {
                return Ok(Some(UiEvent::Quit));
            }
            Event::Error(_) => {}
            _ => {}
        }
        Ok(None)
    }

    fn hit(&self, x: i16, y: i16) -> Option<UiEvent> {
        for area in self.areas() {
            let inside = x >= area.x
                && y >= area.y
                && x < area.x + area.w as i16
                && y < area.y + area.h as i16;
            if inside {
                return Some(area.event);
            }
        }
        None
    }

    fn paint(&mut self, enabled: bool, fallback: bool) -> Result<(), String> {
        self.conn
            .clear_area(false, self.window, 0, 0, 0, 0)
            .map_err(xerr)?;
        self.conn
            .change_gc(
                self.gc,
                &ChangeGCAux::new()
                    .foreground(self.text)
                    .background(self.bg),
            )
            .map_err(xerr)?;
        let lines = if self.ascii { LINES_EN } else { LINES_RU };
        for (index, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let y = 28 + (index as i16) * 18;
            if self.ascii {
                self.conn
                    .image_text8(self.window, self.gc, 16, y, line.as_bytes())
                    .map_err(xerr)?;
            } else if index == 0 && fallback {
                if self
                    .conn
                    .image_text16(self.window, self.gc, 16, y, &xchars(line))
                    .map_err(xerr)?
                    .check()
                    .is_err()
                {
                    self.ascii = true;
                    if !open_font(&self.conn, self.gc, self.window, false, false)? {
                        return Err("в X11 нет шрифта fixed".into());
                    }
                    return self.paint(enabled, false);
                }
            } else {
                self.conn
                    .image_text16(self.window, self.gc, 16, y, &xchars(line))
                    .map_err(xerr)?;
            }
        }
        let status_y = self.height as i16 - 72;
        let status = status_line(self.ascii, enabled);
        if self.ascii {
            self.conn
                .image_text8(self.window, self.gc, 16, status_y, status.as_bytes())
                .map_err(xerr)?;
        } else {
            self.conn
                .image_text16(self.window, self.gc, 16, status_y, &xchars(status))
                .map_err(xerr)?;
        }
        for area in self.areas() {
            if area.fill {
                self.conn
                    .change_gc(self.gc, &ChangeGCAux::new().foreground(self.button))
                    .map_err(xerr)?;
                self.conn
                    .poly_fill_rectangle(
                        self.window,
                        self.gc,
                        &[Rectangle {
                            x: area.x,
                            y: area.y,
                            width: area.w,
                            height: area.h,
                        }],
                    )
                    .map_err(xerr)?;
                self.conn
                    .change_gc(
                        self.gc,
                        &ChangeGCAux::new()
                            .foreground(self.text)
                            .background(self.button),
                    )
                    .map_err(xerr)?;
                self.conn
                    .poly_rectangle(
                        self.window,
                        self.gc,
                        &[Rectangle {
                            x: area.x,
                            y: area.y,
                            width: area.w,
                            height: area.h,
                        }],
                    )
                    .map_err(xerr)?;
            } else {
                self.conn
                    .change_gc(
                        self.gc,
                        &ChangeGCAux::new()
                            .foreground(self.link)
                            .background(self.bg),
                    )
                    .map_err(xerr)?;
            }
            let label = area.label(self.ascii, enabled);
            let text_y = if area.fill { area.y + 18 } else { area.y + 14 };
            if self.ascii {
                self.conn
                    .image_text8(
                        self.window,
                        self.gc,
                        area.x + 12,
                        text_y,
                        label.as_bytes(),
                    )
                    .map_err(xerr)?;
            } else {
                self.conn
                    .image_text16(self.window, self.gc, area.x + 12, text_y, &xchars(label))
                    .map_err(xerr)?;
            }
        }
        self.conn.flush().map_err(xerr)?;
        Ok(())
    }

    fn areas(&self) -> [Area; 3] {
        let y = self.height as i16 - 40;
        let exit_x = self.width as i16 - 16 - 76;
        let toggle_x = exit_x - 8 - 132;
        [
            Area {
                x: toggle_x,
                y,
                w: 132,
                h: 26,
                fill: true,
                event: UiEvent::Toggle,
                kind: AreaKind::Toggle,
            },
            Area {
                x: exit_x,
                y,
                w: 76,
                h: 26,
                fill: true,
                event: UiEvent::Quit,
                kind: AreaKind::Exit,
            },
            Area {
                x: 16,
                y: self.height as i16 - 24,
                w: 230,
                h: 18,
                fill: false,
                event: UiEvent::Link,
                kind: AreaKind::Link,
            },
        ]
    }
}

struct Area {
    x: i16,
    y: i16,
    w: u16,
    h: u16,
    fill: bool,
    event: UiEvent,
    kind: AreaKind,
}

enum AreaKind {
    Toggle,
    Exit,
    Link,
}

impl Area {
    fn label(&self, ascii: bool, enabled: bool) -> &'static str {
        match self.kind {
            AreaKind::Toggle if ascii && enabled => "Pause",
            AreaKind::Toggle if ascii => "Resume",
            AreaKind::Toggle if enabled => "Пауза",
            AreaKind::Toggle => "Продолжить",
            AreaKind::Exit if ascii => "Exit",
            AreaKind::Exit => "Выход",
            AreaKind::Link => "github.com/DmNep/zyabra",
        }
    }
}

fn status_line(ascii: bool, enabled: bool) -> &'static str {
    match (ascii, enabled) {
        (false, true) => "Следит за раскладкой.",
        (false, false) => "На паузе. Клавиши не трогает.",
        (true, true) => "Watching the layout.",
        (true, false) => "Paused. Keys are left alone.",
    }
}

fn open_font(
    conn: &RustConnection,
    gc: Gcontext,
    window: Window,
    utf: bool,
    fresh: bool,
) -> Result<bool, String> {
    let names: &[&[u8]] = if utf {
        &[
            b"-misc-fixed-medium-r-normal--14-130-75-75-c-70-iso10646-1",
            b"-misc-fixed-medium-r-normal--13-120-75-75-c-80-iso10646-1",
            b"-*-fixed-medium-r-*-*-14-*-*-*-*-*-iso10646-1",
        ]
    } else {
        &[b"fixed"]
    };
    for name in names {
        let font = conn.generate_id().map_err(xerr)?;
        if conn.open_font(font, name).map_err(xerr)?.check().is_err() {
            continue;
        }
        if fresh {
            conn.create_gc(gc, window, &CreateGCAux::new().font(font))
                .map_err(xerr)?
                .check()
                .map_err(xerr)?;
        } else {
            conn.change_gc(gc, &ChangeGCAux::new().font(font))
                .map_err(xerr)?
                .check()
                .map_err(xerr)?;
        }
        return Ok(true);
    }
    Ok(false)
}

fn color(conn: &RustConnection, colormap: Colormap, red: u16, green: u16, blue: u16) -> Result<u32, String> {
    Ok(conn
        .alloc_color(colormap, red, green, blue)
        .map_err(xerr)?
        .reply()
        .map_err(xerr)?
        .pixel)
}

fn xchars(text: &str) -> Vec<Char2b> {
    text.encode_utf16()
        .map(|unit| Char2b {
            byte1: (unit >> 8) as u8,
            byte2: unit as u8,
        })
        .collect()
}

fn find_zyabra(conn: &RustConnection, root: Window) -> Option<Window> {
    let mut queue = vec![(root, 0u8)];
    let mut seen = 0;
    while let Some((window, depth)) = queue.pop() {
        seen += 1;
        if seen > 2000 {
            break;
        }
        if depth > 0 && is_zyabra(conn, window) {
            return Some(window);
        }
        if depth >= 3 {
            continue;
        }
        let Ok(cookie) = conn.query_tree(window) else {
            continue;
        };
        let Ok(tree) = cookie.reply() else {
            continue;
        };
        for child in tree.children {
            queue.push((child, depth + 1));
        }
    }
    None
}

fn is_zyabra(conn: &RustConnection, window: Window) -> bool {
    let Ok(cookie) = conn.get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 64)
    else {
        return false;
    };
    let Ok(prop) = cookie.reply() else {
        return false;
    };
    prop.value
        .split(|byte| *byte == 0)
        .any(|part| part == b"zyabra")
}

fn xerr(err: impl std::fmt::Display) -> String {
    format!("ошибка X11: {err}")
}
