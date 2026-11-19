//! One connection to a Rex server: the engine session, its latest state, and the rules for
//! opening its session and pane, taking control, and sizing the remote terminal.
//!
//! A window uses several: one that follows the server's sessions and tabs, and one per visible
//! pane, each streaming and owning its own terminal. That is what makes split panes live.
use rex_core::ghostty::Frame;
use rex_engine::engine::Session;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};

#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
pub struct SessionItem {
    pub session_id: String,
    #[serde(default)]
    pub label: String,
}
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
pub struct Block {
    pub id: String,
    #[serde(default)]
    pub label: String,
}
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Placement {
    pub id: String,
    /// Left, top, width and height as fractions of the tab.
    pub rect: [f64; 4],
    pub floating: bool,
    pub layer: i64,
    /// The floating layer this pane is in, by which it is moved and resized.
    #[serde(rename = "layerId")]
    pub layer_id: String,
}
/// A border between tiled panes that can be dragged.
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Split {
    /// "horizontal" for a border between left and right, "vertical" for top and bottom.
    pub direction: String,
    pub ratio: f64,
    /// The area the two sides share, as fractions of the tab.
    pub rect: [f64; 4],
    pub before: String,
    pub after: String,
}
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub blocks: Vec<Block>,
    pub layout: Vec<Placement>,
    pub splits: Vec<Split>,
}
impl Tab {
    /// The name someone gave this tab, if any. The server's own "Window 1b7118ed" style names do not count.
    pub fn name(&self) -> Option<&str> {
        let generated = |prefix: &str| self.label.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        let label = self.label.trim();
        (!label.is_empty() && !generated("Window ") && !generated("Terminal ") && !matches!(label, "shell" | "terminal" | "window")).then_some(label)
    }
    /// Where each pane sits, back to front. Panes the server gave no geometry for share the tab equally.
    pub fn placements(&self) -> Vec<Placement> {
        let mut placed: Vec<Placement> = self.layout.iter().filter(|p| self.blocks.iter().any(|b| b.id == p.id) && p.rect[2] > 0.0 && p.rect[3] > 0.0).cloned().collect();
        placed.sort_by_key(|p| p.layer);
        let missing: Vec<&Block> = self.blocks.iter().filter(|b| !placed.iter().any(|p| p.id == b.id)).collect();
        if placed.is_empty() {
            let count = missing.len().max(1) as f64;
            return missing.iter().enumerate().map(|(i, b)| Placement { id: b.id.clone(), rect: [i as f64 / count, 0.0, 1.0 / count, 1.0], ..Placement::default() }).collect();
        }
        placed
    }
}
/// What a program reported about itself: working, waiting on the user, done or failed.
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Activity {
    pub session: String,
    pub session_label: String,
    pub block: String,
    pub title: String,
    pub message: String,
    pub state: String,
    pub kind: String,
    pub attention_version: String,
}
impl Activity {
    pub fn needs_attention(&self) -> bool {
        matches!(self.state.as_str(), "blocked" | "failed" | "finished")
    }
    pub fn label(&self) -> &'static str {
        match (self.state.as_str(), self.kind.as_str()) {
            ("blocked", "permission") => "Needs approval",
            ("blocked", "question") => "Has a question",
            ("blocked", "auth") => "Needs sign-in",
            ("blocked", _) => "Needs you",
            ("failed", _) => "Failed",
            ("finished", _) => "Finished",
            ("working", _) => "Working",
            _ => "",
        }
    }
}
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct ClipboardWrite {
    pub serial: u64,
    pub text: String,
}
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct State {
    pub phase: String,
    pub endpoint: String,
    pub error: String,
    pub notice: String,
    pub sessions: Vec<SessionItem>,
    pub windows: Vec<Tab>,
    pub selected_session: String,
    pub selected_block: String,
    pub controlled: bool,
    pub creating: bool,
    pub navigation_id: u64,
    pub connection_epoch: u64,
    pub titles: HashMap<String, String>,
    pub directories: HashMap<String, String>,
    pub programs: HashMap<String, Vec<String>>,
    pub working: Vec<String>,
    pub clipboard: Option<ClipboardWrite>,
}

