//! The terminal view: draws the engine's cell grid with Pango and turns keys, text input and
//! pointer events into terminal events. It holds no connection; the window routes its events.
use crate::{input, palette::{self, Palette, Rgb}};
use gtk::{cairo, gdk, glib, pango, prelude::*};
use rex_core::ghostty::{Cell as GridCell, Frame};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};

pub enum Event {
    Input(String),
    Paste(String),
    Resize(u16, u16),
    Scroll(i32),
    ScrollToBottom,
    Wheel { lines: i32, column: i32, row: i32, cell_width: i32, cell_height: i32 },
    Click { column: i32, row: i32, cell_width: i32, cell_height: i32 },
    /// A click or key on a terminal this client does not control yet.
    Claim,
    Zoom(i32),
    OpenLink(String),
    /// Keyboard focus arrived here.
    Focused,
    /// The pointer was pressed here.
    Pressed,
    /// An image was pasted; it is saved here, ready to send.
    Upload(std::path::PathBuf),
}

const PADDING: f64 = 8.0;

#[derive(Clone, Copy, Default)]
struct Metrics {
    width: f64,
    height: f64,
    baseline: f64,
}

struct Selection {
    frame: Arc<Frame>,
    rows: Vec<Range<usize>>,
    anchor: (i32, i32),
    extent: (i32, i32),
}
impl Selection {
    fn ordered(&self) -> ((i32, i32), (i32, i32)) {
        let key = |p: (i32, i32)| (p.1, p.0);
        if key(self.anchor) <= key(self.extent) { (self.anchor, self.extent) } else { (self.extent, self.anchor) }
    }
    fn text(&self) -> String {
        let (first, last) = self.ordered();
        let mut text = String::new();
        let edge = self.frame.columns as i32 - 1;
        for row in first.1..=last.1 {
            let Some(range) = self.rows.get(row as usize) else { continue };
            let start = if row == first.1 { first.0 } else { 0 };
            let end = if row == last.1 { last.0 } else { self.frame.columns as i32 - 1 };
            let mut line = String::new();
            let mut column = start;
            for cell in &self.frame.cells[range.clone()] {
                let x = cell.x as i32;
                if cell.width == 0 || x + cell.width as i32 <= start || x > end {
                    continue;
                }
                for _ in column..x {
                    line.push(' ');
                }
                line.push_str(if cell.text.is_empty() { " " } else { &cell.text });
                column = x + cell.width as i32;
            }
            // The terminal knows which rows ran on into the next: the edge of the window is not a line break,
            // and a line that happens to end at the edge still is one.
            let ran_on = row < last.1 && self.frame.cells[range.clone()].iter().any(|cell| cell.attributes & palette::WRAPPED != 0);
            if ran_on {
                // Every column counts in a row that ran on, trailing spaces included.
                for _ in column..=end {
                    line.push(' ');
                }
                text.push_str(&line);
            } else {
                text.push_str(if end >= edge { line.trim_end() } else { &line });
                if row < last.1 {
                    text.push('\n');
                }
            }
        }
        text
    }
}

struct Inner {
    this: Weak<Inner>,
    area: gtk::DrawingArea,
    im: gtk::IMMulticontext,
    frame: RefCell<Option<Arc<Frame>>>,
    rows: RefCell<Vec<Range<usize>>>,
    palette: Cell<Palette>,
    font: RefCell<String>,
    font_size: Cell<f64>,
    metrics: Cell<Metrics>,
    /// The text resolution the metrics were measured at. The system's text size changes it.
    measured_at: Cell<f64>,
    grid: Cell<(u16, u16)>,
    controlled: Cell<bool>,
    dimmed: Cell<bool>,
    /// Marks this pane as the one with the keyboard, when there are several.
    marked: Cell<bool>,
    offline: Cell<bool>,
    /// How much the grid is shrunk to fit: a pane this client does not size is shown whole, not clipped.
    view_scale: Cell<f64>,
    cursor_lit: Cell<bool>,
    blink: RefCell<Option<glib::SourceId>>,
    selection: RefCell<Option<Selection>>,
    preedit: RefCell<String>,
    scroll: Cell<f64>,
    pointer: Cell<(f64, f64)>,
    images: RefCell<Vec<((u32, u64), cairo::ImageSurface)>>,
    on_event: RefCell<Box<dyn Fn(Event)>>,
}

#[derive(Clone)]
pub struct Terminal(Rc<Inner>);

fn row_ranges(frame: &Frame) -> Vec<Range<usize>> {
    let mut rows = vec![0..0; frame.rows as usize];
    let mut start = 0;
    while start < frame.cells.len() {
        let y = frame.cells[start].y;
        let mut end = start + 1;
        while end < frame.cells.len() && frame.cells[end].y == y {
            end += 1;
        }
        if let Some(slot) = rows.get_mut(y as usize) {
            *slot = start..end;
        }
        start = end;
    }
    rows
}

fn set_color(cr: &cairo::Context, color: Rgb, alpha: f64) {
    cr.set_source_rgba(color[0] as f64 / 255.0, color[1] as f64 / 255.0, color[2] as f64 / 255.0, alpha);
}

