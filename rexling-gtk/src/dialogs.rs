//! The command palette, preferences, keyboard shortcuts and About.
use crate::{
    palette::Palette,
    theme,
    window::{Command, Ui},
};
use adw::prelude::*;
use gtk::{gdk, glib, pango};
use std::{cell::RefCell, rc::Rc};

/// Every window action: its name in the palette, its `win.` action and its keys.
pub const ACTIONS: &[(&str, &str, &str)] = &[
    ("New Tab", "new-tab", "<Ctrl><Shift>t"),
    ("Split Right", "split-right", "<Ctrl><Shift>d"),
    ("Split Below", "split-below", "<Ctrl><Shift>e"),
    ("Close Pane", "close-pane", "<Ctrl><Shift>w"),
    ("Close Tab", "close-tab", ""),
    ("Rename Tab…", "rename-tab", "F2"),
    ("Move Tab Up", "move-tab-up", "<Ctrl><Shift>Page_Up"),
    ("Move Tab Down", "move-tab-down", "<Ctrl><Shift>Page_Down"),
    ("New Session", "new-session", ""),
    ("Rename Session…", "rename-session", ""),
    ("Close Session…", "close-session", ""),
    ("Attach Image…", "attach-image", ""),
    ("Port Forwards…", "forwards", ""),
    ("Zoom Pane", "zoom-pane", "<Ctrl><Shift>Return"),
    ("Next Pane", "next-pane", "<Ctrl><Alt>Right"),
    ("Previous Pane", "previous-pane", "<Ctrl><Alt>Left"),
    ("Next Tab", "next-tab", "<Ctrl>Page_Down"),
    ("Previous Tab", "previous-tab", "<Ctrl>Page_Up"),
    ("Switch Session", "sessions", "<Ctrl><Shift>s"),
    ("Toggle Sidebar", "toggle-sidebar", "<Ctrl><Shift>b"),
    ("Command Palette", "palette", "<Ctrl><Shift>p"),
    ("Preferences", "preferences", "<Ctrl>comma"),
    ("Keyboard Shortcuts", "shortcuts", "<Ctrl>question"),
    ("About Rexling", "about", ""),
];

/// Keys the terminal itself handles.
const TERMINAL_KEYS: &[(&str, &str)] = &[
    ("Copy", "<Ctrl><Shift>c"),
    ("Paste", "<Ctrl><Shift>v"),
    ("Scroll Back", "<Shift>Page_Up"),
    ("Scroll Forward", "<Shift>Page_Down"),
    ("Larger Text", "<Ctrl>plus"),
    ("Smaller Text", "<Ctrl>minus"),
    ("Reset Text Size", "<Ctrl>0"),
];

fn keys(accelerator: &str) -> String {
    gtk::accelerator_parse(accelerator).map(|(key, modifiers)| gtk::accelerator_get_label(key, modifiers).to_string()).unwrap_or_default()
}

/// Letters of `query` appear in `text` in order; earlier and tighter matches score lower (better).
fn score(text: &str, query: &str) -> Option<usize> {
    let text = text.to_lowercase();
    if let Some(at) = text.find(query) {
        return Some(at);
    }
    let mut position = 0;
    let mut spread = 0;
    for wanted in query.chars().filter(|c| !c.is_whitespace()) {
        let found = text[position..].find(wanted)?;
        spread += found;
        position += found + wanted.len_utf8();
    }
    Some(100 + spread)
}

