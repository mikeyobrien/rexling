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
    /// What the grid places: the terminal, under a bar that only a floating pane shows.
    shell: gtk::Box,
    bar: gtk::Box,
    caption: gtk::Label,
}

/// A saved server that is not open: its sessions and what they report, for the switcher.
struct Watcher {
    /// Held so the connection stays open.
    _client: Rc<Client>,
    state: Rc<State>,
    activity: Rc<Vec<Activity>>,
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
    this: std::rc::Weak<Ui>,
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
    watchers: RefCell<HashMap<String, Watcher>>,
    /// Panes by how recently they were shown, oldest first.
    recent: RefCell<Vec<String>>,
    visible: RefCell<Vec<String>>,
    grid: PaneGrid,
    state: RefCell<Rc<State>>,
    activity: RefCell<Rc<Vec<Activity>>>,
    /// Everything the server reports, including what the user has already looked at.
    reported: RefCell<Rc<Vec<Activity>>>,
    /// Reports already announced, by server. A server's first list is history, not news.
    notified: RefCell<HashMap<String, HashSet<String>>>,
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
    refused: RefCell<String>,
    /// The connection's latest state, connected or not. `state` keeps the last connected one, so
    /// what was on screen stays there, dimmed, while the server is away.
    link: RefCell<Rc<State>>,
    stale_lists: Cell<bool>,
    /// The terminal should take the keyboard as soon as there is one to take it.
    focus_pending: Cell<bool>,
    /// Panes are being placed; focus moving between them now is not the user's doing.
    arranging: Cell<bool>,
    /// A dialog is open over the window; window shortcuts wait until it closes.
    pub modal: Cell<bool>,
    offline: Cell<bool>,
    /// Whether this server has been reached since it was chosen.
    reached: Cell<bool>,
    recovering: Cell<bool>,
    connect_page: adw::StatusPage,
    /// The tab's borders as they were when each floating pane was lifted out, to put it back exactly.
    floated_from: RefCell<HashMap<String, Vec<crate::client::Split>>>,
    /// The pane the server was last told has the focus.
    synced_focus: RefCell<String>,
    /// Finished and failed reports the user is looking at now, by pane.
    reading: RefCell<HashMap<String, (String, String)>>,
    /// Where the open tab sits in the list, to land beside it if it closes.
    tab_at: Cell<usize>,
    /// The tab whose right-click menu is open.
    menu_tab: RefCell<String>,
    /// The tab the list last scrolled into view.
    scrolled_to: RefCell<String>,
    sidebar_toggle: gtk::Button,
    told_restart: Cell<bool>,
    restart_toast: RefCell<Option<adw::Toast>>,
    /// The tab a new tab was asked for from, so it can be placed after it.
    tab_after: RefCell<String>,
    /// The window has gone; nothing may reconnect on its behalf.
    closed: Cell<bool>,
    /// A pane to open as soon as its session is on screen.
    jump: RefCell<String>,
    waiting_list: gtk::ListBox,
    waiting_heading: gtk::Label,
    waiting_box: gtk::Box,
    shown_waiting: RefCell<Vec<Waiting>>,
    known_blocks: RefCell<HashSet<String>>,
}

#[derive(PartialEq, Clone)]
struct PaneRow {
    id: String,
    /// The program, by the name it reported or its command.
    title: String,
    /// Its folder, when it is a shell.
    place: String,
    /// "Needs approval" and the like, and what the program said.
    waiting: String,
    message: String,
    /// Its program's icon.
    tile: String,
    rect: [f64; 4],
    /// "attention", "failed", "done", "working" or nothing.
    status: &'static str,
    focused: bool,
}

/// A pane somewhere else that is waiting on the user.
#[derive(PartialEq, Clone)]
struct Waiting {
    server: String,
    session: String,
    block: String,
    /// "Needs approval", then where: "aurora · codex".
    label: String,
    place: String,
    message: String,
    /// "attention", "failed" or "done".
    kind: &'static str,
}

/// The one name a pane's program goes by: what its icon shows, or the command for a plain shell.
fn identity(state: &State, block: &crate::client::Block) -> String {
    let tile = programs::tile(state.programs.get(&block.id).map_or(&[][..], |p| p.as_slice()), tiles::exists);
    if tile == "shell" { state.program(block) } else { programs::name(&tile) }
}

/// The branch checked out in a folder on this computer, if it is in a Git repository.
fn branch(directory: &str) -> Option<String> {
    let mut folder = std::path::Path::new(directory);
    for _ in 0..24 {
        let head = folder.join(".git/HEAD");
        if head.is_file() {
            let head = std::fs::read_to_string(head).ok()?;
            let head = head.trim();
            // A branch by name, or the short form of a detached commit.
            return Some(head.strip_prefix("ref: refs/heads/").map_or_else(|| head.chars().take(7).collect(), str::to_owned));
        }
        folder = folder.parent()?;
    }
    None
}

/// A small branching line, drawn in the text colour, to say "this is a Git branch".
fn branch_mark() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().content_width(10).content_height(12).valign(gtk::Align::Center).build();
    area.set_draw_func(|area, cr, _, _| {
        let ink = area.color();
        cr.set_source_rgba(ink.red() as f64, ink.green() as f64, ink.blue() as f64, ink.alpha() as f64 * 0.7);
        cr.set_line_width(1.2);
        // A trunk, and a limb joining it from the right.
        cr.move_to(2.5, 3.5);
        cr.line_to(2.5, 8.5);
        cr.move_to(7.5, 3.5);
        cr.curve_to(7.5, 6.5, 2.5, 5.5, 2.5, 8.0);
        let _ = cr.stroke();
        for (x, y) in [(2.5, 2.5), (7.5, 2.5), (2.5, 9.5)] {
            cr.arc(x, y, 1.5, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
    });
    area
}

/// A tab's panes in miniature: each program's icon where its pane is, and nothing else.
fn minimap(panes: &[PaneRow], scale: i32) -> gtk::Fixed {
    const WIDTH: f64 = 50.0;
    const HEIGHT: f64 = 40.0;
    let map = gtk::Fixed::builder().width_request(WIDTH as i32).height_request(HEIGHT as i32).valign(gtk::Align::Center).build();
    for pane in panes.iter().filter(|pane| pane.rect[2] > 0.0 && pane.rect[3] > 0.0) {
        let (x, y, w, h) = (pane.rect[0] * WIDTH, pane.rect[1] * HEIGHT, pane.rect[2] * WIDTH, pane.rect[3] * HEIGHT);
        // The icon keeps its shape, as large as its pane's share allows, centred in it.
        let high = (h - 2.0).min((w - 2.0) * 48.0 / 61.0).floor();
        if high < 6.0 {
            continue;
        }
        let wide = (high * 61.0 / 48.0).round();
        if let Some(texture) = tiles::texture(&pane.tile, high as i32 * scale) {
            let icon = gtk::Picture::builder().paintable(&texture).can_shrink(true).width_request(wide as i32).height_request(high as i32).build();
            map.put(&icon, (x + (w - wide) / 2.0).round(), (y + (h - high) / 2.0).round());
        }
    }
    map
}

#[derive(PartialEq, Clone)]
struct TabRow {
    branch: String,
    /// Its place in the list, when Alt and that digit opens it.
    number: usize,
    panes: Vec<PaneRow>,
    message: String,
    id: String,
    title: String,
    detail: String,
    tile: String,
    status: String,
    status_class: &'static str,
    current: bool,
}

thread_local! {
    /// The session a window being opened should start on.
    static NEXT_WINDOW_SESSION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The forms of address the connect page accepts.
const CONNECT_HINT: &str = "https://host:7297 · ssh://user@host · unix:///path/to/server.sock";

/// Actions that need a connected server, and those that also need a tab on screen.
const SERVER_ACTIONS: &[&str] = &["new-session", "sessions"];
const TAB_ACTIONS: &[&str] = &[
    "forwards",
    "new-tab", "split-right", "split-below", "close-pane", "close-tab", "close-session", "rename-tab", "rename-session",
    "move-tab-up", "move-tab-down", "next-tab", "previous-tab", "next-pane", "previous-pane", "zoom-pane", "float-pane",
    "dock-pane", "attach-image",
];

/// A palette entry: what it is called, a hint beside it, and what it does.
pub type Command = (String, String, Box<dyn Fn(&Rc<Ui>)>);

/// A folder for a narrow column: `~` for home, and only the last two parts of a long path, cut between names.
pub fn short_path(path: &str) -> String {
    let path = home_relative(path);
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if path.chars().count() <= 28 || parts.len() <= 2 {
        return path;
    }
    format!("…/{}", parts[parts.len() - 2..].join("/"))
}

pub fn home_relative(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path == home => "~".into(),
        Ok(home) if !home.is_empty() && path.starts_with(&format!("{home}/")) => format!("~{}", &path[home.len()..]),
        _ => path.to_owned(),
    }
}

/// A pane outline with the half a split would add filled in. No stock icon says "split below".
fn split_icon(below: bool) -> gtk::DrawingArea {
    let icon = gtk::DrawingArea::builder().content_width(16).content_height(16).build();
    icon.set_draw_func(move |area, cr, width, height| {
        let color = area.color();
        let (w, h) = (width as f64, height as f64);
        cr.set_source_rgba(color.red() as f64, color.green() as f64, color.blue() as f64, color.alpha() as f64);
        cr.set_line_width(1.5);
        cr.rectangle(1.5, 2.5, w - 3.0, h - 5.0);
        let _ = cr.stroke();
        if below { cr.rectangle(1.5, h / 2.0, w - 3.0, h / 2.0 - 2.5) } else { cr.rectangle(w / 2.0, 2.5, w / 2.0 - 1.5, h - 5.0) }
        let _ = cr.fill();
    });
    icon
}

fn icon_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
    gtk::Button::builder().icon_name(icon).tooltip_text(tooltip).action_name(action).build()
}

