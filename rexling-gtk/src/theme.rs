//! Colour themes. A theme colours the terminal and, through generated CSS, the whole window,
//! so the chrome and the terminal read as one surface.
use crate::palette::{DARK, LIGHT, Palette, Rgb};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub palette: Palette,
}

#[derive(Deserialize)]
struct Stored {
    id: String,
    name: String,
    dark: bool,
    background: String,
    foreground: String,
    cursor: String,
    ansi: Vec<String>,
}

fn rgb(text: &str) -> Option<Rgb> {
    let value = u32::from_str_radix(text.strip_prefix('#')?, 16).ok()?;
    (text.len() == 7).then_some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

pub const DEFAULT_DARK: &str = "rexling-dark";
pub const DEFAULT_LIGHT: &str = "rexling-light";

/// Rexling's own pair first, then the themes shared with the Android app.
pub fn all() -> &'static [Theme] {
    static THEMES: OnceLock<Vec<Theme>> = OnceLock::new();
    THEMES.get_or_init(|| {
        let mut themes = vec![
            Theme { id: DEFAULT_DARK.into(), name: "Rexling Dark".into(), palette: DARK },
            Theme { id: DEFAULT_LIGHT.into(), name: "Rexling Light".into(), palette: LIGHT },
        ];
        let stored: Vec<Stored> = serde_json::from_str(include_str!("../../rexling-android/app/src/main/assets/themes/rex-builtins.json")).unwrap_or_default();
        for theme in stored {
            let (Some(background), Some(foreground), Some(accent)) = (rgb(&theme.background), rgb(&theme.foreground), rgb(&theme.cursor)) else { continue };
            let mut ansi = [[0u8; 3]; 16];
            if theme.ansi.len() < 16 || !ansi.iter_mut().zip(&theme.ansi).all(|(slot, text)| rgb(text).map(|c| *slot = c).is_some()) {
                continue;
            }
            themes.push(Theme { id: theme.id, name: theme.name, palette: Palette { dark: theme.dark, background, foreground, accent, ansi } });
        }
        themes
    })
}

/// The theme with this id, or the default for that brightness when it is unknown or the wrong brightness.
pub fn find(id: &str, dark: bool) -> &'static Theme {
    let themes = all();
    themes.iter().find(|t| t.id == id && t.palette.dark == dark).unwrap_or(&themes[if dark { 0 } else { 1 }])
}

pub fn mix(a: Rgb, b: Rgb, amount: f64) -> Rgb {
    let channel = |i: usize| (a[i] as f64 + (b[i] as f64 - a[i] as f64) * amount).round() as u8;
    [channel(0), channel(1), channel(2)]
}
fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
fn luminance(c: Rgb) -> f64 {
    let linear = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
}
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Text that would all but vanish into what is behind it, such as white on a cream theme, is
/// moved far enough towards black or white to be read. Text set to exactly the colour behind it
/// was hidden on purpose and stays hidden.
pub fn legible(text: Rgb, behind: Rgb) -> Rgb {
    if text == behind || contrast(text, behind) >= 1.6 {
        return text;
    }
    let towards = if luminance(behind) > 0.4 { [0, 0, 0] } else { [255, 255, 255] };
    (1..=10).map(|step| mix(text, towards, step as f64 / 10.0)).find(|c| contrast(*c, behind) >= 2.6).unwrap_or(towards)
}

/// An accent that stays visible on the background: some themes use a cursor colour close to it.
pub fn accent(palette: &Palette) -> Rgb {
    let mut accent = palette.accent;
    let mut step = 0.0;
    while contrast(accent, palette.background) < 3.0 && step < 1.0 {
        step += 0.1;
        accent = mix(palette.accent, palette.foreground, step);
    }
    accent
}

