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
    ("New Window", "new-window", "<Ctrl><Shift>n"),
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
    ("Float Pane", "float-pane", "<Ctrl><Shift>f"),
    ("Dock Pane", "dock-pane", ""),
    ("Next Pane", "next-pane", "<Ctrl><Alt>Right"),
    ("Previous Pane", "previous-pane", "<Ctrl><Alt>Left"),
    ("Next Tab", "next-tab", "<Ctrl>Page_Down"),
    ("Previous Tab", "previous-tab", "<Ctrl>Page_Up"),
    ("Go to Waiting Pane", "next-waiting", "<Ctrl><Shift>u"),
    ("Switch Session", "sessions", "<Ctrl><Shift>s"),
    ("Toggle Sidebar", "toggle-sidebar", "<Ctrl><Shift>b"),
    ("Command Palette", "palette", "<Ctrl><Shift>p"),
    ("Preferences", "preferences", "<Ctrl>comma"),
    ("Keyboard Shortcuts", "shortcuts", "<Ctrl>question"),
    ("About Rexling", "about", ""),
];

/// Shows a dialog over the window. Window shortcuts wait until it closes, then the terminal has the keyboard again.
fn present(ui: &Rc<Ui>, dialog: &impl IsA<adw::Dialog>) {
    ui.modal.set(true);
    // Ctrl+Shift+D is Split Right, which waits while a dialog is open. It must not fall through to GTK's debugging key.
    let guard = gtk::EventControllerKey::new();
    guard.set_propagation_phase(gtk::PropagationPhase::Capture);
    guard.connect_key_pressed(|_, key, _, state| {
        let chord = state.contains(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK) && matches!(key, gdk::Key::D | gdk::Key::d | gdk::Key::I | gdk::Key::i);
        if chord { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    dialog.as_ref().add_controller(guard);
    let weak = Rc::downgrade(ui);
    dialog.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.modal.set(false);
            ui.focus_terminal();
        }
    });
    dialog.present(Some(&ui.window));
}

/// A menu of window commands, in groups, with their keys written the way the rest of the app writes them.
pub fn menu(groups: &[&[&str]]) -> gtk::Popover {
    build_menu(groups, true)
}
/// A menu without key hints, for commands aimed at something other than what the keys act on.
pub fn menu_plain(groups: &[&[&str]]) -> gtk::Popover {
    build_menu(groups, false)
}
fn build_menu(groups: &[&[&str]], hints: bool) -> gtk::Popover {
    let list = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
    let popover = gtk::Popover::builder().child(&list).css_classes(["menu"]).build();
    for (index, group) in groups.iter().enumerate() {
        if index > 0 {
            list.append(&gtk::Separator::builder().margin_top(4).margin_bottom(4).build());
        }
        for wanted in group.iter() {
            let Some((label, action, accelerator)) = ACTIONS.iter().find(|(_, action, _)| action == wanted) else { continue };
            let row = gtk::Box::builder().spacing(24).build();
            row.append(&gtk::Label::builder().label(*label).xalign(0.0).hexpand(true).build());
            if hints {
                row.append(&gtk::Label::builder().label(keys(accelerator)).css_classes(["dim-label", "caption"]).build());
            }
            let button = gtk::Button::builder().child(&row).action_name(format!("win.{action}")).css_classes(["flat", "rexling-menu-item"]).build();
            let weak = popover.downgrade();
            button.connect_clicked(move |_| {
                if let Some(popover) = weak.upgrade() {
                    popover.popdown();
                }
            });
            list.append(&button);
        }
    }
    popover
}

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