impl Terminal {
    pub fn new(font: &str, font_size: f64, palette: Palette) -> Self {
        let area = gtk::DrawingArea::builder().hexpand(true).vexpand(true).focusable(true).can_focus(true).build();
        area.set_cursor_from_name(Some("text"));
        let inner = Rc::new_cyclic(|this| Inner {
            this: this.clone(),
            area: area.clone(),
            im: gtk::IMMulticontext::new(),
            frame: RefCell::default(),
            rows: RefCell::default(),
            palette: Cell::new(palette),
            font: RefCell::new(font.to_owned()),
            font_size: Cell::new(font_size),
            metrics: Cell::default(),
            measured_at: Cell::new(0.0),
            grid: Cell::new((0, 0)),
            controlled: Cell::new(false),
            dimmed: Cell::new(false),
            marked: Cell::new(false),
            offline: Cell::new(false),
            view_scale: Cell::new(1.0),
            cursor_lit: Cell::new(true),
            blink: RefCell::default(),
            selection: RefCell::default(),
            preedit: RefCell::default(),
            scroll: Cell::new(0.0),
            pointer: Cell::new((0.0, 0.0)),
            images: RefCell::default(),
            on_event: RefCell::new(Box::new(|_| {})),
        });
        let terminal = Self(inner);
        terminal.wire();
        terminal
    }
    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.0.area
    }
    pub fn connect_event(&self, handler: impl Fn(Event) + 'static) {
        let _ = self.0.on_event.replace(Box::new(handler));
    }
    pub fn focus(&self) {
        self.0.area.grab_focus();
    }
    pub fn set_frame(&self, frame: Option<Arc<Frame>>) {
        let inner = &self.0;
        let same = match (&*inner.frame.borrow(), &frame) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        inner.rows.replace(frame.as_deref().map(row_ranges).unwrap_or_default());
        inner.frame.replace(frame);
        inner.restart_blink();
        inner.area.queue_draw();
    }
    pub fn set_controlled(&self, controlled: bool) {
        if self.0.controlled.replace(controlled) != controlled {
            self.0.restart_blink();
            self.0.area.queue_draw();
        }
    }
    /// Panes without the keyboard step back a little so the active one stands out.
    pub fn set_dimmed(&self, dimmed: bool) {
        if self.0.dimmed.replace(dimmed) != dimmed {
            self.0.area.queue_draw();
        }
    }
    /// The server is away: what is shown is the last screen, not a live one.
    pub fn set_offline(&self, offline: bool) {
        if self.0.offline.replace(offline) != offline {
            self.0.area.queue_draw();
        }
    }
    pub fn set_marked(&self, marked: bool) {
        if self.0.marked.replace(marked) != marked {
            self.0.area.queue_draw();
        }
    }
    pub fn has_focus(&self) -> bool {
        self.0.area.has_focus()
    }
    pub fn set_font(&self, font: &str, size: f64) {
        self.0.font.replace(font.to_owned());
        self.set_font_size(size);
    }
    pub fn set_palette(&self, palette: Palette) {
        self.0.palette.set(palette);
        self.0.area.queue_draw();
    }
    pub fn set_font_size(&self, size: f64) {
        self.0.font_size.set(size);
        self.0.measure();
        self.0.report_size();
        self.0.area.queue_draw();
    }

    fn wire(&self) {
        let inner = &self.0;
        let area = &inner.area;
        inner.measure();

        let weak = Rc::downgrade(inner);
        area.set_draw_func(move |_, cr, width, height| {
            if let Some(inner) = weak.upgrade() {
                let started = std::time::Instant::now();
                inner.draw(cr, width as f64, height as f64);
                timing::record(started.elapsed());
            }
        });
        let weak = Rc::downgrade(inner);
        area.connect_resize(move |_, _, _| {
            if let Some(inner) = weak.upgrade() {
                inner.clear_selection();
                inner.report_size();
            }
        });
        // The scale factor changes the pixel grid the font is hinted to.
        let weak = Rc::downgrade(inner);
        area.connect_scale_factor_notify(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.measure();
                inner.report_size();
            }
        });

        // Font settings other than size (the UI font's hinting, antialiasing) also move glyphs.
        if let Some(settings) = gtk::Settings::default() {
            for property in ["gtk-xft-dpi", "gtk-xft-hinting", "gtk-xft-hintstyle", "gtk-xft-antialias", "gtk-font-rendering"] {
                let weak = Rc::downgrade(inner);
                settings.connect_notify_local(Some(property), move |_, _| {
                    if let Some(inner) = weak.upgrade() {
                        inner.measure();
                        inner.report_size();
                        inner.area.queue_draw();
                    }
                });
            }
        }

        inner.im.set_client_widget(Some(area));
        let weak = Rc::downgrade(inner);
        inner.im.connect_commit(move |_, text| {
            if let Some(inner) = weak.upgrade() {
                inner.clear_selection();
                inner.emit(Event::Input(text.to_owned()));
            }
        });
        let weak = Rc::downgrade(inner);
        inner.im.connect_preedit_changed(move |im| {
            if let Some(inner) = weak.upgrade() {
                inner.preedit.replace(im.preedit_string().0.to_string());
                inner.area.queue_draw();
            }
        });

        let keys = gtk::EventControllerKey::new();
        keys.set_im_context(Some(&inner.im));
        let weak = Rc::downgrade(inner);
        keys.connect_key_pressed(move |_, key, _, state| match weak.upgrade() {
            Some(inner) if inner.key(key, state) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        });
        area.add_controller(keys);

        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(inner);
        focus.connect_enter(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.im.focus_in();
                inner.restart_blink();
                inner.emit(Event::Focused);
            }
        });
        let weak = Rc::downgrade(inner);
        focus.connect_leave(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.im.focus_out();
                inner.restart_blink();
            }
        });
        area.add_controller(focus);

        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        let weak = Rc::downgrade(inner);
        scroll.connect_scroll(move |controller, _, dy| {
            let Some(inner) = weak.upgrade() else { return glib::Propagation::Proceed };
            let shift = controller.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
            inner.wheel(dy, shift);
            glib::Propagation::Stop
        });
        area.add_controller(scroll);

        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(inner);
        motion.connect_motion(move |_, x, y| {
            if let Some(inner) = weak.upgrade() {
                inner.pointer.set((x, y));
                let link = inner.link_at(x, y).is_some();
                inner.area.set_cursor_from_name(Some(if link { "pointer" } else { "text" }));
            }
        });
        area.add_controller(motion);

        let click = gtk::GestureClick::builder().button(gdk::BUTTON_PRIMARY).build();
        let weak = Rc::downgrade(inner);
        click.connect_pressed(move |_, presses, x, y| {
            if let Some(inner) = weak.upgrade() {
                inner.emit(Event::Pressed);
                inner.area.grab_focus();
                inner.clear_selection();
                if presses == 2 {
                    inner.select_word(x, y);
                }
            }
        });
        let weak = Rc::downgrade(inner);
        click.connect_released(move |gesture, presses, x, y| {
            if let Some(inner) = weak.upgrade() {
                let shift = gesture.current_event_state().contains(gdk::ModifierType::SHIFT_MASK);
                if presses == 1 && inner.selection.borrow().is_none() {
                    inner.clicked(x, y, shift);
                }
            }
        });
        area.add_controller(click);

        let drag = gtk::GestureDrag::builder().button(gdk::BUTTON_PRIMARY).build();
        let weak = Rc::downgrade(inner);
        drag.connect_drag_update(move |gesture, dx, dy| {
            let (Some(inner), Some((x, y))) = (weak.upgrade(), gesture.start_point()) else { return };
            inner.drag(x, y, x + dx, y + dy);
        });
        area.add_controller(drag);

        let middle = gtk::GestureClick::builder().button(gdk::BUTTON_MIDDLE).build();
        let weak = Rc::downgrade(inner);
        middle.connect_pressed(move |_, _, _, _| {
            if let Some(inner) = weak.upgrade() {
                inner.paste_from(&inner.area.primary_clipboard());
            }
        });
        area.add_controller(middle);
    }
}