impl State {
    pub fn connected(&self) -> bool {
        self.phase == "connected"
    }
    pub fn tab_of(&self, block: &str) -> Option<&Tab> {
        self.windows.iter().find(|tab| tab.blocks.iter().any(|b| b.id == block))
    }
    pub fn session_label(&self) -> &str {
        self.sessions.iter().find(|s| s.session_id == self.selected_session).map_or("", |s| s.label.as_str())
    }
    /// The program in a pane, by its short name ("bash", "nvim").
    pub fn program(&self, block: &Block) -> String {
        match self.programs.get(&block.id).and_then(|p| p.last()) {
            Some(name) => name.rsplit('/').next().unwrap_or(name).trim_start_matches('-').to_owned(),
            None => block.label.clone(),
        }
    }
    /// What a pane is running, for people. A shell is known by where it is; anything else by the
    /// title it set, or its name.
    pub fn title(&self, block: &Block) -> String {
        let program = self.program(block);
        let shell = ["bash", "zsh", "fish", "sh", "dash", "ksh", "nu", "tcsh", "csh", "pwsh"].contains(&program.as_str());
        if shell {
            if let Some(directory) = self.directories.get(&block.id).filter(|d| !d.is_empty()) {
                let home = std::env::var("HOME").unwrap_or_default();
                let name = directory.trim_end_matches('/').rsplit('/').next().unwrap_or("");
                return if *directory == home { "~".into() } else if name.is_empty() { "/".into() } else { name.to_owned() };
            }
            return program;
        }
        match self.titles.get(&block.id).map(|t| t.trim()).filter(|t| !t.is_empty()) {
            Some(title) => title.to_owned(),
            None => program,
        }
    }
}

pub struct Update {
    pub state: Rc<State>,
    pub frame: Option<Arc<Frame>>,
    pub state_changed: bool,
    /// Present when the activity list changed.
    pub activity: Option<Rc<Vec<Activity>>>,
}

/// Whether this window holds the keyboard, shared by all of its connections. Launching only
/// watches; the first click, key or navigation takes control, because taking resizes panes for everyone.
pub type Control = Rc<Cell<bool>>;

pub struct Client {
    this: Weak<Client>,
    session: Session,
    state: RefCell<Rc<State>>,
    json: RefCell<String>,
    revision: Cell<u64>,
    navigation: Cell<u64>,
    /// Numbers requests that are not navigations, such as moving a tab.
    request: Cell<u64>,
    /// The navigation the user asked for that the server has not confirmed yet.
    pending: Cell<u64>,
    wants_control: Control,
    /// A connection that only lists a server's sessions and activity, and never opens one.
    passive: Cell<bool>,
    /// The pane this connection shows. Without one it follows the server's sessions and tabs only.
    target: RefCell<Option<(String, String)>>,
    /// Keys typed before control arrived, sent once it does.
    queued: RefCell<Vec<(&'static str, String)>>,
    activity: RefCell<Option<Arc<Value>>>,
    adopt: Cell<bool>,
    last_take: RefCell<(String, u64, u64)>,
    size: Cell<(u16, u16)>,
    sent_size: Cell<(u16, u16)>,
    resize: RefCell<Option<gtk::glib::SourceId>>,
    clipboard_serial: Cell<u64>,
    pub remembered_session: RefCell<String>,
}

impl Client {
    pub fn new(control: &Control) -> anyhow::Result<Rc<Self>> {
        let session = Session::spawn()?;
        Ok(Rc::new_cyclic(|this| Self {
            this: this.clone(),
            session,
            state: RefCell::default(),
            json: RefCell::default(),
            revision: Cell::new(0),
            navigation: Cell::new(0),
            request: Cell::new(0),
            pending: Cell::new(0),
            wants_control: control.clone(),
            passive: Cell::new(false),
            target: RefCell::default(),
            queued: RefCell::default(),
            activity: RefCell::default(),
            adopt: Cell::new(false),
            last_take: RefCell::default(),
            size: Cell::new((0, 0)),
            sent_size: Cell::new((0, 0)),
            resize: RefCell::default(),
            clipboard_serial: Cell::new(0),
            remembered_session: RefCell::default(),
        }))
    }