pub fn palette(ui: &Rc<Ui>) {
    let mut commands: Vec<Command> = ACTIONS
        .iter()
        .filter(|(_, action, _)| *action != "palette")
        .map(|(label, action, accelerator)| {
            let command: Command = (
                (*label).to_owned(),
                keys(accelerator),
                Box::new(move |ui| {
                    let _ = WidgetExt::activate_action(&ui.window, &format!("win.{action}"), None);
                }),
            );
            command
        })
        .collect();
    commands.extend(ui.destinations());
    let commands = Rc::new(commands);

    let entry = gtk::SearchEntry::builder().placeholder_text("Search commands, tabs, sessions and themes").margin_top(12).margin_start(12).margin_end(12).margin_bottom(6).build();
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Browse).css_classes(["navigation-sidebar", "rexling-palette"]).build();
    let scroller = gtk::ScrolledWindow::builder().child(&list).hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_height(true).max_content_height(420).build();
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).width_request(560).margin_bottom(8).build();
    content.append(&entry);
    content.append(&scroller);
    let dialog = adw::Dialog::builder().child(&content).content_width(560).follows_content_size(true).build();

    let fill = {
        let (list, commands) = (list.clone(), commands.clone());
        move |query: &str| {
            list.remove_all();
            let query = query.trim().to_lowercase();
            let mut matches: Vec<(usize, usize)> = commands.iter().enumerate().filter_map(|(index, (label, _, _))| score(label, &query).map(|s| (s, index))).collect();
            matches.sort();
            for (_, index) in matches.into_iter().take(60) {
                let (label, hint, _) = &commands[index];
                let row = gtk::Box::builder().spacing(12).build();
                row.append(&gtk::Label::builder().label(label.as_str()).xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).build());
                row.append(&gtk::Label::builder().label(hint.as_str()).css_classes(["rexling-key"]).build());
                list.append(&gtk::ListBoxRow::builder().child(&row).name(index.to_string()).build());
            }
            list.select_row(list.row_at_index(0).as_ref());
        }
    };
    fill("");
    entry.connect_search_changed({
        let fill = fill.clone();
        move |entry| fill(&entry.text())
    });

    let run = {
        let (weak, commands, dialog) = (Rc::downgrade(ui), commands.clone(), dialog.clone());
        move |row: &gtk::ListBoxRow| {
            let (Some(ui), Ok(index)) = (weak.upgrade(), row.widget_name().parse::<usize>()) else { return };
            dialog.close();
            if let Some((_, _, command)) = commands.get(index) {
                command(&ui);
            }
        }
    };
    list.connect_row_activated({
        let run = run.clone();
        move |_, row| run(row)
    });
    entry.connect_activate({
        let (list, run) = (list.clone(), run.clone());
        move |_| {
            if let Some(row) = list.selected_row() {
                run(&row);
            }
        }
    });
    entry.connect_stop_search({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    // Arrow keys move through results while the text cursor stays in the search field.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let list = list.clone();
        move |_, key, _, _| {
            let step = match key {
                gdk::Key::Down => 1,
                gdk::Key::Up => -1,
                _ => return glib::Propagation::Proceed,
            };
            let current = list.selected_row().map_or(-1, |row| row.index());
            if let Some(row) = list.row_at_index((current + step).max(0)) {
                list.select_row(Some(&row));
                // Keep the highlighted result in view without moving keyboard focus off the field.
                if let (Some(bounds), Some(adjustment)) = (row.compute_bounds(&list), list.adjustment()) {
                    let (top, bottom) = (bounds.y() as f64, (bounds.y() + bounds.height()) as f64);
                    if top < adjustment.value() {
                        adjustment.set_value(top);
                    } else if bottom > adjustment.value() + adjustment.page_size() {
                        adjustment.set_value(bottom - adjustment.page_size());
                    }
                }
            }
            glib::Propagation::Stop
        }
    });
    entry.add_controller(keys);
    let weak = Rc::downgrade(ui);
    dialog.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.focus_terminal();
        }
    });
    dialog.present(Some(&ui.window));
    entry.grab_focus();
}