/// Keys the way they are written everywhere else: Ctrl+Shift+P, Ctrl+PageUp, Ctrl+Shift+Enter.
fn keys(accelerator: &str) -> String {
    let Some((key, modifiers)) = gtk::accelerator_parse(accelerator) else { return String::new() };
    let mut parts = Vec::new();
    for (modifier, name) in [(gdk::ModifierType::CONTROL_MASK, "Ctrl"), (gdk::ModifierType::ALT_MASK, "Alt"), (gdk::ModifierType::SHIFT_MASK, "Shift")] {
        if modifiers.contains(modifier) {
            parts.push(name.to_owned());
        }
    }
    parts.push(match key {
        gdk::Key::Return => "Enter".into(),
        gdk::Key::Page_Up => "PageUp".into(),
        gdk::Key::Page_Down => "PageDown".into(),
        gdk::Key::comma => ",".into(),
        gdk::Key::question => "?".into(),
        gdk::Key::plus => "+".into(),
        gdk::Key::minus => "−".into(),
        other => match other.to_unicode().filter(|c| c.is_alphanumeric()) {
            Some(character) => character.to_uppercase().to_string(),
            None => other.name().map(|name| name.to_string()).unwrap_or_default(),
        },
    });
    parts.join("+")
}

/// How well `query` matches a command, lower being better: the start of its name, the start of a
/// word in it, anywhere inside it, its kind ("theme", "tab"), then the initials of its words.
fn score(text: &str, kind: &str, query: &str) -> Option<usize> {
    let (text, kind) = (text.to_lowercase(), kind.to_lowercase());
    if query.is_empty() {
        return Some(0);
    }
    if text.starts_with(query) {
        return Some(0);
    }
    if text.split_whitespace().any(|word| word.starts_with(query)) {
        return Some(1);
    }
    if text.contains(query) {
        return Some(2);
    }
    if kind.split_whitespace().any(|word| word.starts_with(query)) {
        return Some(3);
    }
    let initials: String = text.split_whitespace().filter_map(|word| word.chars().next()).collect();
    initials.starts_with(query).then_some(4)
}

