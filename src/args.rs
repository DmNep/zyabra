//! Command line. The program stores nothing: a shortcut keeps the key code.

pub const USAGE: &str = "\
Зябра [--key КОД]
Зябра help

КОД — дополнительная клавиша перекладывания, рядом с Pause.
Десятичное число, шестнадцатеричное с префиксом 0x, или 2sh.
Windows: виртуальный код клавиши. Linux: код evdev.
2sh — второе нажатие Shift подряд, сама клавиша при этом не глотается.

Без аргумента работает только Pause. Зябра этот код никуда не записывает.
";

/// Help for `zyabra help` and the window button. `program` is the file to put
/// in a Windows shortcut's Target field.
pub fn help_text(program: &str) -> String {
    format!(
        "{USAGE}\n\
Ярлык Windows, поле \"Объект\":\n\
\"{program}\" --key 0x2C\n\
\"{program}\" --key 2sh\n\
\n\
Кавычки вокруг пути нужны, если в нём есть пробел.\n\
0x2C на Windows — Print Screen. На Linux та же клавиша — 99.\n\
2sh перекладывает слово со второго Shift подряд, не позже 0,4 с и без другой клавиши между ними. Удержание Shift не считается.\n\
Пока Зябра уже запущена, повторный запуск новый код не подхватывает.\n"
    )
}

/// Path of the running file, for the shortcut example.
pub fn installed_path() -> String {
    let shown = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "zyabra".to_string());
    shown
        .strip_prefix(r"\\?\")
        .unwrap_or(&shown)
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Force {
    Key(u32),
    DoubleShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    Run(Option<Force>),
    Help,
}

pub fn parse_args<I, S>(args: I) -> Result<Launch, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut force_key = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        if arg == "--help" || arg == "-h" || arg.eq_ignore_ascii_case("help") {
            return Ok(Launch::Help);
        }
        let inline = arg.strip_prefix("--key=");
        if arg == "--key" || inline.is_some() {
            if force_key.is_some() {
                return Err(format!("клавиша указана дважды\n\n{USAGE}"));
            }
            let value = if let Some(inline) = inline {
                inline.to_owned()
            } else {
                args.next()
                    .ok_or_else(|| format!("после --key нужен код клавиши\n\n{USAGE}"))?
                    .as_ref()
                    .to_owned()
            };
            force_key = Some(parse_force(&value)?);
            continue;
        }
        return Err(format!("неизвестный аргумент {arg}\n\n{USAGE}"));
    }
    Ok(Launch::Run(force_key))
}

fn parse_force(text: &str) -> Result<Force, String> {
    if text.eq_ignore_ascii_case("2sh") {
        return Ok(Force::DoubleShift);
    }
    Ok(Force::Key(parse_code(text)?))
}

fn parse_code(text: &str) -> Result<u32, String> {
    let (digits, radix) = if let Some(hex) = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
    {
        (hex, 16)
    } else {
        (text, 10)
    };
    if digits.is_empty() {
        return Err(format!("пустой код клавиши\n\n{USAGE}"));
    }
    let code = u32::from_str_radix(digits, radix)
        .map_err(|_| format!("непонятный код клавиши {text}\n\n{USAGE}"))?;
    if code == 0 || code > 0xFFFF {
        return Err(format!("код клавиши вне диапазона 1…FFFF\n\n{USAGE}"));
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_decimal_or_hex_key() {
        assert_eq!(parse_args(std::iter::empty::<&str>()), Ok(Launch::Run(None)));
        assert_eq!(
            parse_args(["--key", "44"]),
            Ok(Launch::Run(Some(Force::Key(0x2C))))
        );
        assert_eq!(
            parse_args(["--key", "0x2C"]),
            Ok(Launch::Run(Some(Force::Key(44))))
        );
        assert_eq!(
            parse_args(["--key=0x2c"]),
            Ok(Launch::Run(Some(Force::Key(44))))
        );
        assert_eq!(
            parse_args(["--key", "2sh"]),
            Ok(Launch::Run(Some(Force::DoubleShift)))
        );
        assert_eq!(
            parse_args(["--key=2SH"]),
            Ok(Launch::Run(Some(Force::DoubleShift)))
        );
        assert_eq!(parse_args(["--help"]), Ok(Launch::Help));
        assert_eq!(parse_args(["-h"]), Ok(Launch::Help));
        assert_eq!(parse_args(["help"]), Ok(Launch::Help));
        assert_eq!(parse_args(["Help"]), Ok(Launch::Help));
    }

    #[test]
    fn help_shows_the_shortcut_target() {
        let text = help_text(r"C:\zyabra\zyabra.exe");
        assert!(text.contains("Зябра help"));
        assert!(text.contains("Объект"));
        assert!(text.contains("\"C:\\zyabra\\zyabra.exe\" --key 0x2C"));
        assert!(text.contains("\"C:\\zyabra\\zyabra.exe\" --key 2sh"));
        assert!(parse_args(["--key", "help"]).is_err());
    }

    #[test]
    fn rejects_a_bad_key() {
        assert!(parse_args(["--key"]).is_err());
        assert!(parse_args(["--key", "0"]).is_err());
        assert!(parse_args(["--key", "xyz"]).is_err());
        assert!(parse_args(["--key", "0x2C", "--key", "1"]).is_err());
        assert!(parse_args(["--pause"]).is_err());
    }
}