/// A miniature terminal in the theme's colours.
fn swatch(palette: Palette) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().content_width(148).content_height(88).build();
    area.set_draw_func(move |_, cr, width, height| {
        let colour = |c: [u8; 3]| (c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0);
        let (width, height, radius) = (width as f64, height as f64, 9.0);
        cr.new_sub_path();
        cr.arc(width - radius, radius, radius, -std::f64::consts::FRAC_PI_2, 0.0);
        cr.arc(width - radius, height - radius, radius, 0.0, std::f64::consts::FRAC_PI_2);
        cr.arc(radius, height - radius, radius, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        cr.arc(radius, radius, radius, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        cr.close_path();
        let (r, g, b) = colour(palette.background);
        cr.set_source_rgb(r, g, b);
        let _ = cr.fill_preserve();
        let (r, g, b) = colour(palette.foreground);
        cr.set_source_rgba(r, g, b, 0.18);
        cr.set_line_width(1.0);
        let _ = cr.stroke();
        // Lines of "text": a prompt, coloured words, and the cursor.
        let lines: [&[(usize, f64)]; 4] = [&[(4, 14.0), (7, 34.0), (2, 22.0)], &[(7, 52.0), (3, 18.0)], &[(1, 26.0), (5, 30.0), (6, 16.0)], &[(4, 14.0)]];
        for (row, words) in lines.iter().enumerate() {
            let (mut x, y) = (12.0, 14.0 + row as f64 * 16.0);
            for (index, length) in words.iter() {
                let (r, g, b) = colour(palette.ansi[*index]);
                cr.set_source_rgb(r, g, b);
                cr.rectangle(x, y, *length, 6.0);
                let _ = cr.fill();
                x += length + 6.0;
            }
            if row == lines.len() - 1 {
                let (r, g, b) = colour(theme::accent(&palette));
                cr.set_source_rgb(r, g, b);
                cr.rectangle(x, y - 3.0, 7.0, 12.0);
                let _ = cr.fill();
            }
        }
    });
    area
}

fn theme_gallery(ui: &Rc<Ui>, dark: bool) -> gtk::FlowBox {
    let gallery = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(4).min_children_per_line(2).column_spacing(6).row_spacing(6).homogeneous(true).build();
    let current = ui.theme().id.clone();
    let cards: Rc<RefCell<Vec<(String, gtk::Box)>>> = Rc::default();
    for theme in theme::all().iter().filter(|t| t.palette.dark == dark) {
        let frame = gtk::Box::builder().css_classes(["rexling-swatch"]).halign(gtk::Align::Center).build();
        frame.append(&swatch(theme.palette));
        if theme.id == current {
            frame.add_css_class("current");
        }
        let card = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
        card.append(&frame);
        card.append(&gtk::Label::builder().label(theme.name.as_str()).css_classes(["caption"]).ellipsize(pango::EllipsizeMode::End).build());
        let button = gtk::Button::builder().child(&card).css_classes(["flat"]).tooltip_text(theme.name.as_str()).build();
        let (weak, id, all) = (Rc::downgrade(ui), theme.id.clone(), cards.clone());
        button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.choose_theme(&id);
                for (other, frame) in all.borrow().iter() {
                    if *other == id { frame.add_css_class("current") } else { frame.remove_css_class("current") }
                }
            }
        });
        cards.borrow_mut().push((theme.id.clone(), frame));
        gallery.insert(&button, -1);
    }
    gallery
}

pub fn preferences(ui: &Rc<Ui>) {
    let dialog = adw::PreferencesDialog::builder().title("Preferences").build();

    let appearance = adw::PreferencesPage::builder().title("Appearance").icon_name("applications-graphics-symbolic").build();
    let look = adw::PreferencesGroup::new();
    let modes = ["system", "dark", "light"];
    let mode = adw::ComboRow::builder().title("Style").model(&gtk::StringList::new(&["Follow System", "Dark", "Light"])).build();
    mode.set_selected(modes.iter().position(|m| *m == ui.config.borrow().appearance).unwrap_or(0) as u32);
    let weak = Rc::downgrade(ui);
    mode.connect_selected_notify(move |row| {
        if let Some(ui) = weak.upgrade() {
            let chosen = modes[(row.selected() as usize).min(2)];
            if ui.config.borrow().appearance != chosen {
                ui.config.borrow_mut().appearance = chosen.into();
                ui.config.borrow().save();
                ui.apply_appearance();
            }
        }
    });
    look.add(&mode);

    let font = gtk::FontDialogButton::builder().dialog(&gtk::FontDialog::builder().modal(true).build()).valign(gtk::Align::Center).use_font(false).use_size(false).build();
    let described = {
        let config = ui.config.borrow();
        // The setting is a fallback list; the picker shows its first family.
        let mut description = pango::FontDescription::from_string(config.font.split(',').next().unwrap_or("monospace").trim());
        description.set_size((config.font_size * pango::SCALE as f64) as i32);
        description
    };
    font.set_font_desc(&described);
    let weak = Rc::downgrade(ui);
    font.connect_font_desc_notify(move |button| {
        let (Some(ui), Some(description)) = (weak.upgrade(), button.font_desc()) else { return };
        let Some(family) = description.family() else { return };
        {
            let mut config = ui.config.borrow_mut();
            // Keep a monospace fallback behind whatever was picked.
            config.font = format!("{family}, monospace");
            if description.size() > 0 {
                config.font_size = (description.size() as f64 / pango::SCALE as f64).clamp(6.0, 32.0);
            }
            config.save();
        }
        ui.apply_font();
    });
    let font_row = adw::ActionRow::builder().title("Font").activatable_widget(&font).build();
    font_row.add_suffix(&font);
    look.add(&font_row);
    appearance.add(&look);

    for (title, dark) in [("Dark Themes", true), ("Light Themes", false)] {
        let group = adw::PreferencesGroup::builder().title(title).build();
        group.add(&theme_gallery(ui, dark));
        appearance.add(&group);
    }
    dialog.add(&appearance);

    let general = adw::PreferencesPage::builder().title("General").icon_name("preferences-system-symbolic").build();
    let servers = adw::PreferencesGroup::builder().title("Servers").build();
    let (current, saved) = {
        let config = ui.config.borrow();
        (config.server.clone(), config.servers.clone())
    };
    for address in saved {
        let row = adw::ActionRow::builder().title(crate::tunnel::label(&address)).subtitle(address.as_str()).activatable(true).build();
        if address == current {
            row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
        } else {
            let forget = gtk::Button::builder().icon_name("user-trash-symbolic").tooltip_text("Forget").valign(gtk::Align::Center).css_classes(["flat"]).build();
            let (weak, target, group, entry) = (Rc::downgrade(ui), address.clone(), servers.clone(), row.clone());
            forget.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    let mut config = ui.config.borrow_mut();
                    config.servers.retain(|s| *s != target);
                    config.sessions.remove(&target);
                    config.save();
                    group.remove(&entry);
                }
            });
            row.add_suffix(&forget);
        }
        let (weak, target, parent) = (Rc::downgrade(ui), address.clone(), dialog.clone());
        row.connect_activated(move |_| {
            if let Some(ui) = weak.upgrade() {
                parent.close();
                ui.connect(&target);
            }
        });
        servers.add(&row);
    }
    let url = adw::EntryRow::builder().title("Add a server: https://…, ssh://user@host or unix:///path").show_apply_button(true).build();
    let (weak, parent) = (Rc::downgrade(ui), dialog.clone());
    url.connect_apply(move |row| {
        if let Some(ui) = weak.upgrade().filter(|_| !row.text().trim().is_empty()) {
            parent.close();
            ui.connect(row.text().trim());
        }
    });
    servers.add(&url);
    general.add(&servers);
    let alerts = adw::PreferencesGroup::builder().title("Notifications").build();
    let notify = adw::SwitchRow::builder().title("When a program needs you").active(ui.config.borrow().notifications).build();
    let weak = Rc::downgrade(ui);
    notify.connect_active_notify(move |row| {
        if let Some(ui) = weak.upgrade() {
            ui.config.borrow_mut().notifications = row.is_active();
            ui.config.borrow().save();
        }
    });
    alerts.add(&notify);
    general.add(&alerts);
    dialog.add(&general);

    let weak = Rc::downgrade(ui);
    dialog.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.focus_terminal();
        }
    });
    dialog.present(Some(&ui.window));
}

