//! Lays panes out by the fractions of the tab the server gives each one, with a hairline between
//! neighbours that can be dragged to resize them.
use crate::client::Split;
use gtk::{gdk, glib, prelude::*, subclass::prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Called with a border and the share of its area the first side should get.
type ResizeHandler = Rc<dyn Fn(&Split, f64)>;

/// How close to a border, in pixels, the pointer grabs it.
const GRAB: f64 = 5.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PaneGrid {
        pub panes: RefCell<Vec<(gtk::Widget, [f64; 4])>>,
        pub splits: RefCell<Vec<Split>>,
        pub dragging: RefCell<Option<Split>>,
        pub on_resize: RefCell<Option<ResizeHandler>>,
        pub last_ratio: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PaneGrid {
        const NAME: &'static str = "RexlingPaneGrid";
        type Type = super::PaneGrid;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_css_name("panegrid");
        }
    }

    impl ObjectImpl for PaneGrid {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().wire();
        }
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for PaneGrid {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            for (child, rect) in self.panes.borrow().iter() {
                let edge = |fraction: f64, size: i32| (fraction.clamp(0.0, 1.0) * size as f64).round() as i32;
                let (left, top) = (edge(rect[0], width), edge(rect[1], height));
                let (right, bottom) = (edge(rect[0] + rect[2], width), edge(rect[1] + rect[3], height));
                // A pane that does not start at the tab's edge gives up one pixel for the divider.
                let (x, y) = (left + i32::from(left > 0), top + i32::from(top > 0));
                let _ = child.measure(gtk::Orientation::Horizontal, -1);
                child.size_allocate(&gtk::Allocation::new(x, y, (right - x).max(1), (bottom - y).max(1)), -1);
            }
        }
    }
}

glib::wrapper! {
    pub struct PaneGrid(ObjectSubclass<imp::PaneGrid>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PaneGrid {
    fn default() -> Self {
        glib::Object::builder().property("hexpand", true).property("vexpand", true).property("overflow", gtk::Overflow::Hidden).build()
    }
}

impl PaneGrid {
    /// The border under the pointer, if any.
    fn split_at(&self, x: f64, y: f64) -> Option<Split> {
        let (width, height) = (self.width() as f64, self.height() as f64);
        self.imp().splits.borrow().iter().rev().find(|split| {
            let [left, top, w, h] = split.rect;
            let (left, top, w, h) = (left * width, top * height, w * width, h * height);
            if split.direction == "horizontal" {
                (x - (left + w * split.ratio)).abs() <= GRAB && y >= top && y <= top + h
            } else {
                (y - (top + h * split.ratio)).abs() <= GRAB && x >= left && x <= left + w
            }
        }).cloned()
    }
    fn wire(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        motion.connect_motion(move |_, x, y| {
            let Some(grid) = weak.upgrade() else { return };
            let held = grid.imp().dragging.borrow().clone();
            let name = match held.or_else(|| grid.split_at(x, y)) {
                Some(split) if split.direction == "horizontal" => Some("col-resize"),
                Some(_) => Some("row-resize"),
                None => None,
            };
            grid.set_cursor_from_name(name);
        });
        self.add_controller(motion);

        // Capture phase: a press on a border belongs to the border, not to the terminal under it.
        let drag = gtk::GestureDrag::builder().button(gdk::BUTTON_PRIMARY).propagation_phase(gtk::PropagationPhase::Capture).build();
        let weak = self.downgrade();
        drag.connect_drag_begin(move |gesture, x, y| {
            let Some(grid) = weak.upgrade() else { return };
            match grid.split_at(x, y) {
                Some(split) => {
                    grid.imp().last_ratio.set(split.ratio);
                    grid.imp().dragging.replace(Some(split));
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
                None => {
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            }
        });
        let weak = self.downgrade();
        drag.connect_drag_update(move |gesture, dx, dy| {
            let (Some(grid), Some((x, y))) = (weak.upgrade(), gesture.start_point()) else { return };
            let imp = grid.imp();
            let Some(split) = imp.dragging.borrow().clone() else { return };
            let [left, top, w, h] = split.rect;
            let ratio = if split.direction == "horizontal" {
                ((x + dx) / grid.width().max(1) as f64 - left) / w.max(f64::EPSILON)
            } else {
                ((y + dy) / grid.height().max(1) as f64 - top) / h.max(f64::EPSILON)
            };
            let ratio = ratio.clamp(0.08, 0.92);
            // Whole-percent steps keep a drag from flooding the server with resizes.
            if (ratio - imp.last_ratio.get()).abs() >= 0.01 {
                imp.last_ratio.set(ratio);
                let handler = imp.on_resize.borrow().clone();
                if let Some(handler) = handler {
                    handler(&split, ratio);
                }
            }
        });
        let weak = self.downgrade();
        drag.connect_drag_end(move |_, _, _| {
            if let Some(grid) = weak.upgrade() {
                grid.imp().dragging.replace(None);
            }
        });
        self.add_controller(drag);
    }
    /// Called with a border and the share of its area the first side should get, as the border is dragged.
    pub fn connect_resize(&self, handler: impl Fn(&Split, f64) + 'static) {
        self.imp().on_resize.replace(Some(Rc::new(handler)));
    }
    pub fn set_splits(&self, splits: Vec<Split>) {
        self.imp().splits.replace(splits);
    }

    /// Shows exactly these panes, back to front. Widgets already shown keep their state.
    pub fn arrange(&self, panes: Vec<(gtk::Widget, [f64; 4])>) {
        let imp = self.imp();
        for (old, _) in imp.panes.borrow().iter() {
            if !panes.iter().any(|(widget, _)| widget == old) {
                old.unparent();
            }
        }
        for (widget, _) in &panes {
            if widget.parent().is_none() {
                widget.set_parent(self);
            }
        }
        // Later panes float above earlier ones.
        for pair in panes.windows(2) {
            pair[1].0.insert_after(self, Some(&pair[0].0));
        }
        imp.panes.replace(panes);
        self.queue_allocate();
    }
}