pub fn palette(ui: &Rc<Ui>) {
    let mut commands: Vec<Command> = ACTIONS
        .iter()
        // Only what can run right now.
        .filter(|(_, action, _)| *action != "palette" && ui.window.lookup_action(action).is_none_or(|a| a.is_enabled()))
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

    let entry = gtk::SearchEntry::builder().search_delay(0).placeholder_text("Search commands, tabs, sessions and themes").margin_top(12).margin_start(12).margin_end(12).margin_bottom(6).build();
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Browse).css_classes(["navigation-sidebar", "rexling-palette"]).build();
    let scroller = gtk::ScrolledWindow::builder().child(&list).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).margin_bottom(8).build();
    content.append(&entry);
    content.append(&scroller);
    // A fixed size: the box must not jump about as the results change.
    let dialog = adw::Dialog::builder().child(&content).content_width(560).content_height(460).build();

    let fill = {
        let (list, commands) = (list.clone(), commands.clone());
        move |query: &str| {
            list.remove_all();
            let query = query.trim().to_lowercase();
            // Only themes, tabs, sessions and servers have a kind; a command's hint is its keys.
            let kind = |hint: &str| if hint.ends_with("theme") || matches!(hint, "Tab" | "Session" | "Server") { hint.to_owned() } else { String::new() };
            let mut matches: Vec<(usize, usize)> = commands.iter().enumerate().filter_map(|(index, (label, hint, _))| score(label, &kind(hint), &query).map(|s| (s, index))).collect();
            matches.sort();
            for (_, index) in matches.into_iter().take(60) {
                let (label, hint, _) = &commands[index];
                let row = gtk::Box::builder().spacing(12).build();
                row.append(&gtk::Label::builder().label(label.as_str()).xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).build());
                row.append(&gtk::Label::builder().label(hint.as_str()).css_classes(["dim-label", "caption"]).build());
                list.append(&gtk::ListBoxRow::builder().child(&row).name(index.to_string()).build());
            }
            list.select_row(list.row_at_index(0).as_ref());
            if list.row_at_index(0).is_none() {
                list.append(&gtk::ListBoxRow::builder().child(&gtk::Label::builder().label("No matches").css_classes(["dim-label"]).margin_top(24).build()).activatable(false).selectable(false).name("none").build());
            }
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
            // The palette is on its way out; what it runs may open a dialog of its own.
            ui.modal.set(false);
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
        let (list, run, fill) = (list.clone(), run.clone(), fill.clone());
        move |entry| {
            // The list follows typing after a short pause; Enter must act on what is typed now.
            fill(&entry.text());
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
    present(ui, &dialog);
    entry.grab_focus();
}

/// A miniature terminal in the theme's colours.
fn swatch(palette: Palette) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().content_width(128).content_height(80).build();
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
    let gallery = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(4).min_children_per_line(2).column_spacing(0).row_spacing(2).homogeneous(true).build();
    let current = {
        let config = ui.config.borrow();
        theme::find(if dark { &config.theme_dark } else { &config.theme_light }, dark).id.clone()
    };
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

    // A terminal needs every character the same width, so only those families are offered.
    let context = ui.window.pango_context();
    // Monospace families that can write ordinary text: icon, emoji and symbol fonts are not for a terminal.
    let writes = |name: &str| context.load_font(&pango::FontDescription::from_string(name)).is_some_and(|font| ['a', 'M', '0'].iter().all(|c| font.has_char(*c)));
    let mut families: Vec<String> = context.list_families().iter().filter(|family| family.is_monospace()).map(|family| family.name().to_string()).filter(|name| writes(name) && !["Emoji", "SignWriting", "Symbols"].iter().any(|word| name.contains(word))).collect();
    families.sort_by_key(|name| name.to_lowercase());
    families.dedup();
    let current = ui.config.borrow().font.split(',').next().unwrap_or("").trim().to_owned();
    if !current.is_empty() && !families.contains(&current) {
        families.insert(0, current.clone());
    }
    // The row opens a page of its own: every family, written in itself, with room for long names.
    let font = adw::ActionRow::builder().title("Font").activatable(true).build();
    let chosen = gtk::Label::builder().label(current.as_str()).ellipsize(pango::EllipsizeMode::End).css_classes(["dim-label"]).build();
    font.add_suffix(&chosen);
    font.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let (weak, list, parent, shown) = (Rc::downgrade(ui), families.clone(), dialog.clone(), chosen.clone());
    font.connect_activated(move |_| {
        let Some(ui) = weak.upgrade() else { return };
        let current = ui.config.borrow().font.split(',').next().unwrap_or("").trim().to_owned();
        let rows = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
        for family in &list {
            let line = gtk::Box::builder().spacing(12).height_request(44).margin_start(12).margin_end(12).build();
            let name = gtk::Label::builder().label(family.as_str()).xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).build();
            let attributes = pango::AttrList::new();
            attributes.insert(pango::AttrString::new_family(family));
            name.set_attributes(Some(&attributes));
            line.append(&name);
            if *family == current {
                line.append(&gtk::Image::from_icon_name("object-select-symbolic"));
            }
            rows.append(&gtk::ListBoxRow::builder().child(&line).name(family.as_str()).build());
        }
        let search = gtk::SearchEntry::builder().placeholder_text("Search fonts").search_delay(0).build();
        let field = search.clone();
        rows.set_filter_func(move |row| row.widget_name().to_lowercase().contains(&field.text().trim().to_lowercase()));
        let nothing = gtk::Label::builder().label("No fonts match").css_classes(["dim-label"]).margin_top(18).visible(false).build();
        let (filtered, empty, names) = (rows.clone(), nothing.clone(), list.clone());
        search.connect_search_changed(move |field| {
            filtered.invalidate_filter();
            let typed = field.text().trim().to_lowercase();
            let any = names.iter().any(|name| name.to_lowercase().contains(&typed));
            filtered.set_visible(any);
            empty.set_visible(!any);
        });
        // Enter takes the first match.
        let (matching, names) = (rows.clone(), list.clone());
        search.connect_activate(move |field| {
            let typed = field.text().trim().to_lowercase();
            if let Some(row) = names.iter().position(|name| name.to_lowercase().contains(&typed)).and_then(|at| matching.row_at_index(at as i32)) {
                row.activate();
            }
        });
        let (weak, parent_for_pick, shown) = (Rc::downgrade(&ui), parent.clone(), shown.clone());
        rows.connect_row_activated(move |_, row| {
            let Some(ui) = weak.upgrade() else { return };
            let family = row.widget_name().to_string();
            // Keep a monospace fallback behind whatever was picked.
            ui.config.borrow_mut().font = format!("{family}, monospace");
            ui.config.borrow().save();
            ui.apply_font();
            shown.set_label(&family);
            parent_for_pick.pop_subpage();
        });
        let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).margin_top(12).margin_bottom(18).margin_start(18).margin_end(18).build();
        body.append(&search);
        body.append(&rows);
        body.append(&nothing);
        let view = adw::ToolbarView::builder().content(&gtk::ScrolledWindow::builder().child(&adw::Clamp::builder().child(&body).maximum_size(520).build()).hscrollbar_policy(gtk::PolicyType::Never).build()).build();
        view.add_top_bar(&adw::HeaderBar::new());
        parent.push_subpage(&adw::NavigationPage::builder().title("Font").child(&view).build());
        search.grab_focus();
    });
    look.add(&font);
    let size = adw::SpinRow::with_range(6.0, 32.0, 1.0);
    size.set_title("Text Size");
    size.set_value(ui.config.borrow().font_size);
    let weak = Rc::downgrade(ui);
    size.connect_value_notify(move |row| {
        if let Some(ui) = weak.upgrade().filter(|ui| ui.config.borrow().font_size != row.value()) {
            ui.config.borrow_mut().font_size = row.value();
            ui.config.borrow().save();
            ui.apply_font();
        }
    });
    look.add(&size);
    appearance.add(&look);

    // The style in use comes first; the other style's pick is marked too, since Follow System switches to it.
    let dark_now = adw::StyleManager::default().is_dark();
    let order = if dark_now { [("Dark Themes", true), ("Light Themes", false)] } else { [("Light Themes", false), ("Dark Themes", true)] };
    for (title, dark) in order {
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
        let Some(ui) = weak.upgrade() else { return };
        // The same check as the first-run page; a mistake here must not cost the working connection.
        match crate::tunnel::address(&row.text()) {
            Ok(address) => {
                // Try it first: the connection in use is only given up for a server that answers.
                row.set_sensitive(false);
                row.set_title("Checking…");
                let (weak, row, parent) = (Rc::downgrade(&ui), row.clone(), parent.clone());
                ui.probe(&address.clone(), move |problem| {
                    row.set_sensitive(true);
                    // Back in the field, ready to correct the address.
                    row.grab_focus();
                    row.set_position(-1);
                    match (problem, weak.upgrade()) {
                        (None, Some(ui)) => {
                            parent.close();
                            ui.connect(&address);
                        }
                        (problem, _) => {
                            row.add_css_class("error");
                            row.set_title(&problem.unwrap_or_else(|| "Can't reach the server".into()));
                        }
                    }
                });
            }
            Err(problem) => {
                row.add_css_class("error");
                row.set_title(problem);
            }
        }
    });
    url.connect_changed(|row| {
        if row.has_css_class("error") {
            row.remove_css_class("error");
            row.set_title("Add a server: https://…, ssh://user@host or unix:///path");
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
    present(ui, &dialog);
}

pub fn shortcuts(ui: &Rc<Ui>) {
    let page = adw::PreferencesPage::new();
    let row = |group: &adw::PreferencesGroup, label: &str, accelerator: &str| {
        let row = adw::ActionRow::builder().title(label.trim_end_matches('…')).build();
        // Each key is drawn as a key.
        let caps = gtk::Box::builder().spacing(4).valign(gtk::Align::Center).build();
        for key in keys(accelerator).split('+').filter(|key| !key.is_empty()) {
            caps.append(&gtk::Label::builder().label(key).css_classes(["rexling-cap"]).build());
        }
        if accelerator.ends_with("plus") {
            caps.append(&gtk::Label::builder().label("+").css_classes(["rexling-cap"]).build());
        }
        row.add_suffix(&caps);
        group.add(&row);
    };
    let groups: [(&str, &[&str]); 3] = [
        ("Tabs", &["new-tab", "next-tab", "previous-tab", "move-tab-up", "move-tab-down", "rename-tab"]),
        ("Panes", &["split-right", "split-below", "next-pane", "previous-pane", "zoom-pane", "float-pane", "close-pane"]),
        ("Window", &["palette", "next-waiting", "sessions", "new-window", "toggle-sidebar", "preferences", "shortcuts"]),
    ];
    for (title, actions) in groups {
        let group = adw::PreferencesGroup::builder().title(title).build();
        for wanted in actions {
            if let Some((label, _, accelerator)) = ACTIONS.iter().find(|(_, action, accelerator)| action == wanted && !accelerator.is_empty()) {
                row(&group, label, accelerator);
            }
        }
        if title == "Tabs" {
            let numbered = adw::ActionRow::builder().title("Open a Tab by Its Number").build();
            let caps = gtk::Box::builder().spacing(4).valign(gtk::Align::Center).build();
            for key in ["Alt", "1–9"] {
                caps.append(&gtk::Label::builder().label(key).css_classes(["rexling-cap"]).build());
            }
            numbered.add_suffix(&caps);
            group.add(&numbered);
        }
        page.add(&group);
    }
    let terminal = adw::PreferencesGroup::builder().title("Terminal").build();
    for (label, accelerator) in TERMINAL_KEYS {
        row(&terminal, label, accelerator);
    }
    page.add(&terminal);
    let view = adw::ToolbarView::builder().content(&page).build();
    view.add_top_bar(&adw::HeaderBar::new());
    let dialog = adw::Dialog::builder().title("Keyboard Shortcuts").child(&view).content_width(460).content_height(640).build();
    let weak = Rc::downgrade(ui);
    dialog.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.focus_terminal();
        }
    });
    present(ui, &dialog);
}

