mod client;
mod config;
mod dialogs;
mod input;
mod palette;
mod panegrid;
mod programs;
mod terminal;
mod theme;
mod tiles;
mod tunnel;
mod window;

use adw::prelude::*;

const APP_ID: &str = "dev.shulhi.Rexling";

fn main() -> gtk::glib::ExitCode {
    // A development build can run beside the installed app under another ID.
    let id = std::env::var("REXLING_APP_ID").unwrap_or_else(|_| APP_ID.to_owned());
    let app = adw::Application::builder().application_id(id).build();
    app.connect_activate(|app| match app.active_window() {
        Some(window) => window.present(),
        None => window::build(app),
    });
    for (_, action, keys) in dialogs::ACTIONS.iter().filter(|(_, _, keys)| !keys.is_empty()) {
        app.set_accels_for_action(&format!("win.{action}"), &[keys]);
    }
    app.run()
}
