//! Selections on the page: HaikuPaint CanvasView's marquee and lasso, the
//! drags MacPaint's DragImage does with them, the Edit menu operations,
//! and the marching ants. While a selection floats, the canvas shows it
//! stamped over `base`, the page underneath.

use rustpaint_engine::geometry::{AxisLock, constrain, near_point, trunc_to_grid};
use rustpaint_engine::selection::map_rect;
use rustpaint_engine::{Canvas, Point, Rect, Selection};

use super::{MAX_POLYGON_POINTS, Modifiers, PageView};
use crate::tool::Tool;

/// Edit menu operations on the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionCommand {
    Invert,
    Fill,
    TraceEdges,
    TraceEdgesShadow,
    FlipHorizontal,
    FlipVertical,
    Rotate,
}

/// What a selection-tool drag is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Drag {
    #[default]
    None,
    /// Making a rectangular selection.
    Marquee,
    /// Drawing a lasso loop.
    Lasso,
    /// Moving the selection.
    Move,
    /// Moving a copy, leaving the original (Option).
    Copy,
    /// Leaving copies along the way (Option and Command).
    Trail,
    /// Stretching a rect selection (Command).
    Stretch,
}

impl Drag {
    /// Whether the selection itself is being dragged.
    fn moves_selection(self) -> bool {
        matches!(self, Drag::Move | Drag::Copy | Drag::Trail | Drag::Stretch)
    }
}

pub(super) struct SelectionState {
    selection: Option<Selection>,
    /// The selection's edges, relative to its rect.
    edges: Vec<Point>,
    base: Canvas,
    floating: bool,
    pub(super) drag: Drag,
    drag_start_rect: Rect,
    stretch_source: Selection,
    anchor: Point,
    keep_width: bool,
    keep_height: bool,

    /// Pixels drawn over the page: ants, or the lasso's black trail.
    overlay: Vec<Point>,
    overlay_ants: bool,
    ants_phase: u32,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selection: None,
            edges: Vec::new(),
            base: Canvas::new(0, 0),
            floating: false,
            drag: Drag::None,
            drag_start_rect: Rect::default(),
            stretch_source: Selection::default(),
            anchor: Point::default(),
            keep_width: false,
            keep_height: false,
            overlay: Vec::new(),
            overlay_ants: false,
            ants_phase: 0,
        }
    }
}

/// MacPaint's GetSelPat: $F8 rotated right by the phase, and one bit
/// further left on each row, giving diagonal stripes that march as the
/// phase steps.
pub fn ant_bit(x: i32, y: i32, phase: u32) -> bool {
    let row = 0xF8u8.rotate_right(phase).rotate_left((y & 7) as u32);
    (row >> (7 - (x & 7))) & 1 != 0
}

/// The frame of rect, as marching ants walk it.
fn frame_pixels(rect: Rect) -> Vec<Point> {
    let mut pixels = Vec::new();
    if rect.is_empty() {
        return pixels;
    }
    for x in rect.left..rect.right {
        pixels.push(Point::new(x, rect.top));
        if rect.height() > 1 {
            pixels.push(Point::new(x, rect.bottom - 1));
        }
    }
    for y in rect.top + 1..rect.bottom - 1 {
        pixels.push(Point::new(rect.left, y));
        if rect.width() > 1 {
            pixels.push(Point::new(rect.right - 1, y));
        }
    }
    pixels
}

/// The pixels of a 1 pixel line, for the lasso's trail.
fn line_pixels(from: Point, to: Point) -> Vec<Point> {
    let mut pixels = Vec::new();
    let dx = (to.x - from.x).abs();
    let dy = -(to.y - from.y).abs();
    let sx = if from.x < to.x { 1 } else { -1 };
    let sy = if from.y < to.y { 1 } else { -1 };
    let mut err = dx + dy;
    let mut point = from;
    loop {
        pixels.push(point);
        if point == to {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            point.x += sx;
        }
        if e2 <= dx {
            err += dx;
            point.y += sy;
        }
    }
    pixels
}