impl Inner {
    fn emit(&self, event: Event) {
        (self.on_event.borrow())(event);
    }

    fn description(&self, attributes: u32) -> pango::FontDescription {
        let mut description = pango::FontDescription::from_string(&self.font.borrow());
        description.set_size((self.font_size.get() * pango::SCALE as f64) as i32);
        if attributes & palette::BOLD != 0 {
            description.set_weight(pango::Weight::Bold);
        }
        if attributes & palette::ITALIC != 0 {
            description.set_style(pango::Style::Italic);
        }
        description
    }
    /// A Pango context whose glyph advances land on whole pixels, so runs of text stay on the cell grid.
    fn context(&self) -> pango::Context {
        let context = self.area.pango_context();
        if let Ok(mut options) = cairo::FontOptions::new() {
            options.set_hint_metrics(cairo::HintMetrics::On);
            pangocairo::functions::context_set_font_options(&context, Some(&options));
        }
        context.set_round_glyph_positions(true);
        context
    }
    fn measure(&self) {
        let context = self.context();
        let description = self.description(0);
        let layout = pango::Layout::new(&context);
        // A terminal shows the characters a program wrote: "->" stays two characters, not an arrow.
        let plain = pango::AttrList::new();
        plain.insert(pango::AttrFontFeatures::new("liga 0, calt 0, dlig 0"));
        layout.set_attributes(Some(&plain));
        layout.set_font_description(Some(&description));
        layout.set_text("MMMMMMMMMM");
        let width = (layout.pixel_size().0 as f64 / 10.0).max(1.0);
        let metrics = context.metrics(Some(&description), None);
        let ascent = (metrics.ascent() as f64 / pango::SCALE as f64).ceil();
        let descent = (metrics.descent() as f64 / pango::SCALE as f64).ceil();
        self.metrics.set(Metrics { width, height: (ascent + descent).max(1.0), baseline: ascent });
        self.measured_at.set(pangocairo::functions::context_get_resolution(&context));
    }
    fn report_size(&self) {
        let metrics = self.metrics.get();
        let (width, height) = (self.area.width() as f64, self.area.height() as f64);
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let grid = (
            ((width - PADDING * 2.0) / metrics.width).floor().clamp(1.0, 1000.0) as u16,
            ((height - PADDING * 2.0) / metrics.height).floor().clamp(1.0, 1000.0) as u16,
        );
        if self.grid.replace(grid) != grid {
            self.emit(Event::Resize(grid.0, grid.1));
        }
    }

    fn owned(&self) -> bool {
        self.controlled.get() && self.area.has_focus()
    }
    fn restart_blink(&self) {
        if let Some(id) = self.blink.take() {
            id.remove();
        }
        self.cursor_lit.set(true);
        let blinking = self.frame.borrow().as_ref().is_some_and(|f| f.cursor_blinking && f.cursor.is_some());
        if blinking && self.owned() {
            let weak = self.this.clone();
            let id = glib::timeout_add_local(Duration::from_millis(500), move || {
                let Some(inner) = weak.upgrade() else { return glib::ControlFlow::Break };
                inner.cursor_lit.set(!inner.cursor_lit.get());
                inner.area.queue_draw();
                glib::ControlFlow::Continue
            });
            self.blink.replace(Some(id));
        }
        self.area.queue_draw();
    }

