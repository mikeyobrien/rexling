//! Sends a file to a pane without a window. Usage: upload <server> <session> <block> <file>
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [server, session, block, file] = &args[..] else { anyhow::bail!("usage: upload <server> <session> <block> <file>") };
    let name = std::path::Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let handle = rex_engine::uploads::create(serde_json::json!({"endpoint": server, "session": session, "block": block,
        "path": file, "name": name, "ref": format!("upload:{}", gtk::glib::uuid_string_random())}))?;
    loop {
        let status = rex_engine::uploads::poll(handle);
        let phase = status["phase"].as_str().unwrap_or("failed").to_owned();
        if matches!(phase.as_str(), "done" | "unsupported" | "failed" | "uncertain" | "canceled") {
            println!("{status}");
            rex_engine::uploads::destroy(handle);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}