impl PageView {
    pub fn has_selection(&self) -> bool {
        self.sel.selection.is_some()
    }

    pub fn selection(&self) -> Option<&Selection> {
        self.sel.selection.as_ref()
    }

    /// The pixels to draw over the page, whether they're marching ants
    /// (else they're black), and the ants' phase.
    pub fn overlay(&self) -> (&[Point], bool, u32) {
        (
            &self.sel.overlay,
            self.sel.overlay_ants,
            self.sel.ants_phase,
        )
    }

    /// Whether there are ants to march or a caret to blink, about 25 times
    /// a second.
    pub fn needs_ticks(&self) -> bool {
        (self.sel.overlay_ants && !self.sel.overlay.is_empty()) || self.text_active()
    }

    /// Steps the marching ants, or the caret's blink.
    pub fn tick(&mut self) {
        if !self.blink_caret() {
            self.sel.ants_phase = (self.sel.ants_phase + 1) & 7;
        }
    }

    pub(super) fn sel_overlay_is_empty(&self) -> bool {
        self.sel.overlay.is_empty()
    }

    /// Whether the pointer shows the arrow at this page point: over a
    /// selection, as in MacPaint's TrackCursor, or while dragging one.
    pub fn shows_arrow(&self, point: Point) -> bool {
        self.sel.drag.moves_selection() || (self.sel.drag == Drag::None && self.in_selection(point))
    }

    /// Edit > Select All. MacPaint selects the whole window.
    pub fn select_all(&mut self) {
        if self.polygon_active {
            self.finish_polygon(false);
        }
        self.accept_selection();
        let selection = Selection::from_rect(&self.canvas, self.visible);
        if selection.rect.is_empty() {
            return;
        }
        self.sel.selection = Some(selection);
        self.sel.floating = false;
        self.selection_changed();
    }

    /// Places pasted pixels as a floating selection, centered in the
    /// window as MacPaint's CenterScrap does.
    pub fn paste(&mut self, mut pasted: Selection) {
        let visible = self.visible;
        let (width, height) = (pasted.rect.width(), pasted.rect.height());
        let mut left = (visible.left + visible.right - width) / 2;
        let mut top = (visible.top + visible.bottom - height) / 2;
        if self.grid {
            left = trunc_to_grid(left + 4);
            top = trunc_to_grid(top + 4);
        }
        pasted.rect = Rect::new(left, top, left + width, top + height);

        if self.polygon_active {
            self.finish_polygon(false);
        }
        self.accept_selection();
        self.begin_operation();
        self.place_selection(pasted);
    }

    /// Clears the selection from the page: Edit > Clear, Cut, Backspace.
    pub fn clear_selection(&mut self) {
        let Some(selection) = &self.sel.selection else {
            return;
        };
        let rect = selection.rect;
        let selection = selection.clone();
        self.begin_operation();
        if self.sel.floating {
            self.canvas.copy_rect(&self.sel.base, rect);
        } else {
            selection.erase_under(&mut self.canvas);
        }
        self.accept_selection();
    }

    pub fn apply_to_selection(&mut self, command: SelectionCommand) {
        if self.sel.selection.is_none() {
            return;
        }
        self.begin_operation();
        self.lift();
        let pattern = self.pattern;
        let Some(selection) = &mut self.sel.selection else {
            return;
        };
        let old = selection.rect;
        match command {
            SelectionCommand::Invert => selection.invert(),
            SelectionCommand::Fill => selection.fill(pattern),
            SelectionCommand::TraceEdges => selection.trace_edges(false),
            SelectionCommand::TraceEdgesShadow => selection.trace_edges(true),
            SelectionCommand::FlipHorizontal => selection.flip_horizontal(),
            SelectionCommand::FlipVertical => selection.flip_vertical(),
            SelectionCommand::Rotate => selection.rotate_left(),
        }
        self.canvas.copy_rect(&self.sel.base, old);
        selection.stamp(&mut self.canvas);
        self.selection_changed();
    }