    fn draw(&self, cr: &cairo::Context, width: f64, height: f64) {
        let palette = self.palette.get();
        set_color(cr, palette.background, 1.0);
        let _ = cr.paint();
        let selection = self.selection.borrow();
        let live = self.frame.borrow();
        let live_rows = self.rows.borrow();
        let (frame, rows): (&Frame, &[Range<usize>]) = match (selection.as_ref(), live.as_ref()) {
            (Some(selected), _) => (&selected.frame, &selected.rows),
            (None, Some(frame)) => (frame, &live_rows),
            _ => return,
        };
        let context = self.context();
        // The system's text size or font rendering changed since the grid was measured: the letters
        // already follow it, so the grid must too, and the remote terminal is told its new size.
        if (pangocairo::functions::context_get_resolution(&context) - self.measured_at.get()).abs() > 0.01 {
            self.measure();
            let weak = self.this.clone();
            glib::idle_add_local_once(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.report_size();
                }
            });
        }
        let metrics = self.metrics.get();
        let layout = pango::Layout::new(&context);
        // A terminal shows the characters a program wrote: "->" stays two characters, not an arrow.
        let plain = pango::AttrList::new();
        plain.insert(pango::AttrFontFeatures::new("liga 0, calt 0, dlig 0"));
        layout.set_attributes(Some(&plain));
        let needed = (PADDING * 2.0 + frame.columns as f64 * metrics.width, PADDING * 2.0 + frame.rows as f64 * metrics.height);
        // Shrink a pane this client does not size so it shows whole, but never past readable: beyond that it is cut off.
        // While the server is away the last screen stays exactly as it was.
        let fit = if self.controlled.get() || self.offline.get() { 1.0 } else { (width / needed.0).min(height / needed.1).clamp(0.75, 1.0) };
        self.view_scale.set(fit);
        let _ = cr.save();
        cr.scale(fit, fit);
        let (width, height) = (width / fit, height / fit);
        let columns = (frame.columns as f64).min(((width - PADDING) / metrics.width).ceil()) as u16;
        let mut visible = rows.len().min(((height - PADDING) / metrics.height).ceil().max(1.0) as usize);
        // While the server is away a banner takes some of the room. The newest lines are the ones to keep in sight.
        if self.offline.get() && needed.1 > height {
            // Down to the last row with anything on it, not to the bottom of an empty screen.
            let last = rows.iter().rposition(|range| frame.cells[range.clone()].iter().any(|cell| !cell.text.trim().is_empty())).unwrap_or(0);
            let bottom = PADDING * 2.0 + (last + 1) as f64 * metrics.height;
            cr.translate(0.0, -(bottom - height).clamp(0.0, needed.1 - height));
            visible = rows.len();
        }
        let mut style = u32::MAX;
        if !frame.images.is_empty() {
            self.draw_images(cr, frame, |layer| layer < 2);
        }

        for (y, range) in rows.iter().take(visible).enumerate() {
            let cells = &frame.cells[range.clone()];
            let top = PADDING + y as f64 * metrics.height;
            // Backgrounds first, merged into runs.
            let mut run: Option<(u16, u16, Rgb)> = None;
            let flush = |run: &mut Option<(u16, u16, Rgb)>| {
                if let Some((start, end, color)) = run.take()
                    && color != palette.background && start < columns {
                        let left = if start == 0 { 0.0 } else { PADDING + start as f64 * metrics.width };
                        let right = if end >= frame.columns { width } else { PADDING + end as f64 * metrics.width };
                        // Edge cells run into the padding, so a full-screen program fills the pane to its border.
                        let above = if y == 0 { 0.0 } else { top };
                        let below = if y + 1 == frame.rows as usize { height } else { top + metrics.height };
                        set_color(cr, color, 1.0);
                        cr.rectangle(left, above, right - left, below - above);
                        let _ = cr.fill();
                    }
            };
            for cell in cells {
                let color = if cell.attributes & palette::INVERSE != 0 { palette.foreground(cell) } else { palette.background(cell) };
                let end = cell.x + cell.width.max(1) as u16;
                match &mut run {
                    Some((_, run_end, run_color)) if *run_end == cell.x && *run_color == color => *run_end = end,
                    Some((_, run_end, _)) if cell.width == 0 && cell.x < *run_end => {}
                    _ => {
                        flush(&mut run);
                        run = Some((cell.x, end, color));
                    }
                }
            }
            flush(&mut run);

            let mut index = 0;
            while index < cells.len() {
                let cell = &cells[index];
                index += 1;
                if cell.x >= columns || cell.width == 0 || cell.text.is_empty() || cell.attributes & palette::INVISIBLE != 0 {
                    continue;
                }
                // Kitty image placeholders are drawn by the image layer, not as text.
                if cell.text.starts_with('\u{10eeee}') {
                    continue;
                }
                let ascii = |c: &GridCell| c.width == 1 && c.text.len() == 1 && (b' '..=b'~').contains(&c.text.as_bytes()[0]);
                let mut span = cell.width as u16;
                let mut text = std::borrow::Cow::Borrowed(cell.text.as_str());
                if ascii(cell) {
                    let mut joined = String::new();
                    while index < cells.len() {
                        let next = &cells[index];
                        if next.x != cell.x + span || next.x >= columns || !ascii(next) || next.attributes != cell.attributes
                            || next.foreground != cell.foreground || next.background != cell.background {
                            break;
                        }
                        if joined.is_empty() {
                            joined.push_str(&cell.text);
                        }
                        joined.push_str(&next.text);
                        span += 1;
                        index += 1;
                    }
                    if !joined.is_empty() {
                        text = std::borrow::Cow::Owned(joined);
                    }
                }
                let x = PADDING + cell.x as f64 * metrics.width;
                let allocated = span as f64 * metrics.width;
                let (color, behind) = if cell.attributes & palette::INVERSE != 0 { (palette.background(cell), palette.foreground(cell)) } else { (palette.foreground(cell), palette.background(cell)) };
                let color = crate::theme::legible(color, behind);
                let alpha = if cell.attributes & palette::FAINT != 0 { 0.6 } else { 1.0 };
                set_color(cr, color, alpha);
                // Block characters are drawn as the shapes they are, edge to edge, so bars and meters have no seams.
                let blocks = if cell.width == 1 { cell.text.chars().next().filter(|_| cell.text.chars().count() == 1).and_then(block) } else { None };
                if let Some((parts, shade)) = blocks {
                    set_color(cr, color, alpha * shade);
                    let _ = cr.save();
                    cr.set_antialias(gtk::cairo::Antialias::None);
                    for [left, up, wide, high] in parts {
                        // Edges land on whole pixels, the same ones the neighbouring cell uses.
                        let (x0, x1) = ((x + left * metrics.width).round(), (x + (left + wide) * metrics.width).round());
                        let (y0, y1) = ((top + up * metrics.height).round(), (top + (up + high) * metrics.height).round());
                        cr.rectangle(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0));
                    }
                    let _ = cr.fill();
                    let _ = cr.restore();
                } else if text.bytes().any(|b| b != b' ') {
                    let cell_style = cell.attributes & (palette::BOLD | palette::ITALIC);
                    if cell_style != style {
                        layout.set_font_description(Some(&self.description(cell_style)));
                        style = cell_style;
                    }
                    layout.set_text(&text);
                    let baseline = layout.baseline() as f64 / pango::SCALE as f64;
                    let drawn = layout.pixel_size().0 as f64;
                    let _ = cr.save();
                    cr.rectangle(x, top, allocated, metrics.height);
                    cr.clip();
                    if drawn > allocated + 0.5 {
                        // A fallback glyph wider than its cells is squeezed, not allowed to overlap its neighbour.
                        cr.translate(x, 0.0);
                        cr.scale(allocated / drawn, 1.0);
                        cr.move_to(0.0, top + metrics.baseline - baseline);
                    } else {
                        let centre = if ascii(cell) { 0.0 } else { ((allocated - drawn) / 2.0).max(0.0).floor() };
                        cr.move_to(x + centre, top + metrics.baseline - baseline);
                    }
                    pangocairo::functions::show_layout(cr, &layout);
                    let _ = cr.restore();
                }
                if cell.attributes & (palette::UNDERLINE | palette::STRIKETHROUGH) != 0 || cell.hyperlink.is_some() {
                    cr.set_line_width(1.0);
                    if cell.attributes & palette::CURLY != 0 {
                        // A wave of two crests to a cell, starting on the cell's edge so runs join up.
                        let (line, half) = ((top + metrics.baseline + 2.0).floor() + 0.5, metrics.width / 4.0);
                        cr.move_to(x, line);
                        let mut at = x;
                        let mut up = true;
                        while at < x + allocated - 0.01 {
                            let rise = if up { -1.6 } else { 1.6 };
                            cr.curve_to(at + half * 0.4, line + rise, at + half * 0.6, line + rise, at + half, line);
                            at += half;
                            up = !up;
                        }
                    } else if cell.attributes & palette::UNDERLINE != 0 || cell.hyperlink.is_some() {
                        let line = (top + metrics.baseline + 1.0).floor() + 0.5;
                        cr.move_to(x, line);
                        cr.line_to(x + allocated, line);
                    }
                    if cell.attributes & palette::STRIKETHROUGH != 0 {
                        let line = (top + metrics.height / 2.0).floor() + 0.5;
                        cr.move_to(x, line);
                        cr.line_to(x + allocated, line);
                    }
                    let _ = cr.stroke();
                }
            }
        }

        if !frame.images.is_empty() {
            self.draw_images(cr, frame, |layer| layer >= 2);
        }
        if let Some(selected) = selection.as_ref() {
            let (first, last) = selected.ordered();
            set_color(cr, palette.accent, 0.35);
            for row in first.1..=last.1 {
                let start = if row == first.1 { first.0 } else { 0 };
                let end = if row == last.1 { last.0 + 1 } else { frame.columns as i32 };
                cr.rectangle(PADDING + start as f64 * metrics.width, PADDING + row as f64 * metrics.height,
                    (end - start) as f64 * metrics.width, metrics.height);
            }
            let _ = cr.fill();
        } else if let Some((cursor_x, cursor_y)) = frame.cursor {
            self.draw_cursor(cr, frame, rows, &layout, cursor_x, cursor_y);
        }
        // How far back the view is scrolled, as a thin thumb on the right edge.
        let hidden = frame.scroll_total.saturating_sub(frame.scroll_visible);
        if hidden > 0 && frame.scroll_offset < hidden {
            let thumb = (height * frame.scroll_visible as f64 / frame.scroll_total.max(1) as f64).max(24.0);
            let top = (height - thumb) * frame.scroll_offset as f64 / hidden as f64;
            set_color(cr, palette.foreground, 0.35);
            cr.rectangle(width - 4.0, top, 3.0, thumb);
            let _ = cr.fill();
        }
        let _ = cr.restore();
        let width = width * fit;
        let height = height * fit;
        if self.offline.get() {
            set_color(cr, palette.background, 0.6);
            let _ = cr.paint();
        } else if self.dimmed.get() {
            set_color(cr, palette.background, 0.38);
            let _ = cr.paint();
        }
        if self.marked.get() {
            // An outline all the way round: a line on one edge reads as a divider.
            set_color(cr, crate::theme::accent(&palette), 0.9);
            cr.set_line_width(2.0);
            cr.rectangle(1.0, 1.0, width - 2.0, height - 2.0);
            let _ = cr.stroke();
        }
    }

    /// Images placed by the kitty graphics protocol. Layers below 2 sit under the text.
    fn draw_images(&self, cr: &cairo::Context, frame: &Frame, wanted: impl Fn(u32) -> bool) {
        let metrics = self.metrics.get();
        let scale_x = metrics.width / frame.image_cell_width.max(1) as f64;
        let scale_y = metrics.height / frame.image_cell_height.max(1) as f64;
        for placement in frame.images.iter().filter(|p| wanted(p.layer) && p.sw > 0 && p.sh > 0) {
            let Some(surface) = self.image(&placement.image) else { continue };
            let x = PADDING + placement.col as f64 * metrics.width + placement.ox as f64 * scale_x;
            let y = PADDING + placement.row as f64 * metrics.height + placement.oy as f64 * scale_y;
            let (width, height) = (placement.width as f64 * scale_x, placement.height as f64 * scale_y);
            let _ = cr.save();
            cr.rectangle(x, y, width, height);
            cr.clip();
            cr.translate(x, y);
            cr.scale(width / placement.sw as f64, height / placement.sh as f64);
            let _ = cr.set_source_surface(&surface, -(placement.sx as f64), -(placement.sy as f64));
            let _ = cr.paint();
            let _ = cr.restore();
        }
    }
    /// The image as a Cairo surface, converted once and kept while it stays in use.
    fn image(&self, image: &rex_core::graphics::Image) -> Option<cairo::ImageSurface> {
        let key = (image.id, image.generation);
        let mut cache = self.images.borrow_mut();
        if let Some(at) = cache.iter().position(|(k, _)| *k == key) {
            let entry = cache.remove(at);
            cache.push(entry);
            return cache.last().map(|(_, surface)| surface.clone());
        }
        let (width, height) = (image.width as usize, image.height as usize);
        if width == 0 || height == 0 || image.rgba.len() < width * height * 4 {
            return None;
        }
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32).ok()?;
        let stride = surface.stride() as usize;
        {
            // Cairo wants premultiplied BGRA; the engine hands over straight RGBA.
            let mut data = surface.data().ok()?;
            for row in 0..height {
                for column in 0..width {
                    let source = &image.rgba[(row * width + column) * 4..][..4];
                    let alpha = source[3] as u32;
                    let scaled = |channel: u8| ((channel as u32 * alpha + 127) / 255) as u8;
                    data[row * stride + column * 4..][..4].copy_from_slice(&[scaled(source[2]), scaled(source[1]), scaled(source[0]), source[3]]);
                }
            }
        }
        if cache.len() >= 24 {
            cache.remove(0);
        }
        cache.push((key, surface.clone()));
        Some(surface)
    }

    fn draw_cursor(&self, cr: &cairo::Context, frame: &Frame, rows: &[Range<usize>], layout: &pango::Layout, cursor_x: u16, cursor_y: u16) {
        let palette = self.palette.get();
        let metrics = self.metrics.get();
        let cell = rows.get(cursor_y as usize).and_then(|range| frame.cells[range.clone()].iter().find(|c| c.x == cursor_x && c.width > 0));
        let x = PADDING + cursor_x as f64 * metrics.width;
        let y = PADDING + cursor_y as f64 * metrics.height;
        let width = cell.map_or(1, |c| c.width.max(1)) as f64 * metrics.width;
        set_color(cr, palette.accent, 1.0);
        // A terminal that cannot take keys shows a hollow cursor.
        if !self.owned() {
            cr.set_line_width(1.0);
            cr.rectangle(x + 0.5, y + 0.5, width - 1.0, metrics.height - 1.0);
            let _ = cr.stroke();
            return;
        }
        let preedit = self.preedit.borrow();
        if !preedit.is_empty() {
            layout.set_font_description(Some(&self.description(0)));
            layout.set_text(&preedit);
            let drawn = layout.pixel_size().0 as f64;
            cr.rectangle(x, y, drawn.max(metrics.width), metrics.height);
            let _ = cr.fill();
            set_color(cr, [255, 255, 255], 1.0);
            cr.move_to(x, y + metrics.baseline - layout.baseline() as f64 / pango::SCALE as f64);
            pangocairo::functions::show_layout(cr, layout);
            return;
        }
        if !self.cursor_lit.get() {
            return;
        }
        match frame.cursor_style {
            0 => cr.rectangle(x, y, 2.0, metrics.height),
            2 => cr.rectangle(x, y + metrics.height - 2.0, width, 2.0),
            3 => {
                cr.set_line_width(1.0);
                cr.rectangle(x + 0.5, y + 0.5, width - 1.0, metrics.height - 1.0);
                let _ = cr.stroke();
                return;
            }
            _ => {
                cr.rectangle(x, y, width, metrics.height);
                let _ = cr.fill();
                if let Some(cell) = cell.filter(|c| !c.text.is_empty() && c.attributes & palette::INVISIBLE == 0) {
                    layout.set_font_description(Some(&self.description(cell.attributes & (palette::BOLD | palette::ITALIC))));
                    layout.set_text(&cell.text);
                    set_color(cr, [255, 255, 255], 1.0);
                    cr.move_to(x, y + metrics.baseline - layout.baseline() as f64 / pango::SCALE as f64);
                    pangocairo::functions::show_layout(cr, layout);
                }
                return;
            }
        }
        let _ = cr.fill();
    }

    fn cell_at(&self, x: f64, y: f64) -> (i32, i32) {
        let metrics = self.metrics.get();
        let scale = self.view_scale.get();
        (((x / scale - PADDING) / metrics.width).floor() as i32, ((y / scale - PADDING) / metrics.height).floor() as i32)
    }
    fn in_grid(frame: &Frame, cell: (i32, i32)) -> bool {
        (0..frame.columns as i32).contains(&cell.0) && (0..frame.rows as i32).contains(&cell.1)
    }
    fn link_at(&self, x: f64, y: f64) -> Option<String> {
        let frame = self.frame.borrow();
        let frame = frame.as_ref()?;
        let (column, row) = self.cell_at(x, y);
        let rows = self.rows.borrow();
        let cells = &frame.cells[rows.get(usize::try_from(row).ok()?)?.clone()];
        if let Some(link) = cells.iter().find(|c| (c.x as i32..c.x as i32 + c.width.max(1) as i32).contains(&column)).and_then(|c| c.hyperlink.clone()) {
            return Some(link);
        }
        // Plain URLs: find the whitespace-delimited word under the pointer.
        let mut line = vec![' '; frame.columns as usize];
        for cell in cells {
            if let (Some(slot), Some(c)) = (line.get_mut(cell.x as usize), cell.text.chars().next()) {
                *slot = c;
            }
        }
        let column = usize::try_from(column).ok().filter(|c| *c < line.len())?;
        if line[column].is_whitespace() {
            return None;
        }
        let start = line[..column].iter().rposition(|c| c.is_whitespace()).map_or(0, |i| i + 1);
        let end = line[column..].iter().position(|c| c.is_whitespace()).map_or(line.len(), |i| column + i);
        let word: String = line[start..end].iter().collect();
        let word = word.trim_matches(|c: char| "()[]<>\"'`,.;:!?".contains(c));
        (word.starts_with("https://") || word.starts_with("http://")).then(|| word.to_owned())
    }

    fn clicked(&self, x: f64, y: f64, shift: bool) {
        if let Some(link) = self.link_at(x, y) {
            self.emit(Event::OpenLink(link));
            return;
        }
        if !self.controlled.get() {
            self.emit(Event::Claim);
            return;
        }
        let frame = self.frame.borrow();
        let Some(frame) = frame.as_ref() else { return };
        let cell = self.cell_at(x, y);
        let below = frame.scroll_total.saturating_sub(frame.scroll_visible).saturating_sub(frame.scroll_offset);
        if frame.input_flags & 1 != 0 && below == 0 && !shift && Self::in_grid(frame, cell) {
            let metrics = self.metrics.get();
            self.emit(Event::Click { column: cell.0, row: cell.1, cell_width: metrics.width.round() as i32, cell_height: metrics.height.round() as i32 });
        }
    }
    fn wheel(&self, dy: f64, shift: bool) {
        if self.selection.borrow().is_some() {
            return;
        }
        let total = self.scroll.get() + dy * 3.0;
        let lines = total.trunc() as i32;
        self.scroll.set(total - lines as f64);
        if lines == 0 {
            return;
        }
        let frame = self.frame.borrow();
        let Some(frame) = frame.as_ref() else { return };
        let below = frame.scroll_total.saturating_sub(frame.scroll_visible).saturating_sub(frame.scroll_offset);
        let application = frame.input_flags & 1 != 0 || frame.input_flags & 6 == 6;
        if application && below == 0 && !shift {
            let metrics = self.metrics.get();
            let (x, y) = self.pointer.get();
            let cell = self.cell_at(x, y);
            self.emit(Event::Wheel { lines, column: cell.0, row: cell.1, cell_width: metrics.width.round() as i32, cell_height: metrics.height.round() as i32 });
        } else {
            self.emit(Event::Scroll(lines));
        }
    }

    fn clear_selection(&self) {
        if self.selection.take().is_some() {
            self.area.queue_draw();
        }
    }
    fn clamp(frame: &Frame, cell: (i32, i32)) -> (i32, i32) {
        (cell.0.clamp(0, frame.columns as i32 - 1), cell.1.clamp(0, frame.rows as i32 - 1))
    }
    fn begin_selection(&self, at: (i32, i32)) -> bool {
        let Some(frame) = self.frame.borrow().clone() else { return false };
        if frame.columns == 0 || frame.rows == 0 {
            return false;
        }
        let at = Self::clamp(&frame, at);
        self.selection.replace(Some(Selection { rows: row_ranges(&frame), frame, anchor: at, extent: at }));
        true
    }
    fn drag(&self, x0: f64, y0: f64, x: f64, y: f64) {
        let metrics = self.metrics.get();
        if self.selection.borrow().is_none() {
            if (x - x0).abs() < metrics.width / 2.0 && (y - y0).abs() < metrics.height / 2.0 {
                return;
            }
            if !self.begin_selection(self.cell_at(x0, y0)) {
                return;
            }
        }
        let cell = self.cell_at(x, y);
        if let Some(selection) = self.selection.borrow_mut().as_mut() {
            selection.extent = Self::clamp(&selection.frame, cell);
        }
        self.area.queue_draw();
    }
    fn select_word(&self, x: f64, y: f64) {
        let at = self.cell_at(x, y);
        if !self.begin_selection(at) {
            return;
        }
        if let Some(selection) = self.selection.borrow_mut().as_mut() {
            let (column, row) = selection.anchor;
            let cells = &selection.frame.cells[selection.rows[row as usize].clone()];
            let blank = |x: i32| cells.iter().find(|c| c.x as i32 == x).is_none_or(|c| c.text.trim().is_empty());
            if !blank(column) {
                let (mut left, mut right) = (column, column);
                while left > 0 && !blank(left - 1) {
                    left -= 1;
                }
                while right < selection.frame.columns as i32 - 1 && !blank(right + 1) {
                    right += 1;
                }
                selection.anchor = (left, row);
                selection.extent = (right, row);
            }
        }
        self.area.queue_draw();
    }
    fn copy(&self) -> bool {
        let Some(text) = self.selection.borrow().as_ref().map(Selection::text).filter(|t| !t.is_empty()) else { return false };
        self.area.clipboard().set_text(&text);
        self.clear_selection();
        true
    }
    fn paste(&self) {
        self.paste_from(&self.area.clipboard());
    }
    fn paste_from(&self, clipboard: &gdk::Clipboard) {
        let weak = self.this.clone();
        // A copied picture has no text to paste: hand it over as a file instead.
        let formats = clipboard.formats();
        // Another program's clipboard is described by media types; this program's own by value types.
        let media = formats.mime_types();
        let image = formats.contains_type(gdk::Texture::static_type()) || media.iter().any(|m| m.starts_with("image/"));
        // Text means words: a picture offered as "text/ico" is still a picture.
        let text = formats.contains_type(glib::Type::STRING) || media.iter().any(|m| m.starts_with("text/plain") || matches!(m.as_str(), "UTF8_STRING" | "STRING" | "TEXT" | "text/uri-list"));
        if image && !text {
            clipboard.read_texture_async(gtk::gio::Cancellable::NONE, move |result| {
                let (Some(inner), Ok(Some(texture))) = (weak.upgrade(), result) else { return };
                let directory = glib::user_cache_dir().join("rexling");
                let path = directory.join(format!("pasted-{:x}.png", glib::monotonic_time()));
                if std::fs::create_dir_all(&directory).is_ok() && texture.save_to_png(&path).is_ok() {
                    inner.emit(Event::Upload(path));
                }
            });
            return;
        }
        clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |result| {
            let Some(inner) = weak.upgrade() else { return };
            if let Ok(Some(text)) = result {
                inner.emit(Event::Paste(text.to_string()));
            }
        });
    }

    /// Returns whether the key was handled here. Unhandled keys reach the input method, which commits text.
    fn key(&self, key: gdk::Key, state: gdk::ModifierType) -> bool {
        let control = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = state.contains(gdk::ModifierType::ALT_MASK);
        self.restart_blink();
        if shift && !control && matches!(key, gdk::Key::Page_Up | gdk::Key::Page_Down) {
            let page = self.grid.get().1.max(1) as i32;
            self.emit(Event::Scroll(if key == gdk::Key::Page_Up { -page } else { page }));
            return true;
        }
        if control && shift {
            match key {
                gdk::Key::C | gdk::Key::c => {
                    self.copy();
                    return true;
                }
                gdk::Key::V | gdk::Key::v => {
                    self.paste();
                    return true;
                }
                _ => {}
            }
        }
        if shift && key == gdk::Key::Insert {
            self.paste();
            return true;
        }
        if control && !alt {
            match key {
                gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => {
                    self.emit(Event::Zoom(1));
                    return true;
                }
                gdk::Key::minus | gdk::Key::KP_Subtract => {
                    self.emit(Event::Zoom(-1));
                    return true;
                }
                gdk::Key::_0 | gdk::Key::KP_0 => {
                    self.emit(Event::Zoom(0));
                    return true;
                }
                _ => {}
            }
        }
        let named = match key {
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => "enter",
            gdk::Key::BackSpace => "backspace",
            gdk::Key::Delete | gdk::Key::KP_Delete => "delete",
            gdk::Key::Insert | gdk::Key::KP_Insert => "insert",
            gdk::Key::Page_Up | gdk::Key::KP_Page_Up => "pageup",
            gdk::Key::Page_Down | gdk::Key::KP_Page_Down => "pagedown",
            gdk::Key::Tab | gdk::Key::KP_Tab | gdk::Key::ISO_Left_Tab => "tab",
            gdk::Key::Escape => "esc",
            gdk::Key::Up | gdk::Key::KP_Up => "up",
            gdk::Key::Down | gdk::Key::KP_Down => "down",
            gdk::Key::Left | gdk::Key::KP_Left => "left",
            gdk::Key::Right | gdk::Key::KP_Right => "right",
            gdk::Key::Home | gdk::Key::KP_Home => "home",
            gdk::Key::End | gdk::Key::KP_End => "end",
            gdk::Key::F1 => "f1",
            gdk::Key::F2 => "f2",
            gdk::Key::F3 => "f3",
            gdk::Key::F4 => "f4",
            gdk::Key::F5 => "f5",
            gdk::Key::F6 => "f6",
            gdk::Key::F7 => "f7",
            gdk::Key::F8 => "f8",
            gdk::Key::F9 => "f9",
            gdk::Key::F10 => "f10",
            gdk::Key::F11 => "f11",
            gdk::Key::F12 => "f12",
            _ => "",
        };
        if !named.is_empty() {
            self.clear_selection();
            self.emit(Event::ScrollToBottom);
            self.emit(Event::Input(input::key(named, control, alt, shift)));
            return true;
        }
        // Plain text is left to the input method; only Ctrl and Alt chords are encoded here.
        if (control || alt)
            && let Some(character) = key.to_unicode().filter(|c| !c.is_control()) {
                self.clear_selection();
                self.emit(Event::ScrollToBottom);
                self.emit(Event::Input(input::modified(character, control, alt)));
                return true;
            }
        false
    }
}