    /// Calls `on_update` on the main loop whenever the engine publishes a new revision.
    pub fn watch(self: &Rc<Self>, on_update: impl Fn(Update) + 'static) {
        let (tx, rx) = async_channel::bounded::<()>(1);
        let updates = self.session.updates.clone();
        std::thread::Builder::new()
            .name("rex-updates".into())
            .spawn(move || {
                let mut seen = 0u64;
                loop {
                    let next = updates.wait(seen, 1000);
                    if next < 0 {
                        break;
                    }
                    if next as u64 > seen {
                        seen = next as u64;
                        // A full channel already has a wake-up queued; the reader takes the latest state.
                        if tx.try_send(()).is_err() && tx.is_closed() {
                            break;
                        }
                    }
                }
            })
            .expect("spawn update watcher");
        let client = Rc::downgrade(self);
        gtk::glib::spawn_future_local(async move {
            while rx.recv().await.is_ok() {
                let Some(client) = client.upgrade() else { break };
                if let Some(update) = client.read() {
                    client.follow(&update.state);
                    on_update(update);
                }
            }
        });
    }

    fn read(&self) -> Option<Update> {
        let shared = self.session.shared.lock().ok()?;
        if shared.revision == self.revision.get() {
            return None;
        }
        self.revision.set(shared.revision);
        let frame = shared.frame.clone();
        let state_changed = *self.json.borrow() != shared.json;
        if state_changed {
            self.json.replace(shared.json.clone());
            let parsed: State = serde_json::from_str(&shared.json).unwrap_or_default();
            self.state.replace(Rc::new(parsed));
        }
        let activity = match &mut *self.activity.borrow_mut() {
            Some(seen) if Arc::ptr_eq(seen, &shared.activity) => None,
            seen => {
                *seen = Some(shared.activity.clone());
                Some(Rc::new(serde_json::from_value::<Vec<Activity>>((*shared.activity).clone()).unwrap_or_default()))
            }
        };
        Some(Update { state: self.state(), frame, state_changed, activity })
    }

    pub fn state(&self) -> Rc<State> {
        self.state.borrow().clone()
    }

