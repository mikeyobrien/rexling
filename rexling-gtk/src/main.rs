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
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| match app.active_window() {
        Some(window) => window.present(),
        None => window::build(app),
    });
    for (_, action, keys) in dialogs::ACTIONS.iter().filter(|(_, _, keys)| !keys.is_empty()) {
        app.set_accels_for_action(&format!("win.{action}"), &[keys]);
    }
    app.run()
}
