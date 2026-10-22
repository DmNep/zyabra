//! Command line. The program stores nothing: a shortcut keeps the key code.

pub const USAGE: &str = "\
Зябра [--key КОД]

КОД — дополнительная клавиша перекладывания, рядом с Pause.
Десятичное число или шестнадцатеричное с префиксом 0x.
Windows: виртуальный код клавиши. Linux: код evdev.
Пример: zyabra --key 0x2C

Без аргумента работает только Pause. Зябра этот код никуда не записывает.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    Run(Option<u32>),
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
        if arg == "--help" || arg == "-h" {
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
            force_key = Some(parse_code(&value)?);
            continue;
        }
        return Err(format!("неизвестный аргумент {arg}\n\n{USAGE}"));
    }
    Ok(Launch::Run(force_key))
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
            Ok(Launch::Run(Some(0x2C)))
        );
        assert_eq!(parse_args(["--key", "0x2C"]), Ok(Launch::Run(Some(44))));
        assert_eq!(parse_args(["--key=0x2c"]), Ok(Launch::Run(Some(44))));
        assert_eq!(parse_args(["--help"]), Ok(Launch::Help));
        assert_eq!(parse_args(["-h"]), Ok(Launch::Help));
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