pub fn about(ui: &Rc<Ui>) {
    let dialog = adw::AboutDialog::builder()
        .application_name("Rexling")
        .application_icon("dev.shulhi.Rexling")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("A native Linux client for Rex")
        .comments("Rexling is an unofficial client for Superlogical Rex, made by its contributors.")
        .website("https://github.com/shulhi/rexling")
        .issue_url("https://github.com/shulhi/rexling/issues")
        .license_type(gtk::License::Apache20)
        .build();
    present(ui, &dialog);
}

/// Asks before ending a session: every program in it stops.
pub fn confirm_close_session(ui: &Rc<Ui>, label: &str, close: impl Fn(&Rc<Ui>) + 'static) {
    let dialog = adw::AlertDialog::builder().heading(format!("Close “{label}”?")).body("Every program in this session will be stopped.").close_response("cancel").default_response("close").build();
    dialog.add_responses(&[("cancel", "Cancel"), ("close", "Close")]);
    dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);
    let weak = Rc::downgrade(ui);
    dialog.connect_response(None, move |_, response| {
        if let (Some(ui), "close") = (weak.upgrade(), response) {
            close(&ui);
        }
    });
    present(ui, &dialog);
}

/// Asks for a name. An empty name is passed on: the server then restores its default.
pub fn rename(ui: &Rc<Ui>, heading: &str, current: &str, apply: impl Fn(&Rc<Ui>, &str) + 'static) {
    ask(ui, heading, current, "Rename", apply);
}