    /// AcceptEdits: the canvas already shows the selection where it is, so
    /// accepting only forgets it.
    pub fn accept_selection(&mut self) {
        self.accept_text();
        if self.sel.selection.is_none() {
            return;
        }
        self.sel.selection = None;
        self.sel.edges.clear();
        self.sel.base = Canvas::new(0, 0);
        self.sel.floating = false;
        self.sel.drag = Drag::None;
        self.set_overlay(Vec::new(), false);
    }

    pub(super) fn selection_mouse_down(
        &mut self,
        point: Point,
        unsnapped: Point,
        modifiers: Modifiers,
    ) {
        // EditDoc: inside the selection, drag it; elsewhere, make a new
        // one.
        if self.sel.selection.is_some() && self.in_selection(unsnapped) {
            self.begin_selection_drag(unsnapped, modifiers);
            return;
        }

        self.accept_selection();
        self.start = point;
        self.last = point;
        if self.tool == Tool::Select {
            self.sel.drag = Drag::Marquee;
            let rect = Rect::new(point.x, point.y, point.x + 1, point.y + 1);
            self.set_overlay(frame_pixels(rect), true);
        } else {
            self.sel.drag = Drag::Lasso;
            self.points = vec![point];
            self.set_overlay(vec![point], false);
        }
    }

    pub(super) fn drag_marquee(&mut self, point: Point) {
        let mut rect = Rect::from_points(self.start, point);
        rect.right += 1;
        rect.bottom += 1;
        self.set_overlay(frame_pixels(rect), true);
    }

    pub(super) fn drag_lasso(&mut self, point: Point) {
        let last = self.last;
        if (point.x - last.x).abs() + (point.y - last.y).abs() > 1
            && self.points.len() < MAX_POLYGON_POINTS
        {
            self.sel.overlay.extend(line_pixels(last, point));
            self.points.push(point);
            self.last = point;
        }
    }

    fn begin_selection_drag(&mut self, point: Point, modifiers: Modifiers) {
        let Some(selection) = &self.sel.selection else {
            return;
        };
        let lasso = selection.lasso;

        // DragImage's modes.
        self.sel.drag = match (modifiers.option, modifiers.command) {
            (true, true) => Drag::Trail,
            (true, false) => Drag::Copy,
            (false, true) if !lasso => Drag::Stretch,
            _ => Drag::Move,
        };

        self.begin_operation();
        if matches!(self.sel.drag, Drag::Copy | Drag::Trail) {
            // The current state, original included, becomes the page.
            self.sel.base = self.canvas.clone();
            self.sel.floating = true;
        } else {
            self.lift();
        }

        self.start = point;
        self.last = Point::new(-100_000, -100_000);
        let rect = self
            .sel
            .selection
            .as_ref()
            .map_or(Rect::default(), |s| s.rect);
        self.sel.drag_start_rect = rect;
        self.axis_lock = AxisLock::new(point);

        if self.sel.drag == Drag::Stretch {
            // Grabbing an outer third anchors the opposite side; the middle
            // third keeps that dimension. The middle of both just moves.
            let third = rect.width() / 3;
            self.sel.keep_width = false;
            if point.x < rect.left + third {
                self.sel.anchor.x = rect.right;
            } else if point.x > rect.right - third {
                self.sel.anchor.x = rect.left;
            } else {
                self.sel.keep_width = true;
            }

            let third = rect.height() / 3;
            self.sel.keep_height = false;
            if point.y < rect.top + third {
                self.sel.anchor.y = rect.bottom;
            } else if point.y > rect.bottom - third {
                self.sel.anchor.y = rect.top;
            } else {
                self.sel.keep_height = true;
            }

            if self.sel.keep_width && self.sel.keep_height {
                self.sel.drag = Drag::Move;
            } else if let Some(selection) = &self.sel.selection {
                self.sel.stretch_source = selection.clone();
            }
        }
    }

