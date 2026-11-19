//! The main window: sessions and tabs in a sidebar, the current tab's panes beside it.
use crate::{
    client::{Activity, Client, Control, State, Update},
    config::Config,
    dialogs,
    panegrid::PaneGrid,
    programs,
    terminal::{Event, Terminal},
    theme, tiles,
    tunnel::{self, Tunnel},
};
use adw::prelude::*;
use gtk::{gdk, gio, pango};
use rex_core::ghostty::Frame;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

/// Connections kept for panes that are not on screen, so going back to a tab is instant.
const SPARE_PANES: usize = 8;

struct Pane {
    client: Rc<Client>,
    terminal: Terminal,
    frame: Rc<RefCell<Option<Arc<Frame>>>>,
}

/// A remote port made reachable on this computer.
pub struct Forward {
    pub handle: u64,
    host: String,
    port: u16,
    /// Whether the same local port was busy and another was picked.
    moved: bool,
    pub target: String,
    pub phase: String,
    pub address: String,
    pub error: String,
}

pub struct Ui {
    pub window: adw::ApplicationWindow,
    pub config: RefCell<Config>,
    control: Control,
    pub chrome: Rc<Client>,
    /// The address connections use: the server itself, or the local end of its SSH tunnel.
    endpoint: RefCell<String>,
    tunnel: RefCell<Option<Tunnel>>,
    /// Why the server could not be reached, when the failure happened before the engine was involved.
    problem: RefCell<String>,
    pub forwards: RefCell<Vec<Forward>>,
    pub forwards_changed: RefCell<Option<Rc<dyn Fn()>>>,
    panes: RefCell<HashMap<String, Pane>>,
    /// Panes by how recently they were shown, oldest first.
    recent: RefCell<Vec<String>>,
    visible: RefCell<Vec<String>>,
    grid: PaneGrid,
    state: RefCell<Rc<State>>,
    activity: RefCell<Rc<Vec<Activity>>>,
    notified: RefCell<Option<HashSet<String>>>,
    tab: RefCell<String>,
    focused: RefCell<String>,
    zoomed: Cell<bool>,
    /// The pane the server last moved the sessions connection to: a tab or split it just created.
    created: RefCell<String>,
    styles: gtk::CssProvider,
    toasts: adw::ToastOverlay,
    split: adw::OverlaySplitView,
    stack: gtk::Stack,
    banner: adw::Banner,
    title: adw::WindowTitle,
    session_button: gtk::MenuButton,
    session_label: gtk::Label,
    session_dot: gtk::Box,
    session_list: gtk::ListBox,
    tab_list: gtk::ListBox,
    latest: gtk::Button,
    server_entry: adw::EntryRow,
    shown_sessions: RefCell<Vec<(String, String, bool, String)>>,
    shown_tabs: RefCell<Vec<TabRow>>,
    was_connected: Cell<bool>,
    notice: RefCell<String>,
}

#[derive(PartialEq, Clone)]
struct TabRow {
    id: String,
    title: String,
    detail: String,
    tile: String,
    status: String,
    status_class: &'static str,
    current: bool,
}

/// A palette entry: what it is called, a hint beside it, and what it does.
pub type Command = (String, String, Box<dyn Fn(&Rc<Ui>)>);

pub fn home_relative(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path == home => "~".into(),
        Ok(home) if !home.is_empty() && path.starts_with(&format!("{home}/")) => format!("~{}", &path[home.len()..]),
        _ => path.to_owned(),
    }
}

fn icon_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
    gtk::Button::builder().icon_name(icon).tooltip_text(tooltip).action_name(action).build()
}

fn scrolled_back(frame: &Frame) -> bool {
    frame.scroll_total.saturating_sub(frame.scroll_visible) > frame.scroll_offset
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map(|first| first.to_uppercase().chain(characters).collect()).unwrap_or_default()
}