/// `REXLING_TIMING=1` prints how long terminal redraws take, for performance work.
mod timing {
    use std::{cell::RefCell, sync::OnceLock, time::Duration};

    thread_local! {
        static SAMPLES: RefCell<Vec<Duration>> = const { RefCell::new(Vec::new()) };
    }
    pub fn record(elapsed: Duration) {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        if !*ENABLED.get_or_init(|| std::env::var_os("REXLING_TIMING").is_some()) {
            return;
        }
        SAMPLES.with(|samples| {
            let mut samples = samples.borrow_mut();
            samples.push(elapsed);
            if samples.len() == 120 {
                samples.sort();
                let total: Duration = samples.iter().sum();
                eprintln!("rexling: 120 redraws: mean {:.2} ms, median {:.2} ms, slowest {:.2} ms",
                    total.as_secs_f64() * 1000.0 / 120.0, samples[60].as_secs_f64() * 1000.0, samples[119].as_secs_f64() * 1000.0);
                samples.clear();
            }
        });
    }
}
/// The rectangles of a block element, as shares of its cell, and how solid it is.
fn block(character: char) -> Option<(Vec<[f64; 4]>, f64)> {
    const UPPER_LEFT: [f64; 4] = [0.0, 0.0, 0.5, 0.5];
    const UPPER_RIGHT: [f64; 4] = [0.5, 0.0, 0.5, 0.5];
    const LOWER_LEFT: [f64; 4] = [0.0, 0.5, 0.5, 0.5];
    const LOWER_RIGHT: [f64; 4] = [0.5, 0.5, 0.5, 0.5];
    let code = character as u32;
    let solid = |parts: Vec<[f64; 4]>| Some((parts, 1.0));
    match code {
        0x2580 => solid(vec![[0.0, 0.0, 1.0, 0.5]]),
        0x2581..=0x2588 => {
            let eighths = (code - 0x2580) as f64 / 8.0;
            solid(vec![[0.0, 1.0 - eighths, 1.0, eighths]])
        }
        0x2589..=0x258f => solid(vec![[0.0, 0.0, (0x2590 - code) as f64 / 8.0, 1.0]]),
        0x2590 => solid(vec![[0.5, 0.0, 0.5, 1.0]]),
        0x2591..=0x2593 => Some((vec![[0.0, 0.0, 1.0, 1.0]], (code - 0x2590) as f64 / 4.0)),
        0x2594 => solid(vec![[0.0, 0.0, 1.0, 0.125]]),
        0x2595 => solid(vec![[0.875, 0.0, 0.125, 1.0]]),
        0x2596 => solid(vec![LOWER_LEFT]),
        0x2597 => solid(vec![LOWER_RIGHT]),
        0x2598 => solid(vec![UPPER_LEFT]),
        0x2599 => solid(vec![UPPER_LEFT, LOWER_LEFT, LOWER_RIGHT]),
        0x259a => solid(vec![UPPER_LEFT, LOWER_RIGHT]),
        0x259b => solid(vec![UPPER_LEFT, UPPER_RIGHT, LOWER_LEFT]),
        0x259c => solid(vec![UPPER_LEFT, UPPER_RIGHT, LOWER_RIGHT]),
        0x259d => solid(vec![UPPER_RIGHT]),
        0x259e => solid(vec![UPPER_RIGHT, LOWER_LEFT]),
        0x259f => solid(vec![UPPER_RIGHT, LOWER_LEFT, LOWER_RIGHT]),
        _ => None,
    }
}

