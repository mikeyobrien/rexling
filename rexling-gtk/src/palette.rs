//! Terminal colours. The defaults follow the Android client's `TerminalPalette`, except that green is
//! a real green: the Android palette reuses blue there, which makes diffs and test results unreadable.
use rex_core::ghostty::Cell;

pub type Rgb = [u8; 3];

#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    pub ansi: [Rgb; 16],
}

const fn rgb(value: u32) -> Rgb {
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}
const fn table(values: [u32; 16]) -> [Rgb; 16] {
    let mut out = [[0; 3]; 16];
    let mut i = 0;
    while i < 16 {
        out[i] = rgb(values[i]);
        i += 1;
    }
    out
}

pub const DARK: Palette = Palette {
    dark: true,
    background: rgb(0x10141b),
    foreground: rgb(0xdce1eb),
    accent: rgb(0x315cf4),
    ansi: table([
        0x232833, 0xf08088, 0x7ccf9a, 0xe9b65a, 0x7293ff, 0xbba2f7, 0x83c8d7, 0xdce1eb, 0x929aaf, 0xff9aa3, 0x97e3b1, 0xffcb75, 0x88a4ff,
        0xcdb4ff, 0x9ce1ec, 0xf2f5fc,
    ]),
};
pub const LIGHT: Palette = Palette {
    dark: false,
    background: rgb(0xffffff),
    foreground: rgb(0x202432),
    accent: rgb(0x3156f4),
    ansi: table([
        0x202432, 0xc03e47, 0x1f8a4c, 0xa65b00, 0x3156f4, 0x873bff, 0x087d91, 0xe3e5ee, 0x62697c, 0xd13f50, 0x24a05a, 0x995200, 0x3156f4,
        0x873bff, 0x007d91, 0xf4f5fa,
    ]),
};

// Cell attribute bits, as the engine sets them.
pub const BOLD: u32 = 1;
pub const ITALIC: u32 = 2;
pub const FAINT: u32 = 4;
pub const INVERSE: u32 = 8;
pub const INVISIBLE: u32 = 16;
pub const STRIKETHROUGH: u32 = 32;
pub const UNDERLINE: u32 = 64;
/// The underline is a curly one, as editors use for spelling and errors.
pub const CURLY: u32 = 1 << 31;
const DEFAULT_FOREGROUND: u32 = 128;
const DEFAULT_BACKGROUND: u32 = 256;
const PALETTE_FOREGROUND: u32 = 512;
const PALETTE_BACKGROUND: u32 = 1024;

impl Palette {
    pub fn foreground(&self, cell: &Cell) -> Rgb {
        if cell.attributes & DEFAULT_FOREGROUND != 0 {
            self.foreground
        } else if cell.attributes & PALETTE_FOREGROUND != 0 {
            self.ansi.get(((cell.attributes >> 11) & 255) as usize).copied().unwrap_or(cell.foreground)
        } else {
            cell.foreground
        }
    }
    pub fn background(&self, cell: &Cell) -> Rgb {
        if cell.attributes & DEFAULT_BACKGROUND != 0 {
            self.background
        } else if cell.attributes & PALETTE_BACKGROUND != 0 {
            self.ansi.get(((cell.attributes >> 19) & 255) as usize).copied().unwrap_or(cell.background)
        } else {
            cell.background
        }
    }
    /// The `theme` command arguments, so programs that ask the terminal for its colours get these.
    pub fn theme(&self) -> serde_json::Value {
        let hex = |c: Rgb| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        serde_json::json!({
            "scheme": if self.dark { "dark" } else { "light" },
            "background": hex(self.background),
            "foreground": hex(self.foreground),
            "cursor": hex(self.accent),
            "palette": self.ansi.iter().enumerate().map(|(i, c)| format!("{i}={}", hex(*c))).collect::<Vec<_>>(),
        })
    }
}