    pub(super) fn drag_selection(&mut self, mut point: Point, modifiers: Modifiers) {
        let drag = self.sel.drag;
        if drag != Drag::Stretch && modifiers.shift {
            point = self.axis_lock.apply(point);
        }

        // Leaving a trail spaces the copies out by the line size.
        let tolerance = match (drag, self.line_width) {
            (Drag::Trail, 1) => 1,
            (Drag::Trail, width) => 2 * width,
            _ => 0,
        };
        if near_point(point, self.last, tolerance) {
            return;
        }
        self.last = point;

        let (page_width, page_height) = (self.canvas.width(), self.canvas.height());
        let Some(selection) = &mut self.sel.selection else {
            return;
        };
        let old = selection.rect;
        let start_rect = self.sel.drag_start_rect;
        if drag == Drag::Stretch {
            let anchor = self.sel.anchor;
            let mut start = self.start;
            if modifiers.shift {
                // Keep the aspect ratio.
                start = constrain(anchor, start, false);
                point = constrain(anchor, point, false);
            }
            let from = Rect::from_points(anchor, start);
            let mut to = Rect::from_points(anchor, point);
            if self.sel.keep_width {
                to.left = from.left;
                to.right = from.right;
            }
            if self.sel.keep_height {
                to.top = from.top;
                to.bottom = from.bottom;
            }
            let rect = map_rect(start_rect, from, to);
            if rect.is_empty() {
                return;
            }
            *selection = self.sel.stretch_source.stretched(rect);
        } else {
            // Pinned so the selection stays on the page (or, if stretched
            // bigger than the page, at least its top left corner does).
            // With the grid on, moves go in steps of 8 (DragImage sets
            // toolGrid for every selection).
            let mut dx = point.x - self.start.x;
            let mut dy = point.y - self.start.y;
            if self.grid {
                dx = trunc_to_grid(dx);
                dy = trunc_to_grid(dy);
            }
            dx = dx.min(page_width - start_rect.right).max(-start_rect.left);
            dy = dy.min(page_height - start_rect.bottom).max(-start_rect.top);
            selection.rect = Rect::new(
                start_rect.left + dx,
                start_rect.top + dy,
                start_rect.right + dx,
                start_rect.bottom + dy,
            );
        }

        if drag != Drag::Trail {
            self.canvas.copy_rect(&self.sel.base, old);
        }
        selection.stamp(&mut self.canvas);
        if drag == Drag::Trail {
            selection.stamp(&mut self.sel.base);
        }

        if drag == Drag::Stretch {
            self.selection_changed();
        } else {
            let rect = selection.rect;
            let edges = self
                .sel
                .edges
                .iter()
                .map(|edge| Point::new(rect.left + edge.x, rect.top + edge.y))
                .collect();
            self.set_overlay(edges, true);
        }
    }

    pub(super) fn end_selection_drag(&mut self, point: Point) {
        let drag = std::mem::take(&mut self.sel.drag);
        match drag {
            Drag::Marquee => {
                // SelDragImage: the rect includes the pixel under the
                // pointer.
                self.set_overlay(Vec::new(), false);
                let mut rect = Rect::from_points(self.start, point);
                if rect.is_empty() {
                    return;
                }
                rect.right += 1;
                rect.bottom += 1;
                let selection = Selection::from_rect(&self.canvas, rect);
                if selection.rect.is_empty() {
                    return;
                }
                self.sel.selection = Some(selection);
                self.sel.floating = false;
                self.selection_changed();
            }
            Drag::Lasso => {
                self.set_overlay(Vec::new(), false);
                if let Some(selection) = Selection::from_lasso(&self.canvas, &self.points) {
                    self.sel.selection = Some(selection);
                    self.sel.floating = false;
                    self.selection_changed();
                }
                self.points.clear();
            }
            _ => {}
        }
    }

