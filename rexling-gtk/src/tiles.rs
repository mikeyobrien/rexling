//! Program artwork. The Android app ships each tile as a list of vector drawing commands;
//! here they become SVG and are rasterised once per size.
use gtk::{gdk, gdk_pixbuf, gio, glib};
use serde_json::{Value, value::RawValue};
use std::{cell::RefCell, collections::HashMap, fmt::Write, sync::OnceLock};

const WIDTH: f64 = 61.0;
const HEIGHT: f64 = 48.0;

/// Each tile's command list, left unparsed until that tile is first drawn.
fn catalogue() -> &'static HashMap<String, Box<RawValue>> {
    #[derive(serde::Deserialize)]
    struct Catalogue {
        icons: HashMap<String, Box<RawValue>>,
    }
    static CATALOGUE: OnceLock<HashMap<String, Box<RawValue>>> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let text = include_str!("../../rexling-android/app/src/main/assets/program_tiles.json");
        serde_json::from_str::<Catalogue>(text).map(|c| c.icons).unwrap_or_default()
    })
}
fn commands(name: &str) -> Option<Vec<Value>> {
    serde_json::from_str(catalogue().get(name)?.get()).ok()
}

pub fn exists(name: &str) -> bool {
    catalogue().contains_key(name)
}

/// CSS colours as the tiles write them, with Display P3 mapped onto sRGB for renderers without wide gamut.
fn colour(value: &str) -> String {
    if let Some(body) = value.strip_prefix("color(display-p3").and_then(|v| v.strip_suffix(')')) {
        let channel: Vec<u8> = body.split_whitespace().filter_map(|v| v.parse::<f64>().ok()).map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8).collect();
        if let [r, g, b, ..] = channel[..] {
            return format!("rgb({r},{g},{b})");
        }
    }
    if value == "currentColor" { "#000".into() } else { value.to_owned() }
}

fn svg(commands: &[Value], pixels: (i32, i32)) -> String {
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {WIDTH} {HEIGHT}">"#,
        pixels.0, pixels.1
    );
    // Each save opens a scope; transforms and clips inside it become nested groups closed at its restore.
    let mut scopes = vec![0usize];
    let mut ids = 0;
    let number = |command: &Value, key: &str| command[key].as_f64().unwrap_or(0.0);
    let group = |out: &mut String, scopes: &mut Vec<usize>, attribute: String| {
        let _ = write!(out, "<g {attribute}>");
        *scopes.last_mut().unwrap() += 1;
    };
    for command in commands {
        match command["op"].as_str().unwrap_or("") {
            "save" => scopes.push(0),
            "restore" => {
                if scopes.len() > 1 {
                    for _ in 0..scopes.pop().unwrap() {
                        out.push_str("</g>");
                    }
                }
            }
            "translate" => group(&mut out, &mut scopes, format!(r#"transform="translate({} {})""#, number(command, "x"), number(command, "y"))),
            "scale" => group(&mut out, &mut scopes, format!(r#"transform="scale({} {})""#, number(command, "x"), number(command, "y"))),
            "transform" => group(&mut out, &mut scopes, format!(r#"transform="{}""#, command["value"].as_str().unwrap_or(""))),
            "clip" => {
                ids += 1;
                let rule = if command["evenOdd"] == true { "evenodd" } else { "nonzero" };
                let _ = write!(out, r#"<clipPath id="c{ids}"><path d="{}" clip-rule="{rule}"/></clipPath>"#, command["path"].as_str().unwrap_or(""));
                group(&mut out, &mut scopes, format!(r#"clip-path="url(#c{ids})""#));
            }
            "path" => {
                let paint = &command["paint"];
                let source = if let Some(kind) = paint["gradient"].as_str() {
                    ids += 1;
                    let _ = write!(out, r#"<{kind} id="g{ids}""#);
                    for (key, value) in paint["attributes"].as_object().into_iter().flatten().filter(|(key, _)| *key != "id") {
                        let _ = write!(out, r#" {key}="{}""#, value.as_str().unwrap_or(""));
                    }
                    out.push('>');
                    for stop in paint["stops"].as_array().into_iter().flatten() {
                        let opacity = stop["stop-opacity"].as_str().map(str::to_owned).or_else(|| stop["stop-opacity"].as_f64().map(|v| v.to_string())).unwrap_or_else(|| "1".into());
                        let _ = write!(out, r#"<stop offset="{}" stop-color="{}" stop-opacity="{opacity}"/>"#,
                            stop["offset"].as_str().unwrap_or("0"), colour(stop["stop-color"].as_str().unwrap_or("#000")));
                    }
                    let _ = write!(out, "</{kind}>");
                    format!("url(#g{ids})")
                } else {
                    colour(paint["color"].as_str().unwrap_or("#000"))
                };
                let _ = write!(out, r#"<path d="{}" opacity="{}""#, command["path"].as_str().unwrap_or(""), paint["alpha"].as_f64().unwrap_or(1.0));
                if paint["stroke"] == true {
                    let _ = write!(out, r#" fill="none" stroke="{source}" stroke-width="{}" stroke-linecap="{}" stroke-linejoin="{}"/>"#,
                        paint["strokeWidth"].as_f64().unwrap_or(1.0),
                        paint["strokeCap"].as_str().filter(|v| !v.is_empty()).unwrap_or("butt"),
                        paint["strokeJoin"].as_str().filter(|v| !v.is_empty()).unwrap_or("miter"));
                } else {
                    let rule = if command["evenOdd"] == true { "evenodd" } else { "nonzero" };
                    let _ = write!(out, r#" fill="{source}" fill-rule="{rule}"/>"#);
                }
            }
            _ => {}
        }
    }
    for _ in 0..scopes.iter().sum() {
        out.push_str("</g>");
    }
    out.push_str("</svg>");
    out
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, i32), Option<gdk::Texture>>> = RefCell::default();
}

/// The tile `name` drawn `height` device pixels tall, or the plain shell tile when there is no such tile.
pub fn texture(name: &str, height: i32) -> Option<gdk::Texture> {
    let name = if exists(name) { name } else { "shell" };
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((name.to_owned(), height))
            .or_insert_with(|| {
                let commands = commands(name)?;
                let width = (height as f64 * WIDTH / HEIGHT).round() as i32;
                let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(svg(&commands, (width, height)).into_bytes()));
                let pixbuf = gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()?;
                Some(gdk::Texture::for_pixbuf(&pixbuf))
            })
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_becomes_balanced_svg() {
        assert!(catalogue().len() > 100);
        for name in catalogue().keys() {
            let document = svg(&commands(name).unwrap(), (61, 48));
            assert_eq!(document.matches("<g ").count(), document.matches("</g>").count(), "{name}");
            assert!(document.contains("<path"), "{name}");
        }
    }

    #[test]
    fn wide_gamut_colours_fall_back_to_srgb() {
        assert_eq!(colour("color(display-p3 1 0.5 0)"), "rgb(255,128,0)");
        assert_eq!(colour("#e6683d"), "#e6683d");
    }
}
