//! Connects without a window and prints each state revision. Usage: probe <server-url> [seconds]
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    let server = std::env::args().nth(1).expect("server URL");
    let seconds: u64 = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(6);
    let session = rex_engine::engine::Session::spawn()?;
    session.command(serde_json::json!({"type":"connect","server":server}))?;
    let end = Instant::now() + Duration::from_secs(seconds);
    let mut revision = 0u64;
    let select = std::env::var("PROBE_SESSION").ok();
    let mut selected = false;
    while Instant::now() < end {
        let next = session.updates.wait(revision, 500);
        if next <= revision as i64 { continue; }
        let text = session.poll(revision)?;
        revision = next as u64;
        if text.is_empty() { continue; }
        let mut value: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(screen) = value.get_mut("screen").filter(|s| !s.is_null()) {
            *screen = serde_json::json!({"columns":screen["columns"],"rows":screen["rows"]});
        }
        value["previews"] = serde_json::Value::Null;
        value["activity"] = serde_json::Value::Null;
        println!("{value}");
        if let (Some(id), false, Some("connected")) = (&select, selected, value["phase"].as_str()) {
            selected = true;
            session.command(serde_json::json!({"type":"session","id":id,"navigationId":1}))?;
        }
    }
    Ok(())
}