    /// The first move or change leaves a white hole behind.
    fn lift(&mut self) {
        if self.sel.floating {
            return;
        }
        let Some(selection) = &self.sel.selection else {
            return;
        };
        self.sel.base = self.canvas.clone();
        selection.erase_under(&mut self.sel.base);
        self.sel.floating = true;
    }

    fn place_selection(&mut self, selection: Selection) {
        self.sel.base = self.canvas.clone();
        self.sel.floating = true;
        selection.stamp(&mut self.canvas);
        self.sel.selection = Some(selection);
        self.selection_changed();
    }

    fn selection_changed(&mut self) {
        let Some(selection) = &self.sel.selection else {
            return;
        };
        let edges = selection.edges();
        let rect = selection.rect;
        self.sel.edges = edges
            .iter()
            .map(|edge| Point::new(edge.x - rect.left, edge.y - rect.top))
            .collect();
        self.set_overlay(edges, true);
    }

    fn in_selection(&self, point: Point) -> bool {
        let Some(selection) = &self.sel.selection else {
            return false;
        };
        if !selection.lasso {
            let rect = &selection.rect;
            return point.x >= rect.left
                && point.x < rect.right
                && point.y >= rect.top
                && point.y < rect.bottom;
        }
        // TrackCursor counts a pixel next to the lasso mask too.
        (-1..=1)
            .any(|dy| (-1..=1).any(|dx| selection.contains(Point::new(point.x + dx, point.y + dy))))
    }