pub fn shortcuts(ui: &Rc<Ui>) {
    let page = adw::PreferencesPage::new();
    let row = |group: &adw::PreferencesGroup, label: &str, accelerator: &str| {
        let row = adw::ActionRow::builder().title(label).build();
        row.add_suffix(&gtk::Label::builder().label(keys(accelerator)).css_classes(["rexling-key"]).build());
        group.add(&row);
    };
    let window = adw::PreferencesGroup::builder().title("Window").build();
    for (label, _, accelerator) in ACTIONS.iter().filter(|(_, _, accelerator)| !accelerator.is_empty()) {
        row(&window, label, accelerator);
    }
    page.add(&window);
    let terminal = adw::PreferencesGroup::builder().title("Terminal").build();
    for (label, accelerator) in TERMINAL_KEYS {
        row(&terminal, label, accelerator);
    }
    page.add(&terminal);
    let view = adw::ToolbarView::builder().content(&page).build();
    view.add_top_bar(&adw::HeaderBar::new());
    adw::Dialog::builder().title("Keyboard Shortcuts").child(&view).content_width(460).content_height(640).build().present(Some(&ui.window));
}

pub fn about(ui: &Rc<Ui>) {
    adw::AboutDialog::builder()
        .application_name("Rexling")
        .application_icon("dev.shulhi.Rexling")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Rexling contributors")
        .comments("An unofficial native client for Superlogical Rex.")
        .website("https://github.com/shulhi/rexling")
        .license_type(gtk::License::Apache20)
        .build()
        .present(Some(&ui.window));
}

/// Asks before ending a session: every program in it stops.
pub fn confirm_close_session(ui: &Rc<Ui>, label: &str, close: impl Fn(&Rc<Ui>) + 'static) {
    let dialog = adw::AlertDialog::builder().heading(format!("Close “{label}”?")).body("Every program in this session will be stopped.").close_response("cancel").default_response("cancel").build();
    dialog.add_responses(&[("cancel", "Cancel"), ("close", "Close Session")]);
    dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);
    let weak = Rc::downgrade(ui);
    dialog.connect_response(None, move |_, response| {
        if let (Some(ui), "close") = (weak.upgrade(), response) {
            close(&ui);
        }
    });
    dialog.present(Some(&ui.window));
}