/// Asks for a name. An empty answer is allowed: the server then picks one.
pub fn ask(ui: &Rc<Ui>, heading: &str, current: &str, verb: &str, apply: impl Fn(&Rc<Ui>, &str) + 'static) {
    let entry = gtk::Entry::builder().text(current).activates_default(true).build();
    // Left empty, the server picks a name.
    if current.is_empty() {
        entry.set_placeholder_text(Some("Name (optional)"));
    }
    let dialog = adw::AlertDialog::builder().heading(heading).extra_child(&entry).close_response("cancel").default_response("rename").build();
    dialog.add_responses(&[("cancel", "Cancel"), ("rename", verb)]);
    dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
    let (weak, field) = (Rc::downgrade(ui), entry.clone());
    dialog.connect_response(None, move |_, response| {
        let Some(ui) = weak.upgrade() else { return };
        if response == "rename" {
            apply(&ui, field.text().trim());
        }
        ui.focus_terminal();
    });
    present(ui, &dialog);
    entry.grab_focus();
    entry.select_region(0, -1);
}

/// Lists the remote ports reachable on this computer and adds new ones.
pub fn forwards(ui: &Rc<Ui>) {
    const ASK: &str = "Port on the server";
    // A plain column, so the sheet can be exactly as tall as what is in it.
    let page = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).width_request(440).margin_top(6).margin_bottom(18).margin_start(18).margin_end(18).build();
    let add = adw::PreferencesGroup::builder().description("Reach a port on the server from this computer").build();
    let port = adw::EntryRow::builder().title(ASK).input_purpose(gtk::InputPurpose::Digits).show_apply_button(true).build();
    let weak = Rc::downgrade(ui);
    port.connect_apply(move |row| {
        let Some(ui) = weak.upgrade() else { return };
        let text = row.text();
        let (host, number) = text.trim().rsplit_once(':').unwrap_or(("127.0.0.1", text.trim()));
        let problem = match number.parse::<u16>() {
            Ok(number) if number > 0 && !host.is_empty() => {
                if ui.forwards.borrow().iter().any(|f| f.target == format!("{host}:{number}")) {
                    Some("That port is already forwarded")
                } else {
                    ui.forward(host, number);
                    row.set_text("");
                    None
                }
            }
            _ => Some("Enter a port from 1 to 65535"),
        };
        if let Some(problem) = problem {
            row.add_css_class("error");
            row.set_title(problem);
        }
    });
    port.connect_changed(|row| {
        if row.has_css_class("error") {
            row.remove_css_class("error");
            row.set_title(ASK);
        }
    });
    add.add(&port);
    page.append(&add);
    let active = adw::PreferencesGroup::new();
    page.append(&active);

    let rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::default();
    let refresh: Rc<dyn Fn()> = {
        let (weak, active, rows) = (Rc::downgrade(ui), active.clone(), rows.clone());
        Rc::new(move || {
            let Some(ui) = weak.upgrade() else { return };
            for row in rows.borrow_mut().drain(..) {
                active.remove(&row);
            }
            for forward in ui.forwards.borrow().iter() {
                // "Port 3000 on the server", then where it can be reached here.
                let (host, number) = forward.target.rsplit_once(':').unwrap_or(("", forward.target.as_str()));
                let title = if host == "127.0.0.1" { format!("Port {number} on the server") } else { format!("{} from the server", forward.target) };
                let status = match forward.phase.as_str() {
                    "active" => format!("{} on this computer", forward.address.replace("127.0.0.1", "localhost")),
                    "failed" if forward.error.is_empty() || forward.error.contains("is unavailable") => "No local port is free".to_owned(),
                    "failed" => crate::tunnel::explain(&forward.error),
                    _ => "Connecting…".to_owned(),
                };
                let row = adw::ActionRow::builder().title(title).subtitle(status).build();
                if forward.phase == "active" {
                    let open = gtk::Button::builder().label("Open").valign(gtk::Align::Center).css_classes(["flat"]).build();
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

    let view = adw::ToolbarView::builder().content(&gtk::ScrolledWindow::builder().child(&page).hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_height(true).propagate_natural_width(true).max_content_height(460).build()).build();
    view.add_top_bar(&adw::HeaderBar::new());
    let dialog = adw::Dialog::builder().title("Port Forwards").child(&view).follows_content_size(true).build();
    let weak = Rc::downgrade(ui);
    dialog.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.forwards_changed.replace(None);
            ui.focus_terminal();
        }
    });
    present(ui, &dialog);
    port.grab_focus();
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn palette_ranks_names_before_kinds_and_initials() {
        assert_eq!(score("Split Right", "", "split"), Some(0));
        assert_eq!(score("Split Right", "", "right"), Some(1));
        assert_eq!(score("Split Right", "", "sr"), Some(4));
        assert_eq!(score("Cathode Amber", "Dark theme", "theme"), Some(3), "searching a kind lists everything of that kind");
        assert_eq!(score("Alpine Milk", "Light theme", "light"), Some(3), "light themes are found whatever they are called");
        assert_eq!(score("Split Right", "", "light"), None, "scattered letters do not match");
        assert_eq!(score("New Tab", "", "split"), None);
    }
}