    pub(super) fn set_overlay(&mut self, points: Vec<Point>, ants: bool) {
        self.sel.overlay = points;
        self.sel.overlay_ants = ants;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::BLACK_PATTERN;

    const NONE: Modifiers = Modifiers {
        shift: false,
        option: false,
        command: false,
    };

    fn black_count(canvas: &Canvas) -> usize {
        canvas.pixels().iter().map(|&p| p as usize).sum()
    }

    fn drag(view: &mut PageView, points: &[(i32, i32)], modifiers: Modifiers) {
        let p = |&(x, y): &(i32, i32)| Point::new(x, y);
        view.mouse_down(p(&points[0]), modifiers);
        for point in &points[1..] {
            view.mouse_moved(p(point), modifiers);
        }
        view.mouse_up(p(points.last().unwrap()), modifiers);
    }

    /// A page with a 10 x 10 black square at (20, 20), and the marquee.
    fn square_page() -> PageView {
        let mut view = PageView::new();
        view.canvas.fill_rect(20, 20, 29, 29, BLACK_PATTERN);
        view.set_tool(Tool::Select);
        view
    }

    #[test]
    fn ants_march_in_diagonal_stripes() {
        // Five on, three off along a row, shifting with the phase and the
        // row.
        let row: Vec<bool> = (0..8).map(|x| ant_bit(x, 0, 0)).collect();
        assert_eq!(row, [true, true, true, true, true, false, false, false]);
        assert!(!ant_bit(0, 0, 1) && ant_bit(1, 0, 1));
        assert_eq!(ant_bit(1, 1, 0), ant_bit(0, 0, 0));
    }

    #[test]
    fn marquee_selects_and_moving_leaves_a_hole() {
        let mut view = square_page();
        drag(&mut view, &[(15, 15), (34, 34)], NONE);
        let selection = view.selection().expect("selected");
        assert_eq!(selection.rect, Rect::new(15, 15, 35, 35));
        assert!(view.needs_ticks() && view.overlay().0.len() == 4 * 19);
        assert!(view.shows_arrow(Point::new(20, 20)) && !view.shows_arrow(Point::new(50, 50)));

        drag(&mut view, &[(20, 20), (120, 70)], NONE);
        assert!(!view.canvas().get(25, 25) && view.canvas().get(125, 75));
        assert_eq!(black_count(view.canvas()), 100);
        assert_eq!(view.selection().unwrap().rect, Rect::new(115, 65, 135, 85));

        // Undo puts it back.
        view.undo();
        assert!(view.canvas().get(25, 25) && !view.canvas().get(125, 75));
        assert!(!view.has_selection());
    }

    #[test]
    fn option_drags_a_copy_and_command_stretches() {
        let mut view = square_page();
        drag(&mut view, &[(20, 20), (29, 29)], NONE);
        drag(
            &mut view,
            &[(25, 25), (125, 25)],
            Modifiers {
                option: true,
                ..NONE
            },
        );
        assert!(view.canvas().get(25, 25) && view.canvas().get(125, 25));
        assert_eq!(black_count(view.canvas()), 200);

        // Grabbing the right third of the copy anchors its left side; the
        // width scales with the pointer's distance from the anchor (MapRect):
        // grabbed 8 from it and dragged to 18, 10 pixels become 22.
        let command = Modifiers {
            command: true,
            ..NONE
        };
        drag(&mut view, &[(128, 25), (138, 25)], command);
        assert_eq!(view.selection().unwrap().rect, Rect::new(120, 20, 142, 30));
        assert_eq!(black_count(view.canvas()), 100 + 22 * 10);
    }

    #[test]
    fn lasso_selects_what_it_encloses() {
        let mut view = PageView::new();
        view.canvas.fill_rect(20, 20, 29, 29, BLACK_PATTERN);
        view.set_tool(Tool::Lasso);
        view.mouse_down(Point::new(10, 10), NONE);
        for (x, y) in [(40, 10), (40, 40), (10, 40), (10, 12)] {
            view.mouse_moved(Point::new(x, y), NONE);
        }
        // The trail shows while drawing, in black.
        assert!(!view.needs_ticks() && view.overlay().0.len() > 100);
        view.mouse_up(Point::new(10, 12), NONE);
        let selection = view.selection().expect("selected");
        assert!(selection.lasso && selection.rect == Rect::new(20, 20, 30, 30));
    }

    #[test]
    fn edit_operations_and_clear() {
        let mut view = square_page();
        drag(&mut view, &[(20, 20), (39, 29)], NONE);
        view.apply_to_selection(SelectionCommand::Invert);
        assert!(!view.canvas().get(25, 25) && view.canvas().get(35, 25));

        view.apply_to_selection(SelectionCommand::Rotate);
        assert_eq!(view.selection().unwrap().rect, Rect::new(25, 15, 35, 35));
        // Undo restores the page before the rotate (with the inversion).
        view.undo();
        assert!(!view.canvas().get(25, 25) && view.canvas().get(35, 25));

        view.select_all();
        assert_eq!(view.selection().unwrap().rect, Rect::new(0, 0, 576, 720));
        assert!(view.key_down(true));
        assert_eq!(black_count(view.canvas()), 0);
        assert!(!view.has_selection());
    }

    #[test]
    fn paste_centers_in_the_window_and_floats() {
        let mut view = PageView::new();
        view.canvas.fill_rect(0, 0, 575, 719, BLACK_PATTERN);
        view.set_visible(Rect::new(0, 0, 200, 100));
        let mut pixels = Canvas::new(10, 10);
        pixels.fill_rect(0, 0, 9, 9, rustpaint_engine::WHITE_PATTERN);
        let pasted = Selection::from_rect(&pixels, Rect::new(0, 0, 10, 10));
        view.paste(pasted);
        assert_eq!(view.selection().unwrap().rect, Rect::new(95, 45, 105, 55));
        assert!(!view.canvas().get(100, 50));

        // Clearing a floating paste reveals the page under it.
        view.clear();
        assert!(view.canvas().get(100, 50) && !view.has_selection());
    }
}