/// Stylesheet that recolours libadwaita with the theme and adds Rexling's own few rules.
pub fn css(palette: &Palette) -> String {
    let (bg, fg) = (palette.background, palette.foreground);
    let accent = accent(palette);
    let on_accent = if contrast(accent, [255, 255, 255]) >= contrast(accent, [0, 0, 0]) { [255, 255, 255] } else { [0, 0, 0] };
    let sidebar = mix(bg, fg, if palette.dark { 0.035 } else { 0.045 });
    let raised = mix(bg, fg, 0.08);
    // Light themes need a firmer line for the same visibility.
    let line = mix(bg, fg, if palette.dark { 0.16 } else { 0.22 });
    format!(
        r#"
@define-color window_bg_color {bg};
@define-color window_fg_color {fg};
@define-color view_bg_color {bg};
@define-color view_fg_color {fg};
@define-color headerbar_bg_color {bg};
@define-color headerbar_fg_color {fg};
@define-color headerbar_backdrop_color {bg};
@define-color headerbar_shade_color transparent;
@define-color headerbar_border_color transparent;
@define-color sidebar_bg_color {sidebar};
@define-color sidebar_fg_color {fg};
@define-color sidebar_backdrop_color {sidebar};
@define-color sidebar_shade_color transparent;
@define-color sidebar_border_color {line};
@define-color popover_bg_color {raised};
@define-color popover_fg_color {fg};
@define-color dialog_bg_color {raised};
@define-color dialog_fg_color {fg};
@define-color card_bg_color alpha({fg}, 0.06);
@define-color card_fg_color {fg};
@define-color accent_bg_color {accent};
@define-color accent_fg_color {on_accent};
@define-color accent_color {accent_text};
:root {{
  --window-bg-color: {bg}; --window-fg-color: {fg};
  --view-bg-color: {bg}; --view-fg-color: {fg};
  --headerbar-bg-color: {bg}; --headerbar-fg-color: {fg}; --headerbar-backdrop-color: {bg};
  --headerbar-shade-color: transparent; --headerbar-border-color: transparent;
  --sidebar-bg-color: {sidebar}; --sidebar-fg-color: {fg}; --sidebar-backdrop-color: {sidebar};
  --sidebar-shade-color: transparent; --sidebar-border-color: {line};
  --popover-bg-color: {raised}; --popover-fg-color: {fg};
  --dialog-bg-color: {raised}; --dialog-fg-color: {fg};
  --card-bg-color: alpha({fg}, 0.06); --card-fg-color: {fg};
  --accent-bg-color: {accent}; --accent-fg-color: {on_accent}; --accent-color: {accent_text};
}}
panegrid {{ background: {line}; }}
.rexling-float {{ border-radius: 10px; border: 2px solid alpha({accent}, 0.7); background: {bg}; }}
.rexling-float-bar {{ padding: 0 4px 0 12px; background: {sidebar}; border-bottom: 1px solid {line}; }}
.rexling-tabs row {{ border-radius: 10px; margin: 1px 6px; padding: 6px 8px; transition: background 120ms ease; }}
.rexling-tabs row:selected {{ background: alpha({fg}, 0.09); }}
.rexling-tab-title {{ font-weight: 600; }}
.rexling-status {{ font-size: 0.82em; font-weight: 600; }}
.rexling-status.attention {{ color: {warm}; }}
.rexling-status.failed {{ color: {red}; }}
.rexling-status.done {{ color: {green}; }}
.rexling-pane-line {{ border-radius: 6px; padding: 2px 6px; margin-left: 34px; }}
.rexling-pane-line:hover {{ background: alpha({fg}, 0.06); }}
.rexling-pane-dot.done {{ background: {green}; }}
.rexling-dot.done {{ background: {green}; }}
button.rexling-wants {{ background-image: radial-gradient(circle at 78% 24%, {warm} 0, {warm} 3px, transparent 3.5px); }}
.rexling-status.working {{ color: alpha({fg}, 0.55); }}
.rexling-dot {{ min-width: 8px; min-height: 8px; border-radius: 4px; background: {warm}; }}
.rexling-dot.failed {{ background: {red}; }}
.rexling-pane-dot {{ min-width: 6px; min-height: 6px; border-radius: 3px; background: alpha({fg}, 0.25); }}
.rexling-pane-dot.focused {{ background: @accent_bg_color; }}
.rexling-pane-dot.attention {{ background: {warm}; }}
.rexling-pane-dot.failed {{ background: {red}; }}
.rexling-pane-dot.working {{ background: alpha({fg}, 0.6); }}
.rexling-waiting {{ padding-top: 4px; padding-bottom: 4px; }}
.rexling-session {{ font-weight: 700; font-size: 1.05em; }}
.rexling-swatch {{ border-radius: 12px; border: 2px solid transparent; padding: 3px; transition: border-color 120ms ease; }}
.rexling-swatch.current {{ border-color: {accent}; }}
.rexling-palette row {{ border-radius: 8px; margin: 0 6px; padding: 8px 10px; }}
.rexling-palette row:hover:not(:selected) {{ background: alpha({fg}, 0.035); }}
.rexling-menu-item {{ padding: 6px 10px; font-weight: normal; }}
.rexling-cap {{ border: 1px solid alpha({fg}, 0.25); border-bottom-width: 2px; border-radius: 6px; padding: 1px 7px; font-size: 0.85em; background: alpha({fg}, 0.06); }}
.rexling-key {{ font-family: monospace; font-size: 0.85em; color: alpha({fg}, 0.6); }}
"#,
        bg = hex(bg),
        fg = hex(fg),
        sidebar = hex(sidebar),
        raised = hex(raised),
        line = hex(line),
        accent = hex(accent),
        accent_text = hex(mix(accent, fg, if contrast(accent, bg) < 4.5 { 0.35 } else { 0.0 })),
        on_accent = hex(on_accent),
        warm = hex(palette.ansi[3]),
        red = hex(palette.ansi[1]),
        green = hex(palette.ansi[2]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faint_text_is_made_legible_but_hidden_text_stays_hidden() {
        let cream = [0xf4, 0xee, 0xdf];
        assert!(contrast(legible([0xff, 0xff, 0xff], cream), cream) >= 2.6);
        assert_eq!(legible(cream, cream), cream);
        assert_eq!(legible([0x20, 0x20, 0x20], cream), [0x20, 0x20, 0x20]);
        let night = [0x10, 0x12, 0x18];
        assert!(contrast(legible([0x16, 0x18, 0x20], night), night) >= 2.6);
    }

    #[test]
    fn bundled_themes_load_and_resolve() {
        assert!(all().len() > 30);
        assert_eq!(find("no-such-theme", true).id, DEFAULT_DARK);
        assert_eq!(find(DEFAULT_DARK, false).id, DEFAULT_LIGHT, "a dark theme is never used for the light slot");
        assert!(all().iter().any(|t| t.id == "abyssal-trench" && t.palette.dark));
    }

    #[test]
    fn accent_is_always_visible() {
        for theme in all() {
            assert!(contrast(accent(&theme.palette), theme.palette.background) >= 2.9, "{}", theme.id);
        }
    }
}