fn scrolled_back(frame: &Frame) -> bool {
    frame.scroll_total.saturating_sub(frame.scroll_visible) > frame.scroll_offset
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
    // One field for the whole menu: typing narrows the sessions, and Enter opens the first match or creates what was typed.
    let new_session = gtk::Entry::builder().placeholder_text("Find or create a session").margin_top(6).margin_start(6).margin_end(6).margin_bottom(6).build();
    let session_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).width_request(260).build();
    session_box.append(&new_session);
    session_box.append(&session_list);
    // Says what Enter will do when nothing matches.
    let create_session = gtk::Button::builder().css_classes(["flat"]).margin_start(6).margin_end(6).margin_bottom(6).visible(false).build();
    session_box.append(&create_session);
    let session_popover = gtk::Popover::builder().child(&session_box).halign(gtk::Align::Start).build();
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
    // Under the tabs: panes elsewhere that are waiting on the user.
    let waiting_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["navigation-sidebar", "rexling-tabs", "rexling-waiting"]).build();
    let waiting_box = gtk::Box::builder().orientation(gtk::Orientation::Vertical).visible(false).build();
    waiting_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let waiting_heading = gtk::Label::builder().xalign(0.0).margin_start(14).margin_top(8).css_classes(["caption-heading", "dim-label"]).build();
    waiting_box.append(&waiting_heading);
    waiting_box.append(&gtk::ScrolledWindow::builder().child(&waiting_list).hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_height(true).max_content_height(190).build());
    sidebar.add_bottom_bar(&waiting_box);

    // Content: the focused pane's title above the tab's panes.
    let title = adw::WindowTitle::new("Rexling", "");
    let header = adw::HeaderBar::builder().title_widget(&title).build();
    let sidebar_toggle = icon_button("sidebar-show-symbolic", "Sidebar", "win.toggle-sidebar");
    header.pack_start(&sidebar_toggle);
    let menu = dialogs::menu(&[&["palette", "next-waiting", "new-window"], &["preferences", "shortcuts", "about"]]);
    header.pack_end(&gtk::MenuButton::builder().icon_name("open-menu-symbolic").tooltip_text("Menu").popover(&menu).primary(true).build());
    for (below, tooltip, action) in [(true, "Split Below", "win.split-below"), (false, "Split Right", "win.split-right")] {
        header.pack_end(&gtk::Button::builder().child(&split_icon(below)).tooltip_text(tooltip).action_name(action).css_classes(["flat", "image-button"]).build());
    }

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

    let server_entry = adw::EntryRow::builder().title("Server address").text(config.server.as_str()).show_apply_button(true).build();
    let server_group = adw::PreferencesGroup::new();
    server_group.add(&server_entry);
    let connect = adw::StatusPage::builder()
        .icon_name("dev.shulhi.Rexling")
        .title("Connect to Rex")
        .description(CONNECT_HINT)
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

    let split = adw::OverlaySplitView::builder().sidebar(&sidebar).content(&content).min_sidebar_width(230.0).max_sidebar_width(320.0).build();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));
    let window = adw::ApplicationWindow::builder().application(app).title("Rexling").default_width(1240).default_height(800).content(&toasts).build();
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth, 760.0, adw::LengthUnit::Sp));
    narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(narrow);

    // One stylesheet for every window: a second copy would override the first for good.
    thread_local! {
        static STYLES: gtk::CssProvider = {
            let styles = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(&gdk::Display::default().expect("display"), &styles, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
            styles
        };
    }
    let styles = STYLES.with(Clone::clone);

    let ui = Rc::new_cyclic(|this| Ui {
        this: this.clone(),
        window: window.clone(),
        config: RefCell::new(config),
        control,
        chrome: chrome.clone(),
        endpoint: RefCell::default(),
        tunnel: RefCell::default(),
        problem: RefCell::default(),
        forwards: RefCell::default(),
        synced_focus: RefCell::default(),
        reading: RefCell::default(),
        tab_at: Cell::new(0),
        menu_tab: RefCell::default(),
        scrolled_to: RefCell::default(),
        sidebar_toggle,
        told_restart: Cell::new(false),
        restart_toast: RefCell::default(),
        tab_after: RefCell::default(),
        closed: Cell::new(false),
        jump: RefCell::default(),
        waiting_list,
        waiting_heading,
        waiting_box,
        shown_waiting: RefCell::default(),
        forwards_changed: RefCell::default(),
        panes: RefCell::default(),
        watchers: RefCell::default(),
        recent: RefCell::default(),
        visible: RefCell::default(),
        grid,
        state: RefCell::default(),
        activity: RefCell::default(),
        reported: RefCell::default(),
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
        refused: RefCell::default(),
        link: RefCell::default(),
        stale_lists: Cell::new(false),
        focus_pending: Cell::new(false),
        arranging: Cell::new(false),
        modal: Cell::new(false),
        offline: Cell::new(false),
        reached: Cell::new(false),
        recovering: Cell::new(false),
        connect_page: connect.clone(),
        floated_from: RefCell::default(),
        known_blocks: RefCell::default(),
    });
    ui.apply_appearance();
    ui.actions();

    let weak = Rc::downgrade(&ui);
    ui.banner.connect_button_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            if !ui.problem.borrow().is_empty() {
                let server = ui.config.borrow().server.clone();
                ui.connect(&server);
            } else if ui.link.borrow().connected() {
                ui.interact()
            } else {
                ui.recover()
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
        let Some(ui) = weak.upgrade() else { return };
        match tunnel::address(&entry.text()) {
            Ok(address) => {
                entry.remove_css_class("error");
                ui.connect(&address);
            }
            Err(problem) => {
                entry.add_css_class("error");
                ui.connect_page.set_description(Some(problem));
            }
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.server_entry.connect_changed(move |entry| {
        // Typing again clears the complaint about what was there before.
        if let Some(ui) = weak.upgrade().filter(|_| entry.has_css_class("error")) {
            entry.remove_css_class("error");
            ui.connect_page.set_description(Some(CONNECT_HINT));
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.tab_list.connect_row_activated(move |_, row| {
        if let Some(ui) = weak.upgrade() {
            ui.interact();
            match row.widget_name().strip_prefix("pane:") {
                Some(pane) => {
                    ui.focused.replace(pane.to_owned());
                    ui.zoomed.set(false);
                    ui.stale_lists.set(true);
                    ui.show_tabs();
                    ui.arrange();
                    ui.focus_terminal();
                }
                None => ui.open_tab(&row.widget_name()),
            }
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.waiting_list.connect_row_activated(move |_, row| {
        let Some(ui) = weak.upgrade() else { return };
        let chosen = row.widget_name().parse::<usize>().ok().and_then(|index| ui.shown_waiting.borrow().get(index).cloned());
        if let Some(waiting) = chosen {
            ui.go_to(&waiting);
        }
    });
    // Right-click a tab for its own commands. One menu serves the whole list, so rebuilding rows cannot orphan it.
    let tab_popover = dialogs::menu(&[&["rename-tab", "move-tab-up", "move-tab-down"], &["close-tab"]]);
    tab_popover.set_has_arrow(false);
    tab_popover.set_halign(gtk::Align::Start);
    // Parented outside the list: a list box treats every child as a row.
    tab_popover.set_parent(&ui.toasts);
    let weak = Rc::downgrade(&ui);
    tab_popover.connect_closed(move |_| {
        // After the chosen command has run, which may itself open a dialog.
        let weak = weak.clone();
        gtk::glib::idle_add_local_once(move || {
            if let Some(ui) = weak.upgrade() {
                ui.menu_tab.replace(String::new());
                ui.sync_actions();
                if !ui.modal.get() {
                    ui.focus_terminal();
                }
            }
        });
    });
    let secondary = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
    let weak = Rc::downgrade(&ui);
    secondary.connect_pressed(move |_, _, x, y| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(row) = ui.tab_list.row_at_y(y as i32).filter(|row| !row.widget_name().starts_with("pane:")) else { return };
        // The menu is about the tab under the pointer; the tab on screen stays where it is.
        let id = row.widget_name().to_string();
        let (at, count) = {
            let state = ui.state.borrow();
            (state.windows.iter().position(|t| t.id == id), state.windows.len())
        };
        for (name, enabled) in [("move-tab-up", at.is_some_and(|at| at > 0)), ("move-tab-down", at.is_some_and(|at| at + 1 < count))] {
            if let Some(action) = ui.window.lookup_action(name).and_downcast::<gio::SimpleAction>() {
                action.set_enabled(enabled);
            }
        }
        ui.menu_tab.replace(id);
        let at = ui.tab_list.compute_point(&ui.toasts, &gtk::graphene::Point::new(x as f32, y as f32)).unwrap_or_else(|| gtk::graphene::Point::new(x as f32, y as f32));
        tab_popover.set_pointing_to(Some(&gdk::Rectangle::new(at.x() as i32, at.y() as i32, 1, 1)));
        tab_popover.popup();
    });
    ui.tab_list.add_controller(secondary);

    let weak = Rc::downgrade(&ui);
    ui.session_list.connect_row_activated(move |_, row| {
        if let Some(ui) = weak.upgrade() {
            ui.session_button.popdown();
            let name = row.widget_name();
            if let Some(server) = name.strip_prefix("server:") {
                ui.connect(server);
            } else if let Some((server, session)) = name.strip_prefix("remote:").and_then(|rest| rest.split_once('|')) {
                ui.open_remote_session(server, session);
            } else {
                ui.open_session(&name);
            }
        }
    });
    let weak = Rc::downgrade(&ui);
    new_session.connect_activate(move |entry| {
        let Some(ui) = weak.upgrade() else { return };
        let typed = entry.text().trim().to_lowercase();
        if typed.is_empty() {
            return;
        }
        let found = ui.shown_sessions.borrow().iter().find(|(id, label, ..)| !id.starts_with("server:") && label.to_lowercase().contains(&typed)).map(|(id, ..)| id.clone());
        ui.session_button.popdown();
        match found.as_deref().map(|id| (id, id.strip_prefix("remote:").and_then(|rest| rest.split_once('|')))) {
            Some((_, Some((server, session)))) => ui.open_remote_session(server, session),
            Some((id, None)) => ui.open_session(id),
            None => {
                ui.interact();
                ui.focus_pending.set(true);
                ui.chrome.new_session(entry.text().trim());
            }
        }
        entry.set_text("");
    });
    let weak = Rc::downgrade(&ui);
    let offer = create_session.clone();
    new_session.connect_changed(move |field| {
        if let Some(ui) = weak.upgrade() {
            ui.session_list.invalidate_filter();
            let typed = field.text().trim().to_owned();
            let found = ui.shown_sessions.borrow().iter().any(|(id, label, ..)| !id.starts_with("server:") && label.to_lowercase().contains(&typed.to_lowercase()));
            offer.set_label(&format!("New Session “{typed}”"));
            offer.set_visible(!typed.is_empty() && !found);
        }
    });
    let field = new_session.clone();
    create_session.connect_clicked(move |_| field.emit_activate());
    let (weak, field) = (Rc::downgrade(&ui), new_session.clone());
    ui.session_list.set_filter_func(move |row| {
        let Some(ui) = weak.upgrade() else { return true };
        let typed = field.text().trim().to_lowercase();
        let name = row.widget_name();
        // Server headings stay; sessions show while they match what is typed.
        typed.is_empty() || name.starts_with("server:") || ui.shown_sessions.borrow().iter().any(|(id, label, ..)| *id == name.as_str() && label.to_lowercase().contains(&typed))
    });
    // The menu opens ready to type in.
    let field = new_session.clone();
    session_popover.connect_show(move |_| {
        field.set_text("");
        field.grab_focus();
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

    // Ctrl+Shift+D is also GTK's own debugging key where that is switched on. While Split Right
    // cannot run, the key does nothing here rather than falling through to it.
    let guard = gtk::EventControllerKey::new();
    guard.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&ui);
    guard.connect_key_pressed(move |controller, key, _, state| {
        let chord = state.contains(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK) && matches!(key, gdk::Key::D | gdk::Key::d);
        let available = weak.upgrade().is_some_and(|ui| !ui.modal.get() && ui.window.lookup_action("split-right").is_some_and(|action| action.is_enabled()));
        if chord && !available {
            return gtk::glib::Propagation::Stop;
        }
        // Escape closes a sidebar that has slid over the terminal; it is not for the program underneath.
        if key == gdk::Key::Escape && let Some(ui) = weak.upgrade().filter(|ui| !ui.modal.get() && ui.split.is_collapsed() && ui.split.shows_sidebar() && !ui.session_button.is_active()) {
            ui.split.set_show_sidebar(false);
            return gtk::glib::Propagation::Stop;
        }
        // Alt and a digit opens the tab with that number; with no such tab the key is the program's.
        if let Some(ui) = weak.upgrade().filter(|ui| !ui.modal.get())
            && state.contains(gdk::ModifierType::ALT_MASK) && !state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK)
            && let Some(digit) = key.to_unicode().and_then(|c| c.to_digit(10)).filter(|d| (1..=9).contains(d))
        {
            let id = ui.state.borrow().windows.get(digit as usize - 1).map(|tab| tab.id.clone());
            if let Some(id) = id {
                ui.interact();
                ui.open_tab(&id);
                return gtk::glib::Propagation::Stop;
            }
        }
        // Typing belongs to the terminal or to a text field, never to a button that happens to hold the keyboard.
        let Some(ui) = weak.upgrade().filter(|ui| !ui.modal.get()) else { return gtk::glib::Propagation::Proceed };
        let held = gtk::prelude::GtkWindowExt::focus(&ui.window);
        let fine = held.as_ref().is_some_and(|w| w.is::<gtk::DrawingArea>() || w.is::<gtk::Text>() || w.is::<gtk::Editable>() || w.ancestor(gtk::Popover::static_type()).is_some());
        let area = ui.panes.borrow().get(&*ui.focused.borrow()).filter(|_| ui.visible.borrow().contains(&*ui.focused.borrow())).map(|pane| pane.terminal.widget().clone());
        let (false, Some(area)) = (fine, area) else { return gtk::glib::Propagation::Proceed };
        area.grab_focus();
        // Window shortcuts still run; anything else goes to the terminal.
        if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
            return gtk::glib::Propagation::Proceed;
        }
        controller.forward(&area);
        gtk::glib::Propagation::Stop
    });
    window.add_controller(guard);

    let weak = Rc::downgrade(&ui);
    window.connect_is_active_notify(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.mark_seen();
        }
    });
    // The sidebar appearing or folding away must not carry the keyboard off with it.
    for property in ["collapsed", "show-sidebar"] {
        let weak = Rc::downgrade(&ui);
        ui.split.connect_notify_local(Some(property), move |_, _| {
            let weak = weak.clone();
            gtk::glib::idle_add_local_once(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.show_waiting();
                    if !ui.modal.get() && !ui.session_button.is_active() {
                        ui.focus_terminal();
                    }
                }
            });
        });
    }
    // The server does not announce when whoever held a pane lets go of it, so a watching window asks now and then.
    let weak = Rc::downgrade(&ui);
    gtk::glib::timeout_add_local(std::time::Duration::from_secs(3), move || {
        let Some(ui) = weak.upgrade() else { return gtk::glib::ControlFlow::Break };
        if ui.watching() {
            let panes = ui.panes.borrow();
            for id in ui.visible.borrow().iter() {
                if let Some(pane) = panes.get(id) {
                    pane.client.refresh();
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    // Closing the session menu hands the keyboard back to the terminal.
    let weak = Rc::downgrade(&ui);
    session_popover.connect_closed(move |_| {
        if let Some(ui) = weak.upgrade() {
            // A sidebar that slid over the terminal for this menu slides away with it.
            if ui.split.is_collapsed() {
                ui.split.set_show_sidebar(false);
            }
            ui.focus_terminal();
        }
    });
    let weak = Rc::downgrade(&ui);
    ui.grid.connect_float(move |layer, bounds| {
        if let Some(ui) = weak.upgrade() {
            ui.interact();
            ui.chrome.move_layer(layer, bounds);
        }
    });
    // Dropping a tab on another moves it there.
    let reorder = gtk::DropTarget::new(gtk::glib::Type::STRING, gdk::DragAction::MOVE);
    let weak = Rc::downgrade(&ui);
    reorder.connect_motion(move |_, _, y| {
        if let Some(ui) = weak.upgrade() {
            match ui.tab_list.row_at_y(y as i32) {
                Some(row) => ui.tab_list.drag_highlight_row(&row),
                None => ui.tab_list.drag_unhighlight_row(),
            }
        }
        gdk::DragAction::MOVE
    });
    let weak = Rc::downgrade(&ui);
    reorder.connect_leave(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.tab_list.drag_unhighlight_row();
        }
    });
    let weak = Rc::downgrade(&ui);
    reorder.connect_drop(move |_, value, _, y| {
        let (Some(ui), Ok(dragged)) = (weak.upgrade(), value.get::<String>()) else { return false };
        ui.tab_list.drag_unhighlight_row();
        ui.drop_tab(&dragged, y);
        true
    });
    ui.tab_list.add_controller(reorder);

    // With no server chosen yet, a Rex server on this computer needs no address at all.
    let server = Some(ui.config.borrow().server.clone()).filter(|s| !s.is_empty()).or_else(tunnel::local_server);
    match server {
        Some(server) => ui.connect(&server),
        None => ui.stack.set_visible_child_name("connect"),
    }
    window.present();
    // The window owns the UI state for as long as it is open.
    window.connect_close_request(move |_| {
        // A closed window lets go of everything it held on the server, so other windows and devices can take over.
        ui.closed.set(true);
        for forward in ui.forwards.borrow_mut().drain(..) {
            rex_engine::forwards::destroy(forward.handle);
        }
        for (_, pane) in ui.panes.borrow_mut().drain() {
            pane.client.disconnect();
        }
        for (_, watcher) in ui.watchers.borrow_mut().drain() {
            watcher._client.disconnect();
        }
        ui.chrome.disconnect();
        gtk::glib::Propagation::Proceed
    });
}

impl Ui {
    fn actions(self: &Rc<Self>) {
        let add = |name: &str, run: fn(&Rc<Ui>)| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                // Window shortcuts do nothing while a dialog is open over the window.
                if let Some(ui) = weak.upgrade().filter(|ui| !ui.modal.get()) {
                    run(&ui);
                }
            });
            self.window.add_action(&action);
        };
        add("new-tab", |ui| {
            ui.interact();
            ui.focus_pending.set(true);
            let folder = ui.state.borrow().directories.get(&*ui.focused.borrow()).cloned().unwrap_or_default();
            ui.tab_after.replace(ui.tab.borrow().clone());
            ui.chrome.new_tab(&folder);
        });
        add("split-right", |ui| ui.split_pane(false));
        add("split-below", |ui| ui.split_pane(true));
        add("close-pane", |ui| {
            let focused = ui.focused.borrow().clone();
            // The keyboard goes to the nearest pane, so closing twice closes neighbours, not something across the tab.
            let state = ui.state.borrow().clone();
            let placed = state.tab_of(&focused).map(|tab| tab.placements()).unwrap_or_default();
            let centre = |r: [f64; 4]| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            if let Some(mine) = placed.iter().find(|p| p.id == focused).map(|p| centre(p.rect)) {
                let distance = |p: &crate::client::Placement| {
                    let (x, y) = centre(p.rect);
                    (x - mine.0).powi(2) + (y - mine.1).powi(2)
                };
                if let Some(nearest) = placed.iter().filter(|p| p.id != focused).min_by(|a, b| distance(a).total_cmp(&distance(b))) {
                    ui.focused.replace(nearest.id.clone());
                    ui.focus_pending.set(true);
                }
            }
            ui.chrome.close_pane(&focused);
        });
        add("close-tab", |ui| {
            let tab = ui.target_tab();
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
            dialogs::ask(ui, "New Session", "", "Create", |ui, name| {
                ui.interact();
                ui.dismiss_restart();
                // Two sessions with one name cannot be told apart, so the name opens the one there is.
                let existing = ui.state.borrow().sessions.iter().find(|s| !name.is_empty() && s.label.eq_ignore_ascii_case(name)).map(|s| s.session_id.clone());
                if let Some(id) = existing {
                    ui.open_session(&id);
                    return;
                }
                ui.focus_pending.set(true);
                ui.chrome.new_session(name);
            });
        });
        add("rename-tab", |ui| {
            let (tab, state) = (ui.target_tab(), ui.state.borrow().clone());
            let Some(current) = state.windows.iter().find(|t| t.id == tab) else { return };
            // Start from what the tab is called now, whether that name was given or derived.
            let shown = ui.shown_tabs.borrow().iter().find(|row| row.id == tab).map(|row| row.title.clone());
            let label = current.name().map(str::to_owned).or(shown).unwrap_or_default();
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
        add("float-pane", |ui| {
            let focused = ui.focused.borrow().clone();
            let state = ui.state.borrow().clone();
            // Remember the pane's place: its nearest split, which side it was on, and its share.
            let placed = state.tab_of(&focused).map(|tab| tab.placements()).unwrap_or_default();
            // The same key puts a floating pane back.
            if placed.iter().any(|p| p.id == focused && p.floating) {
                ui.dock_pane(&focused);
                return;
            }
            if let Some(tab) = state.tab_of(&focused) {
                ui.floated_from.borrow_mut().insert(focused.clone(), tab.splits.clone());
            }
            ui.interact();
            ui.zoomed.set(false);
            ui.chrome.float_pane(&focused);
        });
        add("dock-pane", |ui| {
            let focused = ui.focused.borrow().clone();
            ui.dock_pane(&focused);
        });
        add("attach-image", |ui| ui.choose_image());
        add("new-window", |ui| {
            // Another window is for another session: it opens on one this window is not showing, when there is one.
            let state = ui.state.borrow().clone();
            let other = state.sessions.iter().find(|s| s.session_id != state.selected_session).map(|s| s.session_id.clone());
            NEXT_WINDOW_SESSION.with(|next| next.replace(other));
            if let Some(app) = ui.window.application().and_downcast::<adw::Application>() {
                build(&app);
            }
        });
        add("next-tab", |ui| ui.step_tab(1));
        add("previous-tab", |ui| ui.step_tab(-1));
        add("next-pane", |ui| ui.step_pane(1));
        add("previous-pane", |ui| ui.step_pane(-1));
        add("zoom-pane", |ui| {
            ui.zoomed.set(!ui.zoomed.get());
            ui.stale_lists.set(true);
            ui.show_tabs();
            ui.arrange();
        });
        add("sessions", |ui| {
            // The menu hangs off the sidebar, so a folded sidebar opens first.
            if !ui.split.shows_sidebar() {
                ui.split.set_show_sidebar(true);
            }
            let weak = ui.this.clone();
            gtk::glib::idle_add_local_once(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.session_button.popup();
                }
            });
        });
        add("next-waiting", |ui| {
            // This session's tabs first, then anything waiting elsewhere.
            let state = ui.state.borrow().clone();
            let visible = ui.visible.borrow().clone();
            let here = ui.activity.borrow().iter().find(|a| a.needs_attention() && a.session == state.selected_session && !visible.contains(&a.block)).map(|a| a.block.clone());
            if let Some(block) = here {
                ui.interact();
                if ui.split.is_collapsed() {
                    ui.split.set_show_sidebar(false);
                }
                ui.jump.replace(block);
                ui.arrange();
                ui.focus_terminal();
            } else if let Some(waiting) = ui.waiting().first().cloned() {
                ui.go_to(&waiting);
            }
        });
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
        if self.config.borrow().appearance != "system" {
            items.push(("Follow System Style".into(), String::new(), Box::new(|ui| {
                ui.config.borrow_mut().appearance = "system".into();
                ui.config.borrow().save();
                ui.apply_appearance();
            })));
        }
        for theme in theme::all() {
            let id = theme.id.clone();
            let kind = if theme.palette.dark { "Dark theme" } else { "Light theme" };
            items.push((theme.name.clone(), kind.into(), Box::new(move |ui| ui.choose_theme(&id))));
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
            let switched = chosen.palette.dark != adw::StyleManager::default().is_dark();
            let previous = config.appearance.clone();
            if switched {
                config.appearance = if chosen.palette.dark { "dark" } else { "light" }.into();
            }
            config.save();
            if switched {
                // A theme of the other brightness can only show if the style follows it: say that it changed, and offer the way back.
                let toast = adw::Toast::builder().title(if chosen.palette.dark { "Style set to Dark" } else { "Style set to Light" }).button_label("Undo").build();
                let weak = self.this.clone();
                toast.connect_button_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        ui.config.borrow_mut().appearance = previous.clone();
                        ui.config.borrow().save();
                        ui.apply_appearance();
                    }
                });
                self.toasts.add_toast(toast);
            }
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
        // Whatever asked for it, an address that cannot be one never replaces a working connection.
        let server = match tunnel::address(server) {
            Ok(server) => server,
            Err(problem) => {
                self.toasts.add_toast(adw::Toast::new(problem));
                return;
            }
        };
        self.focus_pending.set(true);
        {
            let mut config = self.config.borrow_mut();
            config.server = server.clone();
            config.save();
            let preferred = NEXT_WINDOW_SESSION.with(|next| next.take());
            self.chrome.remembered_session.replace(preferred.or_else(|| config.sessions.get(&server).cloned()).unwrap_or_default());
        }
        // Nothing from the previous server may linger: its panes, its tunnel, or its last state.
        self.tunnel.replace(None);
        self.endpoint.replace(String::new());
        self.problem.replace(String::new());
        self.link.replace(Rc::default());
        self.offline.set(false);
        self.tab_list.set_sensitive(true);
        self.session_button.set_sensitive(true);
        self.reached.set(false);
        self.floated_from.borrow_mut().clear();
        self.known_blocks.borrow_mut().clear();
        // Forwards belong to the connection that made them.
        for forward in self.forwards.borrow_mut().drain(..) {
            rex_engine::forwards::destroy(forward.handle);
        }
        self.forwards_updated();
        self.created.replace(String::new());
        self.state.replace(Rc::default());
        self.tab.replace(String::new());
        self.focused.replace(String::new());
        self.panes.borrow_mut().clear();
        self.recent.borrow_mut().clear();
        self.show_sessions();
        self.show_tabs();
        self.arrange();
        // The address stays on screen, editable, until the server has answered.
        self.server_entry.set_text(&server);
        self.connect_page.set_description(Some("Connecting…"));
        self.stack.set_visible_child_name("connect");
        self.sync_actions();
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
    /// Finds out whether a server answers, without disturbing the connection in use. Calls back with what went wrong, if anything.
    pub fn probe(self: &Rc<Self>, address: &str, done: impl Fn(Option<String>) + 'static) {
        let done = Rc::new(done);
        if address.strip_prefix("unix://").is_some_and(|path| !std::path::Path::new(path).exists()) {
            return done(Some("There is no Rex server socket at that path".into()));
        }
        if address.starts_with("ssh://") {
            let (tx, rx) = async_channel::bounded(1);
            let target = address.to_owned();
            std::thread::spawn(move || {
                let _ = tx.send_blocking(tunnel::open(&target).map(|_| ()).map_err(|error| error.to_string()));
            });
            gtk::glib::spawn_future_local(async move {
                done(rx.recv().await.ok().and_then(Result::err));
            });
            return;
        }
        let Ok(client) = Client::new(&self.control) else { return done(Some("Could not start a connection".into())) };
        client.set_passive();
        // The connection lives in this slot until the answer is in.
        let slot: Rc<RefCell<Option<Rc<Client>>>> = Rc::new(RefCell::new(Some(client.clone())));
        let finish = {
            let (slot, done) = (slot.clone(), done.clone());
            Rc::new(move |problem: Option<String>| {
                if slot.borrow_mut().take().is_some() {
                    done(problem);
                }
            })
        };
        let answered = finish.clone();
        client.watch(move |update| {
            if update.state.connected() {
                answered(None);
            } else if !update.state.error.is_empty() {
                answered(Some(tunnel::explain(&update.state.error)));
            }
        });
        client.connect(address);
        gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(8), move || finish(Some("Can't reach the server".into())));
    }
    fn attach(self: &Rc<Self>, endpoint: &str) {
        self.endpoint.replace(endpoint.to_owned());
        self.chrome.connect(endpoint);
        self.watch_servers();
    }
    /// Keeps a light connection to every other saved server, so the switcher lists all sessions
    /// and their programs can ask for attention. Servers behind SSH are left until they are opened.
    fn watch_servers(self: &Rc<Self>) {
        let (current, saved) = {
            let config = self.config.borrow();
            (config.server.clone(), config.servers.clone())
        };
        let wanted: Vec<String> = saved.into_iter().filter(|s| *s != current && !s.starts_with("ssh://")).collect();
        self.watchers.borrow_mut().retain(|server, _| wanted.contains(server));
        for server in wanted {
            if self.watchers.borrow().contains_key(&server) {
                continue;
            }
            let Ok(client) = Client::new(&self.control) else { continue };
            client.set_passive();
            let (weak, address, asked) = (Rc::downgrade(self), server.clone(), Cell::new(false));
            let weak_client = Rc::downgrade(&client);
            client.watch(move |update| {
                let (Some(ui), Some(client)) = (weak.upgrade(), weak_client.upgrade()) else { return };
                if update.state.connected() && !asked.replace(true) {
                    client.watch_activity();
                } else if !update.state.connected() {
                    asked.set(false);
                }
                if let Some(watcher) = ui.watchers.borrow_mut().get_mut(&address) {
                    watcher.state = update.state.clone();
                    if let Some(activity) = update.activity {
                        watcher.activity = activity;
                    }
                }
                ui.notify();
                ui.show_sessions();
            });
            client.connect(&server);
            self.watchers.borrow_mut().insert(server, Watcher { _client: client, state: Rc::default(), activity: Rc::default() });
        }
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

    /// Puts the jump-to-latest button in the corner of the pane it acts on.
    fn place_latest(&self) {
        let (focused, state) = (self.focused.borrow().clone(), self.state.borrow());
        let rect = if self.zoomed.get() { None } else { state.tab_of(&focused).and_then(|tab| tab.placements().into_iter().find(|p| p.id == focused)).map(|p| p.rect) };
        let [x, y, w, h] = rect.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let (width, height) = (self.grid.width() as f64, self.grid.height() as f64);
        self.latest.set_margin_end(16 + ((1.0 - x - w).max(0.0) * width) as i32);
        self.latest.set_margin_bottom(16 + ((1.0 - y - h).max(0.0) * height) as i32);
    }

    pub fn focus_terminal(&self) {
        if let Some(pane) = self.panes.borrow().get(&*self.focused.borrow()) {
            pane.terminal.focus();
        }
    }

    // --- Navigation ---------------------------------------------------------------------------

    /// The user has moved on from a restart; so does its message.
    fn dismiss_restart(&self) {
        if let Some(toast) = self.restart_toast.take() {
            toast.dismiss();
        }
    }
    pub fn open_session(self: &Rc<Self>, id: &str) {
        self.interact();
        self.dismiss_restart();
        self.focus_pending.set(true);
        self.tab.replace(String::new());
        self.chrome.select_session(id);
    }
    /// Opens a session that lives on another saved server.
    fn open_remote_session(self: &Rc<Self>, server: &str, session: &str) {
        {
            let mut config = self.config.borrow_mut();
            config.sessions.insert(server.to_owned(), session.to_owned());
        }
        self.interact();
        self.connect(server);
    }
    fn drop_tab(self: &Rc<Self>, dragged: &str, y: f64) {
        let state = self.state.borrow().clone();
        let Some(row) = self.tab_list.row_at_y(y as i32).filter(|row| !row.widget_name().starts_with("pane:")) else { return };
        let target = row.widget_name().to_string();
        if target == dragged {
            return;
        }
        // The upper half of a row means "before it"; the lower half, "after it".
        let lower = row.compute_bounds(&self.tab_list).is_some_and(|bounds| y as f32 > bounds.y() + bounds.height() / 2.0);
        let at = state.windows.iter().position(|tab| tab.id == target);
        let before = match (at, lower) {
            (Some(at), true) => state.windows.get(at + 1).map(|tab| tab.id.clone()),
            (Some(_), false) => Some(target),
            _ => None,
        };
        if before.as_deref() != Some(dragged) {
            self.interact();
            self.chrome.move_tab(dragged, before.as_deref());
        }
    }
    fn dock_pane(self: &Rc<Self>, block: &str) {
        let state = self.state.borrow().clone();
        let Some(tab) = state.tab_of(block) else { return };
        let tiled: Vec<_> = tab.placements().into_iter().filter(|p| !p.floating && p.id != block).collect();
        // Exactly where it came from, if the panes it sat among are all still there and no others have joined them.
        let remembered = self.floated_from.borrow_mut().remove(block).unwrap_or_default();
        let before: HashSet<&String> = remembered.iter().flat_map(|s| s.before_all.iter().chain(&s.after_all)).filter(|id| *id != block).collect();
        let unchanged = !before.is_empty() && before.len() == tiled.len() && tiled.iter().all(|p| before.contains(&p.id));
        let mut steps = if unchanged { crate::client::restore(block, &remembered) } else { Vec::new() };
        if steps.is_empty() {
            // Otherwise beside the largest pane.
            let Some(largest) = tiled.iter().max_by(|a, b| (a.rect[2] * a.rect[3]).total_cmp(&(b.rect[2] * b.rect[3]))) else { return };
            steps.push(crate::client::Placing { id: block.to_owned(), anchor: largest.id.clone(), vertical: false, before: false, ratio: 0.5 });
        }
        self.interact();
        self.focused.replace(block.to_owned());
        self.focus_pending.set(true);
        for step in &steps {
            self.chrome.place_pane(step);
        }
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

    /// The tab a command is about: the one right-clicked while its menu is open, otherwise the open one.
    fn target_tab(&self) -> String {
        let menu = self.menu_tab.borrow().clone();
        if menu.is_empty() { self.tab.borrow().clone() } else { menu }
    }
    fn move_tab(self: &Rc<Self>, step: isize) {
        let state = self.state.borrow().clone();
        let tab = self.target_tab();
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
        if self.closed.get() {
            return;
        }
        if let Some(activity) = update.activity {
            self.reported.replace(activity);
            self.sift();
            self.notify();
            // Status lives outside the state these lists are compared by, so rebuild them.
            self.stale_lists.set(true);
        } else if !update.state_changed {
            return;
        }
        let state = update.state;
        // A late update from the server that was just left says nothing about the new one.
        if state.connected() && !self.live(&state) {
            return;
        }
        self.link.replace(state.clone());
        if !state.connected() {
            self.went_offline(&state);
            return;
        }
        self.state.replace(state.clone());
        self.came_online();

        if let Some(text) = self.chrome.take_clipboard(&state) {
            self.window.clipboard().set_text(&text);
        }
        // While connected, an error is a command the server refused: say so once, and carry on.
        let refused = tunnel::explain(&state.error);
        let internal = state.error.contains("selected session changed");
        if self.refused.replace(refused.clone()) != refused && !state.error.is_empty() && !internal {
            self.toasts.add_toast(adw::Toast::new(&refused));
        }
        // Control changes, reconnects and closed or created panes are already visible, so only other notices are toasted.
        let notice = state.notice.clone();
        let visible_already = ["control", "closed", "created", "Reconnect", "renamed"].iter().any(|word| notice.contains(word));
        if self.notice.replace(notice.clone()) != notice && !notice.is_empty() && !visible_already {
            self.toasts.add_toast(adw::Toast::new(&notice));
        }

        // The sessions connection lands on a pane the window has not seen only when the server just created it.
        let blocks: HashSet<String> = state.windows.iter().flat_map(|tab| &tab.blocks).map(|b| b.id.clone()).collect();
        let known = self.known_blocks.replace(blocks);
        if !state.selected_block.is_empty() && self.created.replace(state.selected_block.clone()) != state.selected_block
            && !known.contains(&state.selected_block)
            && let Some(tab) = state.tab_of(&state.selected_block)
        {
            // A new tab opens beside the one it was made from, not at the far end of the list.
            let origin = self.tab_after.replace(String::new());
            let at = state.windows.iter().position(|t| t.id == origin);
            let made = state.windows.iter().position(|t| t.id == tab.id);
            if let (Some(at), Some(made)) = (at, made) && tab.blocks.len() == 1 && made > at + 1 {
                self.chrome.move_tab(&tab.id, Some(&state.windows[at + 1].id));
            }
            self.tab.replace(tab.id.clone());
            self.focused.replace(state.selected_block.clone());
            self.zoomed.set(false);
        }
        if !state.windows.iter().any(|tab| tab.id == *self.tab.borrow()) {
            let remembered = self.config.borrow().tabs.get(&state.selected_session).cloned().unwrap_or_default();
            let beside = state.windows.get(self.tab_at.get().min(state.windows.len().saturating_sub(1)));
            let tab = state.windows.iter().find(|tab| tab.id == remembered).or(beside.filter(|_| !self.tab.borrow().is_empty())).or(state.windows.first());
            self.tab.replace(tab.map(|tab| tab.id.clone()).unwrap_or_default());
        }

        self.stack.set_visible_child_name(if state.sessions.is_empty() { "empty" } else { "terminal" });
        self.show_sessions();
        self.show_tabs();
        self.arrange();
        self.refresh_banner();
        self.sync_actions();
    }

    /// The server answered. The first time, it becomes a saved server.
    fn came_online(self: &Rc<Self>) {
        if !self.reached.replace(true) {
            let mut config = self.config.borrow_mut();
            let server = config.server.clone();
            if config.servers.first() != Some(&server) || config.reached.as_ref() != Some(&server) {
                config.servers.retain(|s| *s != server);
                config.servers.insert(0, server.clone());
                config.reached = Some(server);
                config.save();
            }
            drop(config);
            self.server_entry.remove_css_class("error");
            self.connect_page.set_description(Some(CONNECT_HINT));
        }
        if !self.was_connected.replace(true) {
            self.chrome.watch_activity();
        }
        self.told_restart.set(false);
        if self.offline.replace(false) {
            self.tab_list.set_sensitive(true);
            self.session_button.set_sensitive(true);
        }
    }

    /// The server is not answering. What was on screen stays, dimmed and inert, and the window keeps trying.
    fn went_offline(self: &Rc<Self>, state: &State) {
        self.was_connected.set(false);
        if !self.reached.get() {
            // Never reached: the address may simply be wrong, so it stays on screen to be fixed.
            let server = self.config.borrow().server.clone();
            let missing = server.strip_prefix("unix://").is_some_and(|path| !std::path::Path::new(path).exists());
            self.connect_page.set_description(Some(&if server.is_empty() {
                CONNECT_HINT.to_owned()
            } else if missing {
                "There is no Rex server socket at that path".to_owned()
            } else if state.error.is_empty() {
                "Connecting…".to_owned()
            } else {
                format!("{}. Trying again…", tunnel::explain(&state.error))
            }));
            self.stack.set_visible_child_name("connect");
        } else if !self.offline.replace(true) {
            self.tab_list.set_sensitive(false);
            self.session_button.set_sensitive(false);
        }
        self.refresh_banner();
        self.sync_actions();
        if self.recovering.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        gtk::glib::timeout_add_local(std::time::Duration::from_secs(3), move || {
            let Some(ui) = weak.upgrade() else { return gtk::glib::ControlFlow::Break };
            let link = ui.link.borrow().clone();
            if link.connected() || ui.endpoint.borrow().is_empty() {
                ui.recovering.set(false);
                return gtk::glib::ControlFlow::Break;
            }
            ui.recover();
            gtk::glib::ControlFlow::Continue
        });
    }

    /// One attempt to get back to the server.
    fn recover(self: &Rc<Self>) {
        let link = self.link.borrow().clone();
        let gone = tunnel::explain(&link.error) == "That session no longer exists";
        if gone {
            // The server is back without the session this window had open: start over on what it has now.
            let server = self.config.borrow().server.clone();
            self.config.borrow_mut().sessions.remove(&server);
            self.recovering.set(false);
            self.connect(&server);
            if !self.told_restart.replace(true) {
                let toast = adw::Toast::builder().title("The server restarted and the open session has ended").timeout(5).build();
                self.toasts.add_toast(toast.clone());
                self.restart_toast.replace(Some(toast));
            }
            return;
        }
        // The engine waits longer and longer between its own attempts; asking again keeps recovery prompt.
        let waiting = |phase: &str| phase == "connecting";
        if !waiting(&link.phase) {
            self.chrome.retry();
        }
        for pane in self.panes.borrow().values() {
            let state = pane.client.state();
            if !state.connected() && !waiting(&state.phase) {
                pane.client.retry();
            }
        }
    }

    /// Turns commands on and off with what they need, so nothing on screen is a dead control.
    fn sync_actions(&self) {
        let state = self.state.borrow().clone();
        let online = self.link.borrow().connected() && !self.offline.get();
        let on_tab = online && !self.visible.borrow().is_empty();
        let enable = |name: &str, enabled: bool| {
            if let Some(action) = self.window.lookup_action(name).and_downcast::<gio::SimpleAction>() {
                action.set_enabled(enabled);
            }
        };
        SERVER_ACTIONS.iter().for_each(|name| enable(name, online));
        self.session_button.set_sensitive(online);
        TAB_ACTIONS.iter().for_each(|name| enable(name, on_tab));
        if on_tab {
            let tab = self.tab.borrow().clone();
            let at = state.windows.iter().position(|t| t.id == tab);
            let panes = state.windows.iter().find(|t| t.id == tab).map(|t| t.placements()).unwrap_or_default();
            let focused = self.focused.borrow().clone();
            let floating = panes.iter().any(|p| p.id == focused && p.floating);
            enable("move-tab-up", at.is_some_and(|at| at > 0));
            enable("move-tab-down", at.is_some_and(|at| at + 1 < state.windows.len()));
            enable("next-tab", state.windows.len() > 1);
            enable("previous-tab", state.windows.len() > 1);
            enable("next-pane", panes.len() > 1);
            enable("previous-pane", panes.len() > 1);
            enable("zoom-pane", panes.len() > 1);
            // The same command floats a tiled pane and puts a floating one back.
            enable("float-pane", floating || panes.iter().filter(|p| !p.floating).count() > 1);
            enable("dock-pane", floating);
        }
    }

    /// Whether a pane on screen is held by another client, so this window can only watch it.
    fn watching(&self) -> bool {
        let panes = self.panes.borrow();
        self.visible.borrow().iter().filter_map(|id| panes.get(id)).any(|pane| {
            let state = pane.client.state();
            state.connected() && pane.client.showing() && !state.controlled && state.other_owner
        })
    }

    fn refresh_banner(&self) {
        self.show_waiting();
        let link = self.link.borrow().clone();
        // One status line: a problem, a connection in progress, or that this window is only watching.
        let problem = self.problem.borrow().clone();
        let (text, button) = if !problem.is_empty() {
            (problem, Some("Retry"))
        } else if self.endpoint.borrow().is_empty() && self.config.borrow().server.starts_with("ssh://") {
            ("Opening SSH tunnel…".to_owned(), None)
        } else if !self.reached.get() {
            // The connect page says what is happening.
            (String::new(), None)
        } else if !link.connected() {
            ("Reconnecting. Typing is paused".to_owned(), Some("Retry"))
        } else if self.watching() {
            ("In use elsewhere".to_owned(), Some("Take Control"))
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
        for (state, class) in [("blocked", "attention"), ("error", "failed"), ("done", "done")] {
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
            reports.map(|a| if a.state == "error" { "failed" } else { "attention" }).max().unwrap_or("").to_owned()
        };
        let label = state.session_label();
        self.session_label.set_label(if label.is_empty() { "Sessions" } else { label });
        self.session_button.set_tooltip_text(Some(&format!("Sessions on {}", self.server_label())));
        // The switcher carries a dot when another session wants the user.
        let elsewhere = state.sessions.iter().any(|s| s.session_id != state.selected_session && !attention(&s.session_id).is_empty());
        let _ = elsewhere;

        let mut next: Vec<_> = state.sessions.iter().map(|s| (s.session_id.clone(), s.label.clone(), s.session_id == state.selected_session, attention(&s.session_id))).collect();
        // Every other saved server follows, with its own sessions under it.
        let config = self.config.borrow();
        let watchers = self.watchers.borrow();
        let mut waiting_elsewhere = false;
        for server in config.servers.iter().filter(|server| **server != config.server) {
            next.push((format!("server:{server}"), tunnel::label(server), false, String::new()));
            let Some(watcher) = watchers.get(server).filter(|w| w.state.connected()) else { continue };
            for session in &watcher.state.sessions {
                let reports = watcher.activity.iter().filter(|a| a.session == session.session_id && a.needs_attention());
                let attention = reports.map(|a| if a.state == "error" { "failed" } else { "attention" }).max().unwrap_or("").to_owned();
                waiting_elsewhere |= !attention.is_empty();
                next.push((format!("remote:{server}|{}", session.session_id), session.label.clone(), false, attention));
            }
        }
        drop(watchers);
        drop(config);
        let _ = waiting_elsewhere;
        self.session_dot.set_visible(false);
        if !self.stale_lists.get() && *self.shown_sessions.borrow() == next {
            return;
        }
        self.session_list.remove_all();
        for (id, label, current, attention) in &next {
            let row = gtk::Box::builder().spacing(10).margin_top(4).margin_bottom(4).build();
            if id.starts_with("server:") {
                row.append(&gtk::Image::from_icon_name("network-server-symbolic"));
                row.add_css_class("dim-label");
            } else if id.starts_with("remote:") {
                row.set_margin_start(26);
            }
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
        // Tabs belong to a session; a server with none has no tabs, whatever was open before.
        let open = state.sessions.iter().any(|s| s.session_id == state.selected_session);
        let current_tab = self.tab.borrow().clone();
        let config = self.config.borrow();
        let local = self.endpoint.borrow().starts_with("unix://") && !config.server.starts_with("ssh://");
        let next: Vec<TabRow> = state
            .windows
            .iter()
            .filter(|_| open)
            .map(|tab| {
                let current = tab.id == current_tab;
                let focused = if current { self.focused.borrow().clone() } else { config.panes.get(&tab.id).cloned().unwrap_or_default() };
                let lead = tab.blocks.iter().find(|b| b.id == focused).or(tab.blocks.first());
                let title = tab.name().map(str::to_owned).or_else(|| lead.map(|b| state.title(b))).unwrap_or_else(|| tab.label.clone());
                let directory = lead.and_then(|b| state.directories.get(&b.id)).map(|d| short_path(d)).unwrap_or_default();
                let detail = match tab.blocks.len() {
                    count if count > 1 && current && self.zoomed.get() => format!("Zoomed · 1 of {count} panes"),
                    // A shell is already titled by its folder; when the path adds nothing, say which shell it is.
                    0 | 1 if directory == title || directory.rsplit('/').next() == Some(title.as_str()) => lead.map(|b| identity(&state, b)).unwrap_or_default(),
                    0 | 1 => directory,
                    count if directory.is_empty() => format!("{count} panes"),
                    _ if directory == title => lead.map(|b| identity(&state, b)).unwrap_or_default(),
                    _ => directory,
                };
                let blocks: Vec<&str> = tab.blocks.iter().map(|b| b.id.as_str()).collect();
                let (status, status_class) = self.status(&blocks, &state.working);
                // How many panes want the user, and what the first one said.
                let waiters: Vec<Activity> = self.activity.borrow().iter().filter(|a| a.needs_attention() && blocks.contains(&a.block.as_str())).cloned().collect();
                // In a split tab each waiting pane says so on its own line, so the tab keeps its usual subtitle.
                let (status, status_class) = if tab.blocks.len() > 1 && !waiters.is_empty() { (String::new(), "") } else { (status, status_class) };
                let message = if waiters.len() == 1 && tab.blocks.len() < 2 { waiters[0].message.clone() } else { String::new() };
                // A folder on this computer shows its branch.
                let branch = lead.and_then(|b| state.directories.get(&b.id)).filter(|_| local).and_then(|d| branch(d)).unwrap_or_default();
                let placed = tab.placements();
                let zoomed = current && self.zoomed.get();
                let panes: Vec<PaneRow> = if tab.blocks.len() < 2 { Vec::new() } else {
                    tab.blocks.iter().map(|b| {
                        let report = waiters.iter().find(|a| a.block == b.id);
                        let tile = programs::tile(state.programs.get(&b.id).map_or(&[][..], |p| p.as_slice()), tiles::exists);
                        // The line is named after the same program its icon shows.
                        let program = identity(&state, b);
                        let shell = state.title(b) != state.program(b) && state.directories.contains_key(&b.id);
                        PaneRow {
                            id: b.id.clone(),
                            title: program,
                            place: if shell { state.title(b) } else { String::new() },
                            waiting: report.map(|a| a.label().to_owned()).unwrap_or_default(),
                            message: report.map(|a| a.message.clone()).unwrap_or_default(),
                            tile,
                            // Zoomed, the miniature is the one pane on screen.
                            rect: match (zoomed, b.id == focused) {
                                (true, true) => [0.0, 0.0, 1.0, 1.0],
                                (true, false) => [0.0; 4],
                                _ => placed.iter().find(|p| p.id == b.id).map_or([0.0, 0.0, 1.0, 1.0], |p| p.rect),
                            },
                            status: self.status(&[b.id.as_str()], &state.working).1,
                            focused: b.id == focused,
                        }
                    }).collect()
                };
                let number = state.windows.iter().position(|t| t.id == tab.id).map_or(0, |at| at + 1);
                let tile = lead.map(|b| programs::tile(state.programs.get(&b.id).map_or(&[][..], |p| p.as_slice()), tiles::exists)).unwrap_or_else(|| "shell".into());
                TabRow { branch, number, panes, message, id: tab.id.clone(), title, detail, tile, status, status_class, current }
            })
            .collect();
        drop(config);
        // Tabs are shown after sessions, so this is where a forced rebuild ends.
        if !self.stale_lists.replace(false) && *self.shown_tabs.borrow() == next {
            self.show_waiting();
            return;
        }
        self.tab_list.remove_all();
        let mut selected = None;
        if let Some(at) = next.iter().position(|row| row.current) {
            self.tab_at.set(at);
        }
        let scale = self.window.scale_factor().max(1);
        for row in &next {
            let text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).hexpand(true).valign(gtk::Align::Center).build();
            text.append(&gtk::Label::builder().label(row.title.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["rexling-tab-title"]).build());
            if !row.status.is_empty() {
                text.append(&gtk::Label::builder().label(row.status.as_str()).xalign(0.0).css_classes(["rexling-status", row.status_class]).build());
                if !row.message.is_empty() {
                    text.append(&gtk::Label::builder().label(row.message.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).lines(2).wrap(true).wrap_mode(pango::WrapMode::WordChar).css_classes(["caption", "dim-label"]).build());
                }
            } else if !row.detail.is_empty() || !row.branch.is_empty() {
                // "bash", then the branch its folder has checked out.
                let line = gtk::Box::builder().spacing(4).css_classes(["caption", "dim-label"]).build();
                if !row.detail.is_empty() {
                    line.append(&gtk::Label::builder().label(row.detail.as_str()).xalign(0.0).ellipsize(if row.branch.is_empty() { pango::EllipsizeMode::Start } else { pango::EllipsizeMode::End }).build());
                }
                if !row.branch.is_empty() {
                    let mark = branch_mark();
                    mark.set_margin_start(if row.detail.is_empty() { 0 } else { 4 });
                    line.append(&mark);
                    line.append(&gtk::Label::builder().label(row.branch.as_str()).xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).build());
                }
                text.append(&line);
            }
            let content = gtk::Box::builder().spacing(10).build();
            let picture = gtk::Image::builder().pixel_size(30).valign(gtk::Align::Center).build();
            if let Some(texture) = tiles::texture(&row.tile, 30 * scale) {
                picture.set_paintable(Some(&texture));
            }
            if row.panes.is_empty() {
                content.append(&picture);
            } else {
                content.append(&minimap(&row.panes, scale));
            }
            content.append(&text);
            if row.status_class == "working" {
                content.append(&adw::Spinner::builder().valign(gtk::Align::Center).build());
            }
            // Its place in the list tells apart tabs that are otherwise alike.
            if next.len() > 1 && row.number <= 9 {
                content.append(&gtk::Label::builder().label(row.number.to_string()).css_classes(["dim-label", "caption", "numeric"]).valign(gtk::Align::Center).build());
            }
            // The open tab lists its panes; any other tab lists the ones that want the user.
            let card = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).build();
            card.append(&content);
            for pane in row.panes.iter().filter(|pane| row.current || !pane.waiting.is_empty()) {
                let line = gtk::Box::builder().spacing(8).css_classes(["rexling-pane-line"]).build();
                let dot = gtk::Box::builder().css_classes(["rexling-pane-dot"]).valign(gtk::Align::Center).build();
                if !pane.status.is_empty() {
                    dot.add_css_class(pane.status);
                } else if pane.focused && row.current {
                    dot.add_css_class("focused");
                }
                line.append(&dot);
                line.append(&gtk::Label::builder().label(pane.title.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(if pane.focused && row.current { vec!["caption"] } else { vec!["caption", "dim-label"] }).build());
                if !pane.waiting.is_empty() {
                    line.append(&gtk::Label::builder().label(pane.waiting.as_str()).xalign(1.0).hexpand(true).css_classes(["rexling-status", if pane.status == "failed" { "failed" } else if pane.status == "done" { "done" } else { "attention" }]).build());
                } else if !pane.place.is_empty() {
                    line.append(&gtk::Label::builder().label(pane.place.as_str()).xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).css_classes(["caption", "dim-label"]).build());
                }
                // A click on a pane's line goes to that pane, not just its tab.
                let click = gtk::GestureClick::builder().button(gdk::BUTTON_PRIMARY).build();
                let (weak, tab, id) = (self.this.clone(), row.id.clone(), pane.id.clone());
                click.connect_released(move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    if let Some(ui) = weak.upgrade() {
                        ui.interact();
                        ui.jump.replace(id.clone());
                        ui.open_tab(&tab);
                    }
                });
                line.add_controller(click);
                card.append(&line);
                // With several panes waiting, each one's question sits under its own line.
                if !pane.message.is_empty() {
                    line.set_tooltip_text(Some(&pane.message));
                    card.append(&gtk::Label::builder().label(pane.message.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["caption", "dim-label", "rexling-pane-said"]).build());
                }
            }
            if next.len() > 1 && row.number > 9 {
                content.append(&gtk::Label::builder().label(" ").css_classes(["caption", "numeric"]).build());
            }
            let list_row = gtk::ListBoxRow::builder().child(&card).name(row.id.as_str()).tooltip_text(row.detail.as_str()).build();
            // Tabs can be dragged onto one another to reorder them.
            let source = gtk::DragSource::builder().actions(gdk::DragAction::MOVE).content(&gdk::ContentProvider::for_value(&row.id.to_value())).build();
            let dragged = list_row.clone();
            source.connect_drag_begin(move |source, _| {
                source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&dragged))), 24, 16);
            });
            list_row.add_controller(source);
            self.tab_list.append(&list_row);
            if row.current {
                self.tab_list.select_row(Some(&list_row));
                selected = Some(list_row.clone());
            }
        }
        // The open tab stays in view when the list is longer than the sidebar.
        if let Some(row) = selected.filter(|_| self.scrolled_to.replace(current_tab.clone()) != current_tab) {
            let list = self.tab_list.clone();
            // After the rebuilt rows have been given their places.
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(90), move || {
                let (Some(bounds), Some(view)) = (row.compute_bounds(&list), list.ancestor(gtk::ScrolledWindow::static_type()).and_downcast::<gtk::ScrolledWindow>()) else { return };
                let adjustment = view.vadjustment();
                let (top, bottom) = (bounds.y() as f64, (bounds.y() + bounds.height()) as f64);
                if top < adjustment.value() {
                    adjustment.set_value(top);
                } else if bottom > adjustment.value() + adjustment.page_size() {
                    adjustment.set_value(bottom - adjustment.page_size());
                }
            });
        }
        self.shown_tabs.replace(next);
        self.show_waiting();
    }

    /// Every pane waiting on the user that is not on screen, most pressing first, across all servers.
    fn waiting(&self) -> Vec<Waiting> {
        let current = self.config.borrow().server.clone();
        // What a server said before it stopped answering is not news.
        let mut sources: Vec<(String, Rc<Vec<Activity>>)> = if self.offline.get() || !self.link.borrow().connected() { Vec::new() } else { vec![(current.clone(), self.activity.borrow().clone())] };
        sources.extend(self.watchers.borrow().iter().filter(|(_, watcher)| watcher.state.connected()).map(|(server, watcher)| (server.clone(), watcher.activity.clone())));
        let visible = self.visible.borrow();
        let selected = self.state.borrow().selected_session.clone();
        let mut found: Vec<(u8, Waiting)> = Vec::new();
        for (server, activity) in sources {
            for report in activity.iter().filter(|a| a.needs_attention() && self.unseen(a)) {
                // This session's tabs already say so in their own rows.
                if server == current && (visible.contains(&report.block) || report.session == selected) {
                    continue;
                }
                let rank = match report.state.as_str() { "blocked" => 0, "error" => 1, _ => 2 };
                let place = if report.title.is_empty() { report.session_label.clone() } else { format!("{} · {}", report.session_label, programs::name(&report.title)) };
                found.push((rank, Waiting { server: server.clone(), session: report.session.clone(), block: report.block.clone(), label: report.label().to_owned(), place, message: report.message.clone(), kind: match report.state.as_str() { "error" => "failed", "done" => "done", _ => "attention" } }));
            }
        }
        found.sort_by_key(|(rank, _)| *rank);
        found.into_iter().map(|(_, waiting)| waiting).collect()
    }

    fn show_waiting(&self) {
        let next = self.waiting();
        if let Some(action) = self.window.lookup_action("next-waiting").and_downcast::<gio::SimpleAction>() {
            let visible = self.visible.borrow();
            let any = !next.is_empty() || (!self.offline.get() && self.activity.borrow().iter().any(|a| a.needs_attention() && !visible.contains(&a.block)));
            action.set_enabled(any);
            // With the sidebar folded away, its button says something is waiting.
            if any && !self.split.shows_sidebar() { self.sidebar_toggle.add_css_class("rexling-wants") } else { self.sidebar_toggle.remove_css_class("rexling-wants") }
        }
        if *self.shown_waiting.borrow() == next {
            return;
        }
        self.waiting_list.remove_all();
        for (index, waiting) in next.iter().enumerate() {
            let text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).hexpand(true).build();
            text.append(&gtk::Label::builder().label(waiting.label.as_str()).xalign(0.0).css_classes(["rexling-status", waiting.kind]).build());
            text.append(&gtk::Label::builder().label(waiting.place.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["caption"]).build());
            if !waiting.message.is_empty() {
                text.append(&gtk::Label::builder().label(waiting.message.as_str()).xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["caption", "dim-label"]).build());
            }
            let line = gtk::Box::builder().spacing(10).build();
            let dot = gtk::Box::builder().css_classes(["rexling-dot"]).valign(gtk::Align::Start).margin_top(6).build();
            if waiting.kind != "attention" {
                dot.add_css_class(waiting.kind);
            }
            line.append(&dot);
            line.append(&text);
            self.waiting_list.append(&gtk::ListBoxRow::builder().child(&line).name(index.to_string()).build());
        }
        self.waiting_heading.set_label(&format!("In other sessions · {}", next.len()));
        self.waiting_box.set_visible(!next.is_empty());
        self.shown_waiting.replace(next);
    }

    /// Opens a pane wherever it is: another tab, another session, or another server.
    fn go_to(self: &Rc<Self>, waiting: &Waiting) {
        self.interact();
        if self.split.is_collapsed() {
            self.split.set_show_sidebar(false);
        }
        let state = self.state.borrow().clone();
        self.jump.replace(waiting.block.clone());
        if waiting.server != self.config.borrow().server {
            self.open_remote_session(&waiting.server, &waiting.session);
        } else if waiting.session != state.selected_session {
            self.open_session(&waiting.session);
        } else {
            self.arrange();
            self.focus_terminal();
        }
    }

    /// Tells the desktop when a program starts waiting on the user while they are looking elsewhere.
    /// Whether a report is still news: a finished or failed one stops being news once it has been looked at.
    fn unseen(&self, report: &Activity) -> bool {
        report.state == "blocked" || report.attention_key.is_empty() || self.config.borrow().seen.get(&report.attention_key) != Some(&report.attention_version)
    }
    /// Keeps the reports that are still news.
    fn sift(&self) {
        let kept: Vec<Activity> = self.reported.borrow().iter().filter(|a| !a.needs_attention() || self.unseen(a)).cloned().collect();
        self.activity.replace(Rc::new(kept));
    }
    /// A finished or failed report is cleared once the user has been in its pane and then typed
    /// there or moved on. Being on screen beside the pane in use is not enough.
    fn mark_seen(&self) {
        let focused = self.focused.borrow().clone();
        // Reports the user has now left behind.
        let left: Vec<(String, String)> = {
            let mut reading = self.reading.borrow_mut();
            let gone: Vec<String> = reading.keys().filter(|block| **block != focused).cloned().collect();
            gone.into_iter().filter_map(|block| reading.remove(&block)).collect()
        };
        if self.window.is_active() {
            for report in self.activity.borrow().iter().filter(|a| matches!(a.state.as_str(), "done" | "error") && a.block == focused && !a.attention_key.is_empty()) {
                self.reading.borrow_mut().insert(report.block.clone(), (report.attention_key.clone(), report.attention_version.clone()));
            }
        }
        self.forget(left);
    }
    /// The user answered in this pane: whatever it reported has been dealt with.
    fn answered(&self, block: &str) {
        let read = self.reading.borrow_mut().remove(block);
        self.forget(read.into_iter().collect());
    }
    fn forget(&self, seen: Vec<(String, String)>) {
        if seen.is_empty() {
            return;
        }
        {
            let mut config = self.config.borrow_mut();
            // Old entries are for reports long gone.
            if config.seen.len() > 400 {
                config.seen.clear();
            }
            config.seen.extend(seen);
            config.save();
        }
        self.sift();
        self.stale_lists.set(true);
        self.show_sessions();
        self.show_tabs();
    }

    fn notify(&self) {
        let mut sources: Vec<(String, Rc<Vec<Activity>>)> = vec![(self.config.borrow().server.clone(), self.reported.borrow().clone())];
        sources.extend(self.watchers.borrow().iter().map(|(server, watcher)| (server.clone(), watcher.activity.clone())));
        let enabled = self.config.borrow().notifications;
        let current = self.config.borrow().server.clone();
        for (server, activity) in sources {
            let key = |a: &Activity| format!("{}:{}:{}", a.block, a.state, a.attention_version);
            let keys: HashSet<String> = activity.iter().filter(|a| a.needs_attention()).map(key).collect();
            // A server's first list is history, not news.
            let Some(seen) = self.notified.borrow_mut().insert(server.clone(), keys) else { continue };
            let Some(app) = self.window.application().filter(|_| enabled) else { continue };
            for report in activity.iter().filter(|a| a.needs_attention() && !seen.contains(&key(a))) {
                if server == current && self.window.is_active() && self.visible.borrow().contains(&report.block) {
                    continue; // Already in front of the user.
                }
                // "Needs approval in aurora", then what the program said or which pane it is.
                let notification = gio::Notification::new(&format!("{} in {}", report.label(), report.session_label));
                let detail = if report.message.is_empty() { report.title.as_str() } else { report.message.as_str() };
                if !detail.is_empty() {
                    notification.set_body(Some(detail));
                }
                app.send_notification(Some(&key(report)), &notification);
            }
        }
    }

    // --- Panes ----------------------------------------------------------------------------------

    fn pane(self: &Rc<Self>, session: &str, block: &str) -> Option<gtk::Widget> {
        if let Some(pane) = self.panes.borrow().get(block) {
            pane.client.show(session, block);
            return Some(pane.shell.clone().upcast());
        }
        let client = Client::new(&self.control).map_err(|error| eprintln!("rexling: {error}")).ok()?;
        client.set_lean();
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
        let held = Cell::new(false);
        let weak_client = Rc::downgrade(&client);
        client.watch(move |update| {
            let (Some(ui), Some(client)) = (weak.upgrade(), weak_client.upgrade()) else { return };
            let showing = client.showing();
            if update.state.connected() {
                let frame = if showing { update.frame } else { None };
                view.set_frame(frame.clone());
                latest.replace(frame);
            }
            // Offline, the last screen stays, faded, instead of going blank.
            view.set_offline(!update.state.connected());
            let controlled = showing && update.state.controlled;
            view.set_controlled(controlled);
            // This window held the pane and another client has taken it: stop claiming the keyboard until the user acts again.
            if controlled {
                held.set(true);
            } else if held.get() && showing && update.state.other_owner {
                held.set(false);
                ui.control.set(false);
            }
            if update.state_changed {
                ui.refresh_banner();
            }
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
                ui.place_latest();
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
        let caption = gtk::Label::builder().xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).css_classes(["caption-heading"]).build();
        let dock = gtk::Button::builder().icon_name("view-restore-symbolic").tooltip_text("Dock").css_classes(["flat", "circular"]).valign(gtk::Align::Center).build();
        let (weak, id) = (Rc::downgrade(self), block.to_owned());
        dock.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.dock_pane(&id);
            }
        });
        let bar = gtk::Box::builder().spacing(6).height_request(crate::panegrid::BAR as i32).css_classes(["rexling-float-bar"]).visible(false).build();
        bar.append(&caption);
        bar.append(&dock);
        let shell = gtk::Box::builder().orientation(gtk::Orientation::Vertical).overflow(gtk::Overflow::Hidden).build();
        shell.append(&bar);
        shell.append(terminal.widget());
        let widget = shell.clone().upcast();
        self.panes.borrow_mut().insert(block.to_owned(), Pane { client, terminal, frame, shell, bar, caption });
        Some(widget)
    }

    /// Puts the current tab's panes on screen and retires connections that are no longer needed.
    fn arrange(self: &Rc<Self>) {
        let state = self.state.borrow().clone();
        // A pane the user asked to go to, once its session has loaded.
        let wanted = self.jump.borrow().clone();
        if !wanted.is_empty() && let Some(tab) = state.tab_of(&wanted) {
            self.jump.replace(String::new());
            self.tab.replace(tab.id.clone());
            self.focused.replace(wanted);
            self.zoomed.set(false);
            self.focus_pending.set(true);
            self.stale_lists.set(true);
            self.show_tabs();
        }
        let open = state.sessions.iter().any(|s| s.session_id == state.selected_session);
        let tab = state.windows.iter().find(|tab| tab.id == *self.tab.borrow()).filter(|_| open && self.live(&state));
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
        let mut floats = Vec::new();
        for placement in &placements {
            if let Some(widget) = self.pane(&state.selected_session, &placement.id) {
                widgets.push((widget, placement.rect));
                shown.push(placement.id.clone());
                let floating = placement.floating && !self.zoomed.get();
                if floating && !placement.layer_id.is_empty() {
                    floats.push((placement.layer_id.clone(), placement.rect));
                }
                if let Some(pane) = self.panes.borrow().get(&placement.id) {
                    pane.bar.set_visible(floating);
                    if floating { pane.shell.add_css_class("rexling-float") } else { pane.shell.remove_css_class("rexling-float") }
                    let block = tab.and_then(|tab| tab.blocks.iter().find(|b| b.id == placement.id));
                    pane.caption.set_label(&block.map(|b| state.title(b)).unwrap_or_default());
                }
            }
        }
        self.grid.set_floats(floats);
        self.grid.set_splits(if self.zoomed.get() { Vec::new() } else { tab.map(|tab| tab.splits.clone()).unwrap_or_default() });
        self.arranging.set(true);
        self.grid.arrange(widgets);
        self.arranging.set(false);

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
            let state = pane.client.state();
            pane.terminal.set_marked(!single && *id == focused && (state.controlled || !state.other_owner));
        }
        let first = self.visible.borrow().is_empty() && !shown.is_empty();
        let changed = self.visible.replace(shown) != *self.visible.borrow();
        if changed {
            self.refresh_banner();
        }
        let wanted = self.focus_pending.get() && self.panes.borrow().contains_key(&focused);
        if wanted {
            self.focus_pending.set(false);
        }
        if first || wanted {
            // Terminals have just appeared: the keyboard belongs in one, not on a header button.
            let weak = Rc::downgrade(self);
            gtk::glib::idle_add_local_once(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.focus_terminal();
                }
            });
        }

        let panes = self.panes.borrow();
        let pane = panes.get(&focused);
        self.latest.set_visible(pane.is_some_and(|p| p.frame.borrow().as_deref().is_some_and(scrolled_back)));
        self.place_latest();
        let weak = Rc::downgrade(self);
        gtk::glib::idle_add_local_once(move || {
            if let Some(ui) = weak.upgrade() {
                ui.mark_seen();
            }
        });
        // The server hears which pane the user is in, so other clients and the command line act on the same one.
        if self.control.get() && !focused.is_empty() && *self.synced_focus.borrow() != focused {
            self.synced_focus.replace(focused.clone());
            self.chrome.focus_block(&focused);
        }
        if let Some(pane) = pane.filter(|p| !p.terminal.has_focus() && gtk::prelude::GtkWindowExt::focus(&self.window).is_none_or(|w| w.is::<gtk::DrawingArea>() || w.is::<gtk::ListBoxRow>())) {
            pane.terminal.focus();
        }
        drop(panes);

        let block = tab.and_then(|tab| tab.blocks.iter().find(|b| b.id == focused));
        let pane_title = block.map(|b| state.title(b));
        let heading = tab.and_then(|tab| tab.name()).map(str::to_owned).or(pane_title).unwrap_or_else(|| "Rexling".into());
        self.title.set_title(&heading);
        let directory = state.directories.get(&focused).map(|d| short_path(d)).unwrap_or_default();
        let zoomed = self.zoomed.get() && tab.is_some_and(|tab| tab.blocks.len() > 1);
        self.title.set_subtitle(&match (zoomed, directory == heading) {
            (true, _) => format!("Zoomed · 1 of {} panes", tab.map_or(1, |tab| tab.blocks.len())),
            (false, true) => String::new(),
            (false, false) => directory,
        });
        self.window.set_title(Some(&heading));
        self.remember(&state);
        self.sync_actions();
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
            // Nothing typed while the server is away is sent later by surprise.
            Event::Input(_) | Event::Paste(_) if self.offline.get() => {}
            Event::Input(text) => {
                self.answered(block);
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
            // The pointer chose this pane.
            Event::Pressed => {
                if *self.focused.borrow() != block && self.visible.borrow().iter().any(|id| id == block) {
                    self.focused.replace(block.to_owned());
                    self.show_tabs();
                    self.arrange();
                }
            }
            // Focus that wandered here while panes were being moved about goes back where the user left it.
            Event::Focused => {
                if *self.focused.borrow() != block && !self.arranging.get() {
                    let weak = self.this.clone();
                    gtk::glib::idle_add_local_once(move || {
                        if let Some(ui) = weak.upgrade() {
                            ui.focus_terminal();
                        }
                    });
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