/// Asks for a name. An empty name is passed on: the server then restores its default.
pub fn rename(ui: &Rc<Ui>, heading: &str, current: &str, apply: impl Fn(&Rc<Ui>, &str) + 'static) {
    let entry = gtk::Entry::builder().text(current).activates_default(true).build();
    let dialog = adw::AlertDialog::builder().heading(heading).extra_child(&entry).close_response("cancel").default_response("rename").build();
    dialog.add_responses(&[("cancel", "Cancel"), ("rename", "Rename")]);
    dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
    let (weak, field) = (Rc::downgrade(ui), entry.clone());
    dialog.connect_response(None, move |_, response| {
        let Some(ui) = weak.upgrade() else { return };
        if response == "rename" {
            apply(&ui, field.text().trim());
        }
        ui.focus_terminal();
    });
    dialog.present(Some(&ui.window));
    entry.grab_focus();
}

/// Lists the remote ports reachable on this computer and adds new ones.
pub fn forwards(ui: &Rc<Ui>) {
    let page = adw::PreferencesPage::new();
    let add = adw::PreferencesGroup::new();
    let port = adw::EntryRow::builder().title("Port on the server, or host:port").input_purpose(gtk::InputPurpose::Digits).show_apply_button(true).build();
    let weak = Rc::downgrade(ui);
    port.connect_apply(move |row| {
        let Some(ui) = weak.upgrade() else { return };
        let text = row.text();
        let (host, number) = text.trim().rsplit_once(':').unwrap_or(("127.0.0.1", text.trim()));
        match number.parse::<u16>() {
            Ok(number) if number > 0 && !host.is_empty() => {
                ui.forward(host, number);
                row.set_text("");
                row.remove_css_class("error");
            }
            _ => row.add_css_class("error"),
        }
    });
    add.add(&port);
    page.add(&add);
    let active = adw::PreferencesGroup::new();
    page.add(&active);

    let rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::default();
    let refresh: Rc<dyn Fn()> = {
        let (weak, active, rows) = (Rc::downgrade(ui), active.clone(), rows.clone());
        Rc::new(move || {
            let Some(ui) = weak.upgrade() else { return };
            for row in rows.borrow_mut().drain(..) {
                active.remove(&row);
            }
            for forward in ui.forwards.borrow().iter() {
                let status = match forward.phase.as_str() {
                    "active" => forward.address.clone(),
                    "failed" if forward.error.is_empty() || forward.error.contains("is unavailable") => "No local port is free".to_owned(),
                    "failed" => forward.error.clone(),
                    _ => "Connecting…".to_owned(),
                };
                let row = adw::ActionRow::builder().title(forward.target.as_str()).subtitle(status).build();
                if forward.phase == "active" {
                    let open = gtk::Button::builder().icon_name("web-browser-symbolic").tooltip_text("Open in Browser").valign(gtk::Align::Center).css_classes(["flat"]).build();
                    let (window, address) = (ui.window.clone(), forward.address.clone());
                    open.connect_clicked(move |_| gtk::UriLauncher::new(&format!("http://{address}")).launch(Some(&window), gtk::gio::Cancellable::NONE, |_| {}));
                    row.add_suffix(&open);
                }
                let stop = gtk::Button::builder().icon_name("user-trash-symbolic").tooltip_text("Stop").valign(gtk::Align::Center).css_classes(["flat"]).build();
                let (weak, handle) = (Rc::downgrade(&ui), forward.handle);
                stop.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        ui.stop_forward(handle);
                    }
                });
                row.add_suffix(&stop);
                active.add(&row);
                rows.borrow_mut().push(row);
            }
        })
    };
    refresh();
    ui.forwards_changed.replace(Some(refresh));

    let view = adw::ToolbarView::builder().content(&page).build();
    view.add_top_bar(&adw::HeaderBar::new());
    let dialog = adw::Dialog::builder().title("Port Forwards").child(&view).content_width(460).content_height(420).build();
    let weak = Rc::downgrade(ui);
    dialog.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.forwards_changed.replace(None);
            ui.focus_terminal();
        }
    });
    dialog.present(Some(&ui.window));
    port.grab_focus();
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn palette_ranks_direct_matches_first() {
        assert_eq!(score("Split Right", "split"), Some(0));
        assert!(score("Split Right", "sr").is_some());
        assert!(score("New Tab", "split").is_none());
        assert!(score("Toggle Sidebar", "rght").is_none());
        assert!(score("Split Right", "right") < score("Split Right", "srt"), "a whole word outranks scattered letters");
    }
}