pub fn build(app: &adw::Application) {
    let control: Control = Rc::new(Cell::new(false));
    let chrome = match Client::new(&control) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("rexling: could not start the terminal engine: {error}");
            return;
        }
    };
    let config = Config::load();
    chrome.remembered_session.replace(config.sessions.get(&config.server).cloned().unwrap_or_default());

    // Sidebar: the session switcher above the session's tabs.
    let session_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["navigation-sidebar"]).build();
    let new_session = gtk::Entry::builder().placeholder_text("New session").margin_top(6).margin_start(6).margin_end(6).margin_bottom(6).build();
    let session_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).width_request(260).build();
    session_box.append(&session_list);
    session_box.append(&new_session);
    let session_popover = gtk::Popover::builder().child(&session_box).build();
    let session_label = gtk::Label::builder().label("Sessions").css_classes(["rexling-session"]).ellipsize(pango::EllipsizeMode::End).build();
    let session_dot = gtk::Box::builder().css_classes(["rexling-dot"]).valign(gtk::Align::Center).visible(false).build();
    let session_content = gtk::Box::builder().spacing(8).build();
    session_content.append(&session_label);
    session_content.append(&session_dot);
    session_content.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
    let session_button = gtk::MenuButton::builder().child(&session_content).popover(&session_popover).tooltip_text("Sessions").css_classes(["flat"]).build();
    let sidebar_header = adw::HeaderBar::builder().show_title(false).show_end_title_buttons(false).show_start_title_buttons(false).build();
    sidebar_header.pack_start(&session_button);
    sidebar_header.pack_end(&icon_button("tab-new-symbolic", "New Tab", "win.new-tab"));
    let tab_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Single).css_classes(["navigation-sidebar", "rexling-tabs"]).build();
    let sidebar = adw::ToolbarView::builder().content(&gtk::ScrolledWindow::builder().child(&tab_list).hscrollbar_policy(gtk::PolicyType::Never).build()).build();
    sidebar.add_top_bar(&sidebar_header);

    // Content: the focused pane's title above the tab's panes.
    let title = adw::WindowTitle::new("Rexling", "");
    let header = adw::HeaderBar::builder().title_widget(&title).build();
    header.pack_start(&icon_button("sidebar-show-symbolic", "Sidebar", "win.toggle-sidebar"));
    let menu = gio::Menu::new();
    let commands = gio::Menu::new();
    commands.append(Some("Command Palette"), Some("win.palette"));
    menu.append_section(None, &commands);
    let application = gio::Menu::new();
    application.append(Some("Preferences"), Some("win.preferences"));
    application.append(Some("Keyboard Shortcuts"), Some("win.shortcuts"));
    application.append(Some("About Rexling"), Some("win.about"));
    menu.append_section(None, &application);
    header.pack_end(&gtk::MenuButton::builder().icon_name("open-menu-symbolic").tooltip_text("Menu").menu_model(&menu).primary(true).build());
    header.pack_end(&icon_button("view-paged-symbolic", "Split Below", "win.split-below"));
    header.pack_end(&icon_button("view-dual-symbolic", "Split Right", "win.split-right"));

    let banner = adw::Banner::new("");
    let latest = gtk::Button::builder()
        .icon_name("go-bottom-symbolic")
        .tooltip_text("Latest")
        .css_classes(["circular", "osd"])
        .halign(gtk::Align::End)
        .valign(gtk::Align::End)
        .margin_end(16)
        .margin_bottom(16)
        .visible(false)
        .build();
    let grid = PaneGrid::default();
    let overlay = gtk::Overlay::builder().child(&grid).build();
    overlay.add_overlay(&latest);

    let server_entry = adw::EntryRow::builder().title("Server URL").text(config.server.as_str()).show_apply_button(true).build();
    let server_group = adw::PreferencesGroup::new();
    server_group.add(&server_entry);
    let connect = adw::StatusPage::builder()
        .icon_name("dev.shulhi.Rexling")
        .title("Connect to Rex")
        .child(&adw::Clamp::builder().maximum_size(420).child(&server_group).build())
        .build();
    let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).transition_duration(160).build();
    stack.add_named(&connect, Some("connect"));
    stack.add_named(&overlay, Some("terminal"));
    let empty = adw::StatusPage::builder()
        .icon_name("dev.shulhi.Rexling")
        .title("No Sessions")
        .child(&gtk::Button::builder().label("New Session").action_name("win.new-session").halign(gtk::Align::Center).css_classes(["pill", "suggested-action"]).build())
        .build();
    stack.add_named(&empty, Some("empty"));

    let content = adw::ToolbarView::builder().content(&stack).build();
    content.add_top_bar(&header);
    content.add_top_bar(&banner);

    let split = adw::OverlaySplitView::builder().sidebar(&sidebar).content(&content).min_sidebar_width(230.0).max_sidebar_width(300.0).build();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));
    let window = adw::ApplicationWindow::builder().application(app).title("Rexling").default_width(1240).default_height(800).content(&toasts).build();
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth, 760.0, adw::LengthUnit::Sp));
    narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(narrow);

    let styles = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(&gdk::Display::default().expect("display"), &styles, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);

    let ui = Rc::new(Ui {
        window: window.clone(),
        config: RefCell::new(config),
        control,
        chrome: chrome.clone(),
        endpoint: RefCell::default(),
        tunnel: RefCell::default(),
        problem: RefCell::default(),
        forwards: RefCell::default(),
        forwards_changed: RefCell::default(),
        panes: RefCell::default(),
        recent: RefCell::default(),
        visible: RefCell::default(),
        grid,
        state: RefCell::default(),
        activity: RefCell::default(),
        notified: RefCell::default(),
        tab: RefCell::default(),
        focused: RefCell::default(),
        zoomed: Cell::new(false),
        created: RefCell::default(),
        styles,
        toasts,
        split,
        stack,
        banner,
        title,
        session_button,
        session_label,
        session_dot,
        session_list,
        tab_list,
        latest,
        server_entry,
        shown_sessions: RefCell::default(),
        shown_tabs: RefCell::default(),
        was_connected: Cell::new(false),
        notice: RefCell::default(),
    });
    ui.apply_appearance();
    ui.actions();

    let weak = Rc::downgrade(&ui);
    ui.banner.connect_button_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            if !ui.problem.borrow().is_empty() {
                let server = ui.config.borrow().server.clone();
                ui.connect(&server);
            } else if ui.chrome.state().connected() {
                ui.interact()
            } else {
                ui.chrome.retry()
            }
            ui.focus_terminal();
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.latest.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            if let Some(pane) = ui.panes.borrow().get(&*ui.focused.borrow()) {
                pane.client.scroll_to_bottom();
            }
            ui.focus_terminal();
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.server_entry.connect_apply(move |entry| {
        if let Some(ui) = weak.upgrade() {
            ui.connect(entry.text().trim());
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.tab_list.connect_row_activated(move |_, row| {
        if let Some(ui) = weak.upgrade() {
            ui.interact();
            ui.open_tab(&row.widget_name());
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.session_list.connect_row_activated(move |_, row| {
        if let Some(ui) = weak.upgrade() {
            ui.session_button.popdown();
            ui.open_session(&row.widget_name());
        }
    });
    let weak = Rc::downgrade(&ui);
    new_session.connect_activate(move |entry| {
        if let Some(ui) = weak.upgrade() {
            ui.interact();
            ui.chrome.new_session(entry.text().trim());
            entry.set_text("");
            ui.session_button.popdown();
        }
    });
    let weak = Rc::downgrade(&ui);
    adw::StyleManager::default().connect_dark_notify(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.apply_theme();
        }
    });
    let weak = Rc::downgrade(&ui);
    chrome.watch(move |update| {
        if let Some(ui) = weak.upgrade() {
            ui.update(update);
        }
    });

    let weak = Rc::downgrade(&ui);
    ui.grid.connect_resize(move |split, ratio| {
        if let Some(ui) = weak.upgrade() {
            ui.interact();
            ui.chrome.resize_split(split, ratio);
        }
    });

    // With no server chosen yet, a Rex server on this computer needs no address at all.
    let server = Some(ui.config.borrow().server.clone()).filter(|s| !s.is_empty()).or_else(tunnel::local_server);
    match server {
        Some(server) => ui.connect(&server),
        None => ui.stack.set_visible_child_name("connect"),
    }
    window.present();
    // The window owns the UI state for as long as it is open.
    window.connect_destroy(move |_| {
        let _ = &ui;
    });
}

impl Ui {
    fn actions(self: &Rc<Self>) {
        let add = |name: &str, run: fn(&Rc<Ui>)| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(ui) = weak.upgrade() {
                    run(&ui);
                }
            });
            self.window.add_action(&action);
        };
        add("new-tab", |ui| {
            ui.interact();
            ui.chrome.new_tab();
        });
        add("split-right", |ui| ui.split_pane(false));
        add("split-below", |ui| ui.split_pane(true));
        add("close-pane", |ui| {
            let focused = ui.focused.borrow().clone();
            ui.chrome.close_pane(&focused);
        });
        add("close-tab", |ui| {
            let tab = ui.tab.borrow().clone();
            ui.chrome.close_tab(&tab);
        });
        add("close-session", |ui| {
            let state = ui.state.borrow().clone();
            let id = state.selected_session.clone();
            if !id.is_empty() {
                dialogs::confirm_close_session(ui, state.session_label(), move |ui| ui.chrome.close_session(&id));
            }
        });
        add("new-session", |ui| {
            ui.interact();
            ui.chrome.new_session("");
        });
        add("rename-tab", |ui| {
            let (tab, state) = (ui.tab.borrow().clone(), ui.state.borrow().clone());
            let Some(current) = state.windows.iter().find(|t| t.id == tab) else { return };
            let label = current.name().unwrap_or("").to_owned();
            dialogs::rename(ui, "Rename Tab", &label, move |ui, name| ui.chrome.rename_tab(&tab, name));
        });
        add("rename-session", |ui| {
            let state = ui.state.borrow().clone();
            let id = state.selected_session.clone();
            if !id.is_empty() {
                dialogs::rename(ui, "Rename Session", state.session_label(), move |ui, name| ui.chrome.rename_session(&id, name));
            }
        });
        add("move-tab-up", |ui| ui.move_tab(-1));
        add("move-tab-down", |ui| ui.move_tab(1));
        add("forwards", dialogs::forwards);
        add("attach-image", |ui| ui.choose_image());
        add("next-tab", |ui| ui.step_tab(1));
        add("previous-tab", |ui| ui.step_tab(-1));
        add("next-pane", |ui| ui.step_pane(1));
        add("previous-pane", |ui| ui.step_pane(-1));
        add("zoom-pane", |ui| {
            ui.zoomed.set(!ui.zoomed.get());
            ui.arrange();
        });
        add("sessions", |ui| ui.session_button.popup());
        add("toggle-sidebar", |ui| ui.split.set_show_sidebar(!ui.split.shows_sidebar()));
        add("palette", dialogs::palette);
        add("preferences", dialogs::preferences);
        add("shortcuts", dialogs::shortcuts);
        add("about", dialogs::about);
    }

    /// Every command the palette offers that is not an action: sessions, tabs and themes.
    pub fn destinations(self: &Rc<Self>) -> Vec<Command> {
        let state = self.state.borrow().clone();
        let mut items: Vec<Command> = Vec::new();
        for row in self.shown_tabs.borrow().iter() {
            let id = row.id.clone();
            items.push((row.title.clone(), "Tab".into(), Box::new(move |ui| ui.open_tab(&id))));
        }
        for session in &state.sessions {
            let id = session.session_id.clone();
            items.push((session.label.clone(), "Session".into(), Box::new(move |ui| ui.open_session(&id))));
        }
        for server in self.config.borrow().servers.iter().filter(|s| **s != self.config.borrow().server) {
            let address = server.clone();
            items.push((tunnel::label(server), "Server".into(), Box::new(move |ui| ui.connect(&address))));
        }
        for theme in theme::all() {
            let id = theme.id.clone();
            items.push((theme.name.clone(), "Theme".into(), Box::new(move |ui| ui.choose_theme(&id))));
        }
        items
    }

    // --- Appearance ---------------------------------------------------------------------------

    pub fn theme(&self) -> &'static theme::Theme {
        let config = self.config.borrow();
        let dark = adw::StyleManager::default().is_dark();
        theme::find(if dark { &config.theme_dark } else { &config.theme_light }, dark)
    }
    /// Applies the chosen brightness, then the theme for it.
    pub fn apply_appearance(&self) {
        let scheme = match self.config.borrow().appearance.as_str() {
            "dark" => adw::ColorScheme::ForceDark,
            "light" => adw::ColorScheme::ForceLight,
            _ => adw::ColorScheme::Default,
        };
        adw::StyleManager::default().set_color_scheme(scheme);
        self.apply_theme();
    }
    fn apply_theme(&self) {
        let palette = self.theme().palette;
        self.styles.load_from_string(&theme::css(&palette));
        for pane in self.panes.borrow().values() {
            pane.terminal.set_palette(palette);
            pane.client.set_theme(palette.theme());
        }
    }
    /// Picks a theme. Choosing one of the other brightness switches the window to that brightness too.
    pub fn choose_theme(&self, id: &str) {
        let Some(chosen) = theme::all().iter().find(|t| t.id == id) else { return };
        {
            let mut config = self.config.borrow_mut();
            if chosen.palette.dark {
                config.theme_dark = id.to_owned();
            } else {
                config.theme_light = id.to_owned();
            }
            if chosen.palette.dark != adw::StyleManager::default().is_dark() {
                config.appearance = if chosen.palette.dark { "dark" } else { "light" }.into();
            }
            config.save();
        }
        self.apply_appearance();
    }
    pub fn apply_font(&self) {
        let (font, size) = {
            let config = self.config.borrow();
            (config.font.clone(), config.font_size)
        };
        for pane in self.panes.borrow().values() {
            pane.terminal.set_font(&font, size);
        }
    }

    // --- Connection and control ---------------------------------------------------------------

    pub fn connect(self: &Rc<Self>, server: &str) {
        let server = server.trim().to_owned();
        if server.is_empty() {
            return;
        }
        {
            let mut config = self.config.borrow_mut();
            config.server = server.clone();
            config.servers.retain(|s| *s != server);
            config.servers.insert(0, server.clone());
            config.save();
            self.chrome.remembered_session.replace(config.sessions.get(&server).cloned().unwrap_or_default());
        }
        // Nothing from the previous server may linger: its panes, its tunnel, or its last state.
        self.tunnel.replace(None);
        self.endpoint.replace(String::new());
        self.problem.replace(String::new());
        self.state.replace(Rc::default());
        self.tab.replace(String::new());
        self.focused.replace(String::new());
        self.panes.borrow_mut().clear();
        self.recent.borrow_mut().clear();
        self.show_sessions();
        self.show_tabs();
        self.arrange();
        self.stack.set_visible_child_name("terminal");
        if !server.starts_with("ssh://") {
            self.attach(&server);
            return;
        }
        self.banner.set_title("Opening SSH tunnel…");
        self.banner.set_button_label(None);
        self.banner.set_revealed(true);
        let (tx, rx) = async_channel::bounded(1);
        let address = server.clone();
        std::thread::spawn(move || {
            let _ = tx.send_blocking(tunnel::open(&address));
        });
        let weak = Rc::downgrade(self);
        gtk::glib::spawn_future_local(async move {
            let (Ok(result), Some(ui)) = (rx.recv().await, weak.upgrade()) else { return };
            // The user may have picked another server while this one was connecting.
            if ui.config.borrow().server != server {
                return;
            }
            match result {
                Ok((tunnel, endpoint)) => {
                    ui.tunnel.replace(Some(tunnel));
                    ui.attach(&endpoint);
                }
                Err(error) => {
                    ui.problem.replace(error.to_string());
                    ui.refresh_banner();
                }
            }
        });
    }
    fn attach(&self, endpoint: &str) {
        self.endpoint.replace(endpoint.to_owned());
        self.chrome.connect(endpoint);
    }
    /// Whether `state` is the current server's, and connected.
    fn live(&self, state: &State) -> bool {
        let endpoint = self.endpoint.borrow();
        state.connected() && !endpoint.is_empty() && state.endpoint.trim_end_matches('/') == endpoint.trim_end_matches('/')
    }
    pub fn server_label(&self) -> String {
        tunnel::label(&self.config.borrow().server)
    }

    /// The user acted on a terminal or navigated: from here this window holds the keyboard for the panes it shows.
    fn interact(&self) {
        if !self.control.replace(true) {
            for id in self.visible.borrow().iter() {
                if let Some(pane) = self.panes.borrow().get(id) {
                    pane.client.claim();
                }
            }
            self.refresh_banner();
        }
    }

    pub fn focus_terminal(&self) {
        if let Some(pane) = self.panes.borrow().get(&*self.focused.borrow()) {
            pane.terminal.focus();
        }
    }

    // --- Navigation ---------------------------------------------------------------------------

    pub fn open_session(self: &Rc<Self>, id: &str) {
        self.interact();
        self.tab.replace(String::new());
        self.chrome.select_session(id);
    }
    pub fn open_tab(self: &Rc<Self>, id: &str) {
        if *self.tab.borrow() != id {
            self.tab.replace(id.to_owned());
            self.focused.replace(String::new());
            self.zoomed.set(false);
            self.show_tabs();
        }
        self.arrange();
        self.focus_terminal();
    }
    fn step_tab(self: &Rc<Self>, step: isize) {
        let state = self.state.borrow().clone();
        let count = state.windows.len() as isize;
        if let Some(current) = state.windows.iter().position(|tab| tab.id == *self.tab.borrow()).filter(|_| count > 1) {
            self.interact();
            self.open_tab(&state.windows[(current as isize + step).rem_euclid(count) as usize].id.clone());
        }
    }
    fn step_pane(self: &Rc<Self>, step: isize) {
        let state = self.state.borrow().clone();
        let Some(tab) = state.windows.iter().find(|tab| tab.id == *self.tab.borrow()) else { return };
        let order: Vec<String> = tab.blocks.iter().map(|b| b.id.clone()).collect();
        let count = order.len() as isize;
        if let Some(current) = order.iter().position(|id| *id == *self.focused.borrow()).filter(|_| count > 1) {
            self.interact();
            self.focus_pane(&order[(current as isize + step).rem_euclid(count) as usize]);
        }
    }
    fn focus_pane(self: &Rc<Self>, block: &str) {
        self.focused.replace(block.to_owned());
        self.arrange();
        self.focus_terminal();
    }
    fn split_pane(self: &Rc<Self>, below: bool) {
        self.interact();
        self.zoomed.set(false);
        let focused = self.focused.borrow().clone();
        self.chrome.split(&focused, below);
    }

    fn move_tab(self: &Rc<Self>, step: isize) {
        let state = self.state.borrow().clone();
        let tab = self.tab.borrow().clone();
        let Some(at) = state.windows.iter().position(|t| t.id == tab) else { return };
        let target = at as isize + step;
        if target < 0 || target >= state.windows.len() as isize {
            return;
        }
        // Moving down means landing before the tab after the next one.
        let before = if step < 0 { state.windows.get(target as usize) } else { state.windows.get(target as usize + 1) };
        self.chrome.move_tab(&tab, before.map(|t| t.id.as_str()));
    }

    // --- Images and port forwards ---------------------------------------------------------------

    /// Sends a file to the focused pane; Rex types its path there once it has arrived.
    pub fn upload(self: &Rc<Self>, block: &str, path: &std::path::Path) {
        let state = self.state.borrow().clone();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "image".into());
        let request = serde_json::json!({"endpoint": *self.endpoint.borrow(), "session": state.selected_session, "block": block,
            "path": path.to_string_lossy(), "name": name, "ref": format!("upload:{}", gtk::glib::uuid_string_random())});
        let handle = match rex_engine::uploads::create(request) {
            Ok(handle) => handle,
            Err(error) => {
                self.toasts.add_toast(adw::Toast::new(&format!("Could not send {name}: {error}")));
                return;
            }
        };
        self.interact();
        let toast = adw::Toast::builder().title(format!("Sending {name}…")).timeout(0).build();
        self.toasts.add_toast(toast.clone());
        let weak = Rc::downgrade(self);
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(60), move || {
            let status = rex_engine::uploads::poll(handle);
            let phase = status["phase"].as_str().unwrap_or("failed");
            if !matches!(phase, "done" | "unsupported" | "failed" | "uncertain" | "canceled") {
                return gtk::glib::ControlFlow::Continue;
            }
            rex_engine::uploads::destroy(handle);
            toast.dismiss();
            if let Some(ui) = weak.upgrade().filter(|_| phase != "done") {
                let reason = match (phase, status["error"].as_str().unwrap_or("")) {
                    ("unsupported", _) => "this server does not accept uploads".to_owned(),
                    (_, "") => "the upload did not finish".to_owned(),
                    (_, error) => error.to_owned(),
                };
                ui.toasts.add_toast(adw::Toast::new(&format!("Could not send {name}: {reason}")));
            }
            gtk::glib::ControlFlow::Break
        });
    }
    fn choose_image(self: &Rc<Self>) {
        let block = self.focused.borrow().clone();
        if block.is_empty() {
            return;
        }
        let weak = Rc::downgrade(self);
        gtk::FileDialog::builder().title("Attach Image").modal(true).build().open(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if let (Some(ui), Ok(file)) = (weak.upgrade(), result)
                && let Some(path) = file.path()
            {
                ui.upload(&block, &path);
            }
        });
    }

    fn start_forward(&self, host: &str, port: u16, local: u16) -> anyhow::Result<u64> {
        let state = self.state.borrow().clone();
        rex_engine::forwards::create(serde_json::json!({"id": format!("forward-{port}-{:x}", gtk::glib::monotonic_time()), "endpoint": *self.endpoint.borrow(),
            "session": state.selected_session, "host": host, "port": port, "local_port": local}))
    }
    /// Makes `host:port` on the server reachable on this computer, on the same port number when it is free.
    pub fn forward(self: &Rc<Self>, host: &str, port: u16) {
        match self.start_forward(host, port, port) {
            Ok(handle) => {
                self.forwards.borrow_mut().push(Forward { handle, host: host.to_owned(), port, moved: false, target: format!("{host}:{port}"), phase: "connecting".into(), address: String::new(), error: String::new() });
                self.watch_forwards();
            }
            Err(error) => self.toasts.add_toast(adw::Toast::new(&format!("Could not forward port {port}: {error}"))),
        }
    }
    pub fn stop_forward(&self, handle: u64) {
        rex_engine::forwards::destroy(handle);
        self.forwards.borrow_mut().retain(|f| f.handle != handle);
        self.forwards_updated();
    }
    fn forwards_updated(&self) {
        let handler = self.forwards_changed.borrow().clone();
        if let Some(handler) = handler {
            handler();
        }
    }
    fn watch_forwards(self: &Rc<Self>) {
        self.forwards_updated();
        if self.forwards.borrow().len() != 1 {
            return; // The timer below is already running.
        }
        let weak = Rc::downgrade(self);
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
            let Some(ui) = weak.upgrade() else { return gtk::glib::ControlFlow::Break };
            let mut changed = false;
            for forward in ui.forwards.borrow_mut().iter_mut() {
                let status = rex_engine::forwards::poll(forward.handle);
                // The same port number is taken here: let the system pick another, once.
                if status["port_unavailable"] == true && !forward.moved {
                    forward.moved = true;
                    if let Ok(handle) = ui.start_forward(&forward.host, forward.port, 0) {
                        rex_engine::forwards::destroy(forward.handle);
                        forward.handle = handle;
                        continue;
                    }
                }
                let text = |key: &str| status[key].as_str().unwrap_or("").to_owned();
                let next = (status["phase"].as_str().unwrap_or("failed").to_owned(), text("address"), text("error"));
                if (forward.phase.clone(), forward.address.clone(), forward.error.clone()) != next {
                    (forward.phase, forward.address, forward.error) = next;
                    changed = true;
                }
            }
            if changed {
                ui.forwards_updated();
            }
            if ui.forwards.borrow().is_empty() { gtk::glib::ControlFlow::Break } else { gtk::glib::ControlFlow::Continue }
        });
    }

    // --- State from the server ----------------------------------------------------------------

    fn update(self: &Rc<Self>, update: Update) {
        if let Some(activity) = update.activity {
            self.activity.replace(activity);
            self.notify();
            self.shown_tabs.borrow_mut().clear();
            self.shown_sessions.borrow_mut().clear();
        } else if !update.state_changed {
            return;
        }
        let state = update.state;
        // A late update from the server that was just left says nothing about the new one.
        if state.connected() && !self.live(&state) {
            return;
        }
        self.state.replace(state.clone());

        let connected = self.live(&state);
        if connected && !self.was_connected.replace(true) {
            self.chrome.watch_activity();
        } else if !connected {
            self.was_connected.set(false);
        }
        if let Some(text) = self.chrome.take_clipboard(&state) {
            self.window.clipboard().set_text(&text);
        }
        // Control changes and closed or created panes are already visible, so only other notices are toasted.
        let notice = state.notice.clone();
        if self.notice.replace(notice.clone()) != notice && !notice.is_empty() && !["control", "closed", "created"].iter().any(|word| notice.contains(word)) {
            self.toasts.add_toast(adw::Toast::new(&notice));
        }

        // The sessions connection lands on a pane only when the server just created it.
        if !state.selected_block.is_empty() && self.created.replace(state.selected_block.clone()) != state.selected_block
            && let Some(tab) = state.tab_of(&state.selected_block) {
                self.tab.replace(tab.id.clone());
                self.focused.replace(state.selected_block.clone());
                self.zoomed.set(false);
            }
        if connected && !state.windows.iter().any(|tab| tab.id == *self.tab.borrow()) {
            let remembered = self.config.borrow().tabs.get(&state.selected_session).cloned().unwrap_or_default();
            let tab = state.windows.iter().find(|tab| tab.id == remembered).or(state.windows.first());
            self.tab.replace(tab.map(|tab| tab.id.clone()).unwrap_or_default());
        }

        if !self.config.borrow().server.is_empty() {
            self.stack.set_visible_child_name(if connected && state.sessions.is_empty() { "empty" } else { "terminal" });
        }
        self.show_sessions();
        self.show_tabs();
        self.arrange();
        self.refresh_banner();
    }

    fn refresh_banner(&self) {
        let state = self.state.borrow().clone();
        // One status line: a problem, a connection in progress, or that this window is only watching.
        let problem = self.problem.borrow().clone();
        let (text, button) = if !problem.is_empty() {
            (problem, Some("Retry"))
        } else if self.endpoint.borrow().is_empty() && !self.config.borrow().server.is_empty() {
            ("Opening SSH tunnel…".to_owned(), None)
        } else if !state.error.is_empty() {
            (state.error.clone(), Some("Retry"))
        } else if !state.connected() && !state.phase.is_empty() && state.phase != "disconnected" {
            (format!("{}…", capitalised(&state.phase)), None)
        } else if state.connected() && !self.visible.borrow().is_empty() && !self.control.get() {
            ("Viewing only".to_owned(), Some("Take Control"))
        } else {
            (String::new(), None)
        };
        self.banner.set_title(&text);
        self.banner.set_button_label(button);
        self.banner.set_revealed(!text.is_empty());
    }

    /// The most pressing thing any pane in `blocks` reported: a label and its style class.
    fn status(&self, blocks: &[&str], working: &[String]) -> (String, &'static str) {
        let activity = self.activity.borrow();
        let reports: Vec<&Activity> = activity.iter().filter(|a| blocks.contains(&a.block.as_str())).collect();
        for (state, class) in [("blocked", "attention"), ("failed", "failed"), ("finished", "attention")] {
            if let Some(report) = reports.iter().find(|a| a.state == state) {
                return (report.label().to_owned(), class);
            }
        }
        if blocks.iter().any(|b| working.iter().any(|w| w == b)) || reports.iter().any(|a| a.state == "working") {
            return ("Working".to_owned(), "working");
        }
        (String::new(), "")
    }

    fn show_sessions(&self) {
        let state = self.state.borrow().clone();
        let activity = self.activity.borrow().clone();
        let attention = |session: &str| {
            let reports = activity.iter().filter(|a| a.session == session && a.needs_attention());
            reports.map(|a| if a.state == "failed" { "failed" } else { "attention" }).max().unwrap_or("").to_owned()
        };
        let label = state.session_label();
        self.session_label.set_label(if label.is_empty() { "Sessions" } else { label });
        self.session_button.set_tooltip_text(Some(&format!("Sessions on {}", self.server_label())));
        // The switcher carries a dot when another session wants the user.
        let elsewhere = state.sessions.iter().any(|s| s.session_id != state.selected_session && !attention(&s.session_id).is_empty());
        self.session_dot.set_visible(elsewhere);

        let next: Vec<_> = state.sessions.iter().map(|s| (s.session_id.clone(), s.label.clone(), s.session_id == state.selected_session, attention(&s.session_id))).collect();
        if *self.shown_sessions.borrow() == next {
            return;
        }
        self.session_list.remove_all();
        for (id, label, current, attention) in &next {
            let row = gtk::Box::builder().spacing(10).margin_top(4).margin_bottom(4).build();
            row.append(&gtk::Label::builder().label(label.as_str()).xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).build());
            if !attention.is_empty() {
                row.append(&gtk::Box::builder().css_classes(["rexling-dot", attention.as_str()]).valign(gtk::Align::Center).build());
            }
            if *current {
                row.append(&gtk::Image::from_icon_name("object-select-symbolic"));
            }
            self.session_list.append(&gtk::ListBoxRow::builder().child(&row).name(id.as_str()).build());
        }
        self.shown_sessions.replace(next);
    }

    fn show_tabs(&self) {
        let state = self.state.borrow().clone();
        let current_tab = self.tab.borrow().clone();
        let config = self.config.borrow();
        let next: Vec<TabRow> = state
            .windows
            .iter()
            .map(|tab| {
                let current = tab.id == current_tab;
                let focused = if current { self.focused.borrow().clone() } else { config.panes.get(&tab.id).cloned().unwrap_or_default() };
                let lead = tab.blocks.iter().find(|b| b.id == focused).or(tab.blocks.first());
                let title = tab.name().map(str::to_owned).or_else(|| lead.map(|b| state.title(b))).unwrap_or_else(|| tab.label.clone());
                let directory = lead.and_then(|b| state.directories.get(&b.id)).map(|d| home_relative(d)).unwrap_or_default();
                let detail = match tab.blocks.len() {
                    // A shell is already titled by its folder; the full path only helps when it says more.
                    0 | 1 if directory == title => String::new(),
                    0 | 1 => directory,
                    count if directory.is_empty() => format!("{count} panes"),
                    count => format!("{count} panes · {directory}"),
                };
                let blocks: Vec<&str> = tab.blocks.iter().map(|b| b.id.as_str()).collect();
                let (status, status_class) = self.status(&blocks, &state.working);
                let tile = lead.map(|b| programs::tile(state.programs.get(&b.id).map_or(&[][..], |p| p.as_slice()), tiles::exists)).unwrap_or_else(|| "shell".into());
                TabRow { id: tab.id.clone(), title, detail, tile, status, status_class, current }
            })
            .collect();
        drop(config);
        if *self.shown_tabs.borrow() == next {
            return;
        }
        self.tab_list.remove_all();
        let scale = self.window.scale_factor().max(1);
        for row in &next {
            let text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).hexpand(true).valign(gtk::Align::Center).build();
            text.append(&gtk::Label::builder().label(row.title.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["rexling-tab-title"]).build());
            if !row.status.is_empty() {
                text.append(&gtk::Label::builder().label(row.status.as_str()).xalign(0.0).css_classes(["rexling-status", row.status_class]).build());
            } else if !row.detail.is_empty() {
                text.append(&gtk::Label::builder().label(row.detail.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::Start).css_classes(["caption", "dim-label"]).build());
            }
            let content = gtk::Box::builder().spacing(10).build();
            let picture = gtk::Image::builder().pixel_size(30).valign(gtk::Align::Center).build();
            if let Some(texture) = tiles::texture(&row.tile, 30 * scale) {
                picture.set_paintable(Some(&texture));
            }
            content.append(&picture);
            content.append(&text);
            if row.status_class == "working" {
                content.append(&adw::Spinner::builder().valign(gtk::Align::Center).build());
            }
            let list_row = gtk::ListBoxRow::builder().child(&content).name(row.id.as_str()).tooltip_text(row.detail.as_str()).build();
            // Right-click: the tab's own commands, applied to that tab.
            let menu = gio::Menu::new();
            menu.append(Some("Rename…"), Some("win.rename-tab"));
            menu.append(Some("Move Up"), Some("win.move-tab-up"));
            menu.append(Some("Move Down"), Some("win.move-tab-down"));
            menu.append(Some("Close"), Some("win.close-tab"));
            let popover = gtk::PopoverMenu::builder().menu_model(&menu).has_arrow(false).halign(gtk::Align::Start).build();
            popover.set_parent(&list_row);
            let secondary = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
            let (list, target, menu_popover) = (self.tab_list.clone(), list_row.clone(), popover.clone());
            secondary.connect_pressed(move |_, _, x, y| {
                list.emit_by_name::<()>("row-activated", &[&target]);
                menu_popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                menu_popover.popup();
            });
            list_row.add_controller(secondary);
            list_row.connect_destroy(move |_| popover.unparent());
            self.tab_list.append(&list_row);
            if row.current {
                self.tab_list.select_row(Some(&list_row));
            }
        }
        self.shown_tabs.replace(next);
    }

    /// Tells the desktop when a program starts waiting on the user while they are looking elsewhere.
    fn notify(&self) {
        let activity = self.activity.borrow().clone();
        let keys: HashSet<String> = activity.iter().filter(|a| a.needs_attention()).map(|a| format!("{}:{}:{}", a.block, a.state, a.attention_version)).collect();
        // The first list is history, not news.
        let Some(seen) = self.notified.replace(Some(keys.clone())) else { return };
        if !self.config.borrow().notifications {
            return;
        }
        let Some(app) = self.window.application() else { return };
        for report in activity.iter().filter(|a| a.needs_attention()) {
            let key = format!("{}:{}:{}", report.block, report.state, report.attention_version);
            let watching = self.window.is_active() && self.visible.borrow().contains(&report.block);
            if seen.contains(&key) || watching {
                continue;
            }
            // "Needs approval in aurora", then what the program said or which pane it is.
            let notification = gio::Notification::new(&format!("{} in {}", report.label(), report.session_label));
            let detail = if report.message.is_empty() { report.title.as_str() } else { report.message.as_str() };
            if !detail.is_empty() {
                notification.set_body(Some(detail));
            }
            app.send_notification(Some(&key), &notification);
        }
    }

    // --- Panes ----------------------------------------------------------------------------------

    fn pane(self: &Rc<Self>, session: &str, block: &str) -> Option<gtk::Widget> {
        if let Some(pane) = self.panes.borrow().get(block) {
            pane.client.show(session, block);
            return Some(pane.terminal.widget().clone().upcast());
        }
        let client = Client::new(&self.control).map_err(|error| eprintln!("rexling: {error}")).ok()?;
        let (font, size) = {
            let config = self.config.borrow();
            (config.font.clone(), config.font_size)
        };
        let server = self.endpoint.borrow().clone();
        let palette = self.theme().palette;
        let terminal = Terminal::new(&font, size, palette);
        let frame: Rc<RefCell<Option<Arc<Frame>>>> = Rc::default();

        let (weak, id) = (Rc::downgrade(self), block.to_owned());
        terminal.connect_event(move |event| {
            if let Some(ui) = weak.upgrade() {
                ui.terminal_event(&id, event);
            }
        });
        let (weak, id, view, latest, themed) = (Rc::downgrade(self), block.to_owned(), terminal.clone(), frame.clone(), Cell::new(false));
        let weak_client = Rc::downgrade(&client);
        client.watch(move |update| {
            let (Some(ui), Some(client)) = (weak.upgrade(), weak_client.upgrade()) else { return };
            let showing = client.showing();
            let frame = if showing { update.frame } else { None };
            view.set_frame(frame.clone());
            view.set_controlled(showing && update.state.controlled);
            latest.replace(frame);
            if update.state.connected() && !themed.replace(true) {
                client.set_theme(ui.theme().palette.theme());
            } else if !update.state.connected() {
                themed.set(false);
            }
            if let Some(text) = client.take_clipboard(&update.state) {
                ui.window.clipboard().set_text(&text);
            }
            if *ui.focused.borrow() == id {
                ui.latest.set_visible(latest.borrow().as_deref().is_some_and(scrolled_back));
            }
        });
        // Dropping files on a terminal sends them to it.
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let (weak, id) = (Rc::downgrade(self), block.to_owned());
        drop.connect_drop(move |_, value, _, _| {
            let (Some(ui), Ok(files)) = (weak.upgrade(), value.get::<gdk::FileList>()) else { return false };
            for path in files.files().iter().filter_map(|file| file.path()) {
                ui.upload(&id, &path);
            }
            true
        });
        terminal.widget().add_controller(drop);
        client.connect(&server);
        client.show(session, block);
        let widget = terminal.widget().clone().upcast();
        self.panes.borrow_mut().insert(block.to_owned(), Pane { client, terminal, frame });
        Some(widget)
    }

    /// Puts the current tab's panes on screen and retires connections that are no longer needed.
    fn arrange(self: &Rc<Self>) {
        let state = self.state.borrow().clone();
        let tab = state.windows.iter().find(|tab| tab.id == *self.tab.borrow()).filter(|_| self.live(&state));
        let mut placements = tab.map(|tab| tab.placements()).unwrap_or_default();
        if !placements.iter().any(|p| p.id == *self.focused.borrow()) {
            let remembered = tab.and_then(|tab| self.config.borrow().panes.get(&tab.id).cloned()).unwrap_or_default();
            let choice = placements.iter().find(|p| p.id == remembered).or(placements.last()).map(|p| p.id.clone());
            self.focused.replace(choice.unwrap_or_default());
        }
        let focused = self.focused.borrow().clone();
        if self.zoomed.get() {
            placements.retain(|p| p.id == focused);
            placements.iter_mut().for_each(|p| p.rect = [0.0, 0.0, 1.0, 1.0]);
        }

        let mut shown = Vec::new();
        let mut widgets = Vec::new();
        for placement in &placements {
            if let Some(widget) = self.pane(&state.selected_session, &placement.id) {
                widgets.push((widget, placement.rect));
                shown.push(placement.id.clone());
            }
        }
        self.grid.set_splits(if self.zoomed.get() { Vec::new() } else { tab.map(|tab| tab.splits.clone()).unwrap_or_default() });
        self.grid.arrange(widgets);

        // Keep a few recent panes connected; drop the rest and any whose terminal is gone.
        {
            let mut recent = self.recent.borrow_mut();
            recent.retain(|id| !shown.contains(id));
            recent.extend(shown.iter().cloned());
            let live: HashSet<&str> = state.windows.iter().flat_map(|tab| &tab.blocks).map(|b| b.id.as_str()).collect();
            let mut panes = self.panes.borrow_mut();
            if state.connected() {
                recent.retain(|id| live.contains(id.as_str()));
            }
            let spare = recent.len().saturating_sub(shown.len());
            if spare > SPARE_PANES {
                recent.drain(..spare - SPARE_PANES);
            }
            panes.retain(|id, _| recent.contains(id));
        }
        let single = shown.len() < 2;
        for (id, pane) in self.panes.borrow().iter() {
            pane.terminal.set_dimmed(!single && shown.contains(id) && *id != focused);
        }
        let changed = self.visible.replace(shown) != *self.visible.borrow();
        if changed {
            self.refresh_banner();
        }

        let panes = self.panes.borrow();
        let pane = panes.get(&focused);
        self.latest.set_visible(pane.is_some_and(|p| p.frame.borrow().as_deref().is_some_and(scrolled_back)));
        if let Some(pane) = pane.filter(|p| !p.terminal.has_focus() && gtk::prelude::GtkWindowExt::focus(&self.window).is_none_or(|w| w.is::<gtk::DrawingArea>() || w.is::<gtk::ListBoxRow>())) {
            pane.terminal.focus();
        }
        drop(panes);

        let block = tab.and_then(|tab| tab.blocks.iter().find(|b| b.id == focused));
        let heading = block.map(|b| state.title(b)).unwrap_or_else(|| "Rexling".into());
        self.title.set_title(&heading);
        let directory = state.directories.get(&focused).map(|d| home_relative(d)).unwrap_or_default();
        self.title.set_subtitle(if directory == heading { "" } else { &directory });
        self.window.set_title(Some(&heading));
        self.remember(&state);
    }

    fn remember(&self, state: &State) {
        let (tab, focused) = (self.tab.borrow().clone(), self.focused.borrow().clone());
        if !state.connected() || state.selected_session.is_empty() || tab.is_empty() || focused.is_empty() {
            return;
        }
        let mut config = self.config.borrow_mut();
        let server = config.server.clone();
        let changed = config.sessions.get(&server) != Some(&state.selected_session) || config.tabs.get(&state.selected_session) != Some(&tab) || config.panes.get(&tab) != Some(&focused);
        if changed {
            config.sessions.insert(server, state.selected_session.clone());
            config.tabs.insert(state.selected_session.clone(), tab.clone());
            config.panes.insert(tab, focused);
            // Forget what no longer exists on this server.
            let sessions: HashSet<&String> = state.sessions.iter().map(|s| &s.session_id).collect();
            config.tabs.retain(|session, _| sessions.contains(session));
            let current = state.selected_session.clone();
            let tabs: HashSet<String> = state.windows.iter().map(|t| t.id.clone()).chain(config.tabs.iter().filter(|(s, _)| **s != current).map(|(_, t)| t.clone())).collect();
            config.panes.retain(|tab, _| tabs.contains(tab));
            config.save();
        }
    }

    fn terminal_event(self: &Rc<Self>, block: &str, event: Event) {
        let Some(client) = self.panes.borrow().get(block).map(|p| p.client.clone()) else { return };
        match event {
            Event::Input(text) => {
                self.interact();
                client.input(&text);
            }
            Event::Paste(text) => {
                self.interact();
                client.paste(&text);
            }
            Event::Resize(columns, rows) => client.set_size(columns, rows),
            Event::Scroll(lines) => client.scroll(lines),
            Event::ScrollToBottom => {
                if self.panes.borrow().get(block).is_some_and(|p| p.frame.borrow().as_deref().is_some_and(scrolled_back)) {
                    client.scroll_to_bottom();
                }
            }
            Event::Wheel { lines, column, row, cell_width, cell_height } => client.wheel(lines, column, row, cell_width, cell_height),
            Event::Click { column, row, cell_width, cell_height } => client.click(column, row, cell_width, cell_height),
            Event::Claim => self.interact(),
            Event::Focused => {
                if *self.focused.borrow() != block && self.visible.borrow().iter().any(|id| id == block) {
                    self.focused.replace(block.to_owned());
                    self.show_tabs();
                    self.arrange();
                }
            }
            Event::Zoom(step) => {
                {
                    let mut config = self.config.borrow_mut();
                    config.font_size = if step == 0 { Config::default().font_size } else { (config.font_size + step as f64).clamp(6.0, 32.0) };
                    config.save();
                }
                self.apply_font();
            }
            Event::Upload(path) => self.upload(block, &path),
            Event::OpenLink(link) => gtk::UriLauncher::new(&link).launch(Some(&self.window), gio::Cancellable::NONE, |_| {}),
        }
    }
}