    fn command(&self, value: Value) -> bool {
        match self.session.command(value) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("rexling: {error}");
                false
            }
        }
    }
    fn settled(&self, state: &State) -> bool {
        state.connected() && !state.creating && state.navigation_id >= self.pending.get()
    }
    fn navigate(&self) -> u64 {
        let id = self.navigation.get().max(self.state().navigation_id) + 1;
        self.navigation.set(id);
        self.pending.set(id);
        id
    }

    /// Reacts to new state: open this connection's session and pane, then keep control and size current.
    fn follow(&self, state: &State) {
        if !self.settled(state) {
            return;
        }
        if self.passive.get() {
            return;
        }
        let Some((session, block)) = self.target.borrow().clone() else {
            // No pane: hold a session open so its tabs are known.
            if self.adopt.get() && !state.selected_session.is_empty() && state.selected_session != *self.remembered_session.borrow() {
                // The server moved us to the session we just asked it to create.
                self.adopt.set(false);
                self.remembered_session.replace(state.selected_session.clone());
            }
            let wanted = self.remembered_session.borrow().clone();
            if state.selected_session.is_empty() || (!wanted.is_empty() && wanted != state.selected_session && state.sessions.iter().any(|s| s.session_id == wanted)) {
                let choice = state.sessions.iter().find(|s| s.session_id == wanted).or(state.sessions.first());
                if let Some(choice) = choice.filter(|s| s.session_id != state.selected_session) {
                    let id = choice.session_id.clone();
                    self.command(json!({"type":"session","id":id,"navigationId":self.navigate()}));
                }
            }
            return;
        };
        if state.selected_session != session {
            if state.sessions.iter().any(|s| s.session_id == session) {
                self.command(json!({"type":"session","id":session,"block":block,"navigationId":self.navigate()}));
            }
            return;
        }
        if state.selected_block != block {
            if state.tab_of(&block).is_some() {
                self.command(json!({"type":"block","id":block,"navigationId":self.navigate()}));
            }
            return;
        }
        if !state.controlled {
            self.sent_size.set((0, 0));
            if self.wants_control.get() {
                self.take(state);
            }
        } else {
            if self.size.get() != self.sent_size.get() {
                self.schedule_resize();
            }
            for (kind, text) in self.queued.take() {
                self.send(kind, &text);
            }
        }
    }

    /// Shows `block` of `session` on this connection.
    pub fn show(&self, session: &str, block: &str) {
        let next = Some((session.to_owned(), block.to_owned()));
        if *self.target.borrow() != next {
            self.target.replace(next);
            self.queued.borrow_mut().clear();
            self.follow(&self.state());
        }
    }
    /// Whether this connection currently streams its pane.
    pub fn showing(&self) -> bool {
        let state = self.state();
        self.target.borrow().as_ref().is_some_and(|(session, block)| state.connected() && state.selected_session == *session && state.selected_block == *block)
    }

    fn take(&self, state: &State) {
        let (columns, rows) = self.size.get();
        if columns == 0 || rows == 0 || state.selected_block.is_empty() {
            return;
        }
        let key = (state.selected_block.clone(), state.navigation_id, state.connection_epoch);
        if *self.last_take.borrow() == key {
            return;
        }
        self.last_take.replace(key);
        self.sent_size.set((columns, rows));
        self.command(json!({"type":"take","session":state.selected_session,"block":state.selected_block,
            "navigationId":state.navigation_id,"columns":columns,"rows":rows}));
    }

    /// Takes the pane if the window holds the keyboard. Returns whether input can be sent right now.
    pub fn claim(&self) -> bool {
        let state = self.state();
        if state.controlled && self.showing() {
            return true;
        }
        if self.wants_control.get() && self.showing() && self.settled(&state) {
            self.last_take.replace(Default::default());
            self.take(&state);
        }
        false
    }

    pub fn connect(&self, server: &str) {
        self.pending.set(0);
        self.command(json!({"type":"connect","server":server}));
    }
    pub fn retry(&self) {
        self.command(json!({"type":"retry"}));
    }
    pub fn set_theme(&self, theme: Value) {
        self.command(json!({"type":"theme","args":theme}));
    }
    pub fn select_session(&self, id: &str) {
        self.remembered_session.replace(id.to_owned());
        self.follow(&self.state());
    }
    pub fn watch_activity(&self) {
        self.command(json!({"type":"activity","open":true}));
    }

    pub fn new_session(&self, label: &str) {
        self.adopt.set(true);
        self.command(json!({"type":"new_session","label":label}));
    }
    pub fn set_passive(&self) {
        self.passive.set(true);
    }
    /// Lifts a tiled pane out of the layout so it floats above it.
    pub fn float_pane(&self, block: &str) {
        let state = self.state();
        if let Some(tab) = state.tab_of(block) {
            self.command(json!({"type":"float_pane","session":state.selected_session,"window":tab.id,"id":block}));
        }
    }
    /// Puts a floating pane back into the layout, beside `anchor`.
    pub fn dock_pane(&self, block: &str, anchor: &str) {
        let state = self.state();
        if let Some(tab) = state.tab_of(block) {
            self.command(json!({"type":"dock_pane","session":state.selected_session,"window":tab.id,"id":block,"anchor":anchor,"direction":"right"}));
        }
    }
    pub fn move_layer(&self, layer: &str, bounds: [f64; 4]) {
        let state = self.state();
        if self.settled(&state) {
            self.command(json!({"type":"move_layer","session":state.selected_session,"layer":layer,"bounds":bounds}));
        }
    }
    pub fn resize_split(&self, split: &Split, ratio: f64) {
        let state = self.state();
        if self.settled(&state) {
            self.command(json!({"type":"resize_pane","session":state.selected_session,"direction":split.direction,
                "before":split.before,"after":split.after,"ratio":ratio}));
        }
    }
    pub fn rename_tab(&self, id: &str, label: &str) {
        let state = self.state();
        self.command(json!({"type":"rename_tab","session":state.selected_session,"window":id,"id":id,"label":label}));
    }
    pub fn rename_session(&self, id: &str, label: &str) {
        self.command(json!({"type":"rename_session","id":id,"label":label}));
    }
    /// Moves a tab to sit before `before`, or to the end when there is none.
    pub fn move_tab(&self, id: &str, before: Option<&str>) {
        let state = self.state();
        self.command(json!({"type":"move_tab","session":state.selected_session,"id":id,"before":before.unwrap_or(""),"requestId":self.request.replace(self.request.get() + 1) + 1}));
    }
    pub fn close_session(&self, id: &str) {
        self.command(json!({"type":"close_session","id":id}));
    }
    pub fn close_tab(&self, id: &str) {
        let state = self.state();
        if let Some(tab) = state.windows.iter().find(|tab| tab.id == id).filter(|_| self.settled(&state)) {
            let blocks: Vec<&str> = tab.blocks.iter().map(|b| b.id.as_str()).collect();
            self.command(json!({"type":"close_tab","session":state.selected_session,"id":id,"blocks":blocks}));
        }
    }
    pub fn new_tab(&self) {
        let state = self.state();
        if self.settled(&state) && !state.selected_session.is_empty() {
            self.command(json!({"type":"new_tab","session":state.selected_session}));
        }
    }
    /// Splits `anchor`. `below` stacks the new pane under it; otherwise it goes beside it.
    pub fn split(&self, anchor: &str, below: bool) {
        let state = self.state();
        if self.settled(&state) && state.tab_of(anchor).is_some() {
            self.command(json!({"type":"new_pane","session":state.selected_session,"anchor":anchor,
                "direction":if below {"vertical"} else {"horizontal"}}));
        }
    }
    pub fn close_pane(&self, block: &str) {
        let state = self.state();
        if self.settled(&state) && state.tab_of(block).is_some() {
            self.command(json!({"type":"close_pane","session":state.selected_session,"id":block}));
        }
    }

    pub fn input(&self, text: &str) {
        self.send("input", text);
    }
    pub fn paste(&self, text: &str) {
        self.send("paste", text);
    }
    fn send(&self, kind: &str, text: &str) {
        let state = self.state();
        if text.is_empty() {
            return;
        }
        if !self.claim() || !self.settled(&state) {
            // Nothing typed is lost while control is on its way; a stale backlog is not replayed.
            let kind = if kind == "paste" { "paste" } else { "input" };
            let mut queued = self.queued.borrow_mut();
            if self.wants_control.get() && queued.len() < 256 {
                queued.push((kind, text.to_owned()));
            }
            return;
        }
        self.command(json!({"type":kind,"text":text,"session":state.selected_session,"block":state.selected_block,
            "navigationId":state.navigation_id,"connectionEpoch":state.connection_epoch}));
    }
    pub fn scroll(&self, lines: i32) {
        let state = self.state();
        self.command(json!({"type":"scroll","block":state.selected_block,"lines":lines}));
    }
    pub fn scroll_to_bottom(&self) {
        let state = self.state();
        self.command(json!({"type":"scroll","block":state.selected_block,"bottom":true}));
    }
    pub fn wheel(&self, lines: i32, column: i32, row: i32, cell_width: i32, cell_height: i32) {
        let state = self.state();
        let (columns, rows) = self.size.get();
        if self.settled(&state) {
            self.command(json!({"type":"wheel","session":state.selected_session,"block":state.selected_block,"lines":lines,
                "col":column,"row":row,"cw":cell_width,"ch":cell_height,"columns":columns,"rows":rows}));
        }
    }
    pub fn click(&self, column: i32, row: i32, cell_width: i32, cell_height: i32) {
        let state = self.state();
        if self.settled(&state) && state.controlled {
            self.command(json!({"type":"click","session":state.selected_session,"block":state.selected_block,
                "navigationId":state.navigation_id,"connectionEpoch":state.connection_epoch,
                "col":column,"row":row,"cw":cell_width,"ch":cell_height}));
        }
    }

    /// The terminal widget's grid changed. The remote pane follows after a pause, so dragging a window edge sends one resize.
    pub fn set_size(&self, columns: u16, rows: u16) {
        if self.size.replace((columns, rows)) == (columns, rows) {
            return;
        }
        let state = self.state();
        if state.controlled {
            self.schedule_resize();
        } else if self.wants_control.get() && self.showing() && self.settled(&state) {
            self.take(&state);
        }
    }
    fn schedule_resize(&self) {
        if self.resize.borrow().is_some() {
            return;
        }
        let weak = self.this.clone();
        let id = gtk::glib::timeout_add_local_once(Duration::from_millis(150), move || {
            let Some(client) = weak.upgrade() else { return };
            client.resize.replace(None);
            let state = client.state();
            let size = client.size.get();
            if state.controlled && client.showing() && client.settled(&state) && size != client.sent_size.get() && size.0 > 0 && size.1 > 0 {
                client.sent_size.set(size);
                client.command(json!({"type":"resize","session":state.selected_session,"block":state.selected_block,
                    "navigationId":state.navigation_id,"columns":size.0,"rows":size.1}));
            }
        });
        self.resize.replace(Some(id));
    }

    /// Text a program asked to put on the clipboard (OSC 52), once per write.
    pub fn take_clipboard(&self, state: &State) -> Option<String> {
        let write = state.clipboard.as_ref()?;
        if write.serial == 0 || write.serial == self.clipboard_serial.replace(write.serial) {
            return None;
        }
        Some(write.text.clone())
    }
}
