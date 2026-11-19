//! Key names to the byte sequences a terminal expects, matching the Android client's `TerminalInput`.

/// Shift+Enter has no legacy encoding. The engine turns this kitty sequence into a real
/// Shift+Enter key event, which Rex encodes for whatever keyboard mode the program enabled.
pub const SHIFT_ENTER: &str = "\x1b[13;2u";

fn sequence(name: &str) -> Option<&'static str> {
    Some(match name {
        "esc" => "\x1b",
        "tab" => "\t",
        "enter" => "\r",
        "backspace" => "\x7f",
        "pageup" => "\x1b[5~",
        "pagedown" => "\x1b[6~",
        "up" => "\x1b[A",
        "down" => "\x1b[B",
        "right" => "\x1b[C",
        "left" => "\x1b[D",
        "insert" => "\x1b[2~",
        "delete" => "\x1b[3~",
        "home" => "\x1b[H",
        "end" => "\x1b[F",
        "f1" => "\x1bOP",
        "f2" => "\x1bOQ",
        "f3" => "\x1bOR",
        "f4" => "\x1bOS",
        "f5" => "\x1b[15~",
        "f6" => "\x1b[17~",
        "f7" => "\x1b[18~",
        "f8" => "\x1b[19~",
        "f9" => "\x1b[20~",
        "f10" => "\x1b[21~",
        "f11" => "\x1b[23~",
        "f12" => "\x1b[24~",
        _ => return None,
    })
}

pub fn key(name: &str, control: bool, alt: bool, shift: bool) -> String {
    let Some(sequence) = sequence(name) else { return String::new() };
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(control);
    if modifier == 1 {
        return sequence.into();
    }
    if name == "enter" && modifier == 2 {
        return SHIFT_ENTER.into();
    }
    if name == "tab" && shift {
        return format!("{}\x1b[Z", if alt { "\x1b" } else { "" });
    }
    if let Some(body) = sequence.strip_prefix("\x1b[").or_else(|| sequence.strip_prefix("\x1bO")) {
        return match body.strip_suffix('~') {
            Some(number) => format!("\x1b[{number};{modifier}~"),
            None => format!("\x1b[1;{modifier}{body}"),
        };
    }
    let input = if control && name == "backspace" { "\x08" } else { sequence };
    format!("{}{input}", if alt { "\x1b" } else { "" })
}

/// A printable character typed with Ctrl and/or Alt held.
pub fn modified(character: char, control: bool, alt: bool) -> String {
    let mut out = String::new();
    if alt {
        out.push('\x1b');
    }
    let upper = character.to_ascii_uppercase();
    if control && character == ' ' {
        out.push('\0');
    } else if control && ('@'..='_').contains(&upper) {
        out.push((upper as u8 & 0x1f) as char);
    } else if control && character == '?' {
        out.push('\x7f');
    } else {
        out.push(character);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_modified_keys() {
        assert_eq!(key("up", false, false, false), "\x1b[A");
        assert_eq!(key("up", true, false, false), "\x1b[1;5A");
        assert_eq!(key("delete", false, false, true), "\x1b[3;2~");
        assert_eq!(key("f1", false, true, false), "\x1b[1;3P");
        assert_eq!(key("enter", false, false, true), SHIFT_ENTER);
        assert_eq!(key("tab", false, false, true), "\x1b[Z");
        assert_eq!(key("backspace", true, false, false), "\x08");
        assert_eq!(key("esc", false, true, false), "\x1b\x1b");
    }

    #[test]
    fn control_characters() {
        assert_eq!(modified('c', true, false), "\x03");
        assert_eq!(modified('[', true, false), "\x1b");
        assert_eq!(modified(' ', true, false), "\0");
        assert_eq!(modified('x', false, true), "\x1bx");
        assert_eq!(modified('é', true, false), "é");
    }
}
