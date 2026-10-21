//! The page and the tools that draw on it: a port of HaikuPaint's
//! CanvasView, which follows MacPaint's EditDoc and the tool procedures it
//! calls. All pixel logic belongs in the engine; this only turns mouse
//! input, in page coordinates, into engine calls.

use rustpaint_engine::canvas::mask_stroke_points;
use rustpaint_engine::geometry::{AxisLock, constrain, near_point, snap_to_grid};
use rustpaint_engine::paint_font::{DEFAULT_BRUSH, GLYPH_FIRST_BRUSH, GLYPH_SPRAY};
use rustpaint_engine::symmetry::{NO_SYMMETRY, symmetric_points};
use rustpaint_engine::{
    BLACK_PATTERN, Canvas, Mask16, PaintMode, Pattern, Point, Rect, Shape, WHITE_PATTERN,
};

use crate::art::mask_from_glyph;
use crate::tool::Tool;

/// MacPaint's eraser is its 16 x 16 cursor box, centered on the hotspot.
const ERASER_SIZE: i32 = 16;

/// MacPaint's PolyLine/CreatePoly limit.
const MAX_POLYGON_POINTS: usize = 1000;

/// MacPaint's modifier keys. On macOS they are Option and Command; on
/// Linux and Windows Ctrl and Alt stand in for them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub shift: bool,
    /// Draws lines and borders in the pattern.
    pub option: bool,
    /// Paints in OR mode.
    pub command: bool,
}

pub struct PageView {
    canvas: Canvas,
    /// The image before the current operation: MacPaint's one level of
    /// undo, and what live previews are restored from.
    undo: Canvas,
    can_undo: bool,

    tool: Tool,
    pattern: Pattern,
    line_width: i32,
    border: bool,
    brush_mask: Mask16,
    spray_mask: Mask16,
    grid: bool,
    symmetry: u8,
    mirror_center: Point,
    /// The part of the page in the window.
    visible: Rect,

    tracking: bool,
    pencil_black: bool,
    start: Point,
    last: Point,
    axis_lock: AxisLock,
    preview_rect: Rect,

    /// Freeform and polygon vertices, in mouse coordinates.
    points: Vec<Point>,
    polygon_active: bool,
}

impl PageView {
    pub fn new() -> Self {
        let canvas = Canvas::default();
        Self {
            undo: canvas.clone(),
            visible: Rect::new(0, 0, canvas.width(), canvas.height()),
            canvas,
            can_undo: false,
            tool: Tool::Pencil,
            pattern: BLACK_PATTERN,
            line_width: 1,
            border: true,
            brush_mask: mask_from_glyph(GLYPH_FIRST_BRUSH + DEFAULT_BRUSH),
            spray_mask: mask_from_glyph(GLYPH_SPRAY),
            grid: false,
            symmetry: NO_SYMMETRY,
            mirror_center: Point::default(),
            tracking: false,
            pencil_black: true,
            start: Point::default(),
            last: Point::default(),
            axis_lock: AxisLock::default(),
            preview_rect: Rect::default(),
            points: Vec::new(),
            polygon_active: false,
        }
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if self.polygon_active {
            self.finish_polygon(false);
        }
        self.tool = tool;
    }

    pub fn set_pattern(&mut self, pattern: Pattern) {
        self.pattern = pattern;
    }

    pub fn line_width(&self) -> i32 {
        self.line_width
    }

    pub fn border(&self) -> bool {
        self.border
    }

    pub fn set_line_width(&mut self, width: i32, border: bool) {
        self.line_width = width;
        self.border = border;
    }

    pub fn set_visible(&mut self, visible: Rect) {
        self.visible = visible;
    }

    /// Whether a polygon is being placed: its next edge follows the
    /// pointer with the button up.
    pub fn polygon_active(&self) -> bool {
        self.polygon_active
    }

    /// Clears the whole page, as double-clicking the eraser does.
    pub fn clear(&mut self) {
        if self.polygon_active {
            self.finish_polygon(false);
        }
        self.begin_operation();
        self.canvas.clear();
    }

    /// MacPaint's one level of undo, which undoing again redoes.
    pub fn undo(&mut self) {
        if self.polygon_active {
            self.finish_polygon(false);
        }
        if !self.can_undo {
            return;
        }
        std::mem::swap(&mut self.canvas, &mut self.undo);
        self.canvas.set_paint_mode(PaintMode::Copy);
    }

    /// A key went down. Returns whether the page used it.
    pub fn key_down(&mut self) -> bool {
        // As in MacPaint, any key ends a polygon in progress.
        if self.polygon_active {
            self.finish_polygon(false);
            return true;
        }
        false
    }

    pub fn mouse_down(&mut self, point: Point, modifiers: Modifiers) {
        // The selection tools, the grabber and text arrive in later
        // milestones.
        if matches!(
            self.tool,
            Tool::Lasso | Tool::Select | Tool::Grabber | Tool::Text
        ) {
            return;
        }

        let point = self.tool_point(point);
        self.tracking = true;

        // A polygon spans several clicks; vertices are added on mouse up.
        if self.tool.is_polygon() {
            if !self.polygon_active {
                self.begin_operation();
                self.begin_preview();
                self.set_paint_mode(modifiers);
                self.points = vec![point];
                self.polygon_active = true;
            }
            return;
        }

        self.start = point;
        self.last = point;
        self.axis_lock = AxisLock::new(point);
        self.begin_operation();
        self.set_paint_mode(modifiers);

        match self.tool {
            Tool::PaintBucket => {
                self.canvas.flood_fill(point.x, point.y, self.pattern);
                self.tracking = false;
            }
            Tool::Spray => self
                .canvas
                .stamp_mask(&self.spray_mask, point.x, point.y, self.pattern),
            Tool::Brush => {
                // Brush Mirrors reflect around the middle of the window.
                let visible = self.visible;
                self.mirror_center = Point::new(
                    (visible.left + visible.right) / 2,
                    (visible.top + visible.bottom) / 2,
                );
                self.stamp_brush(point);
            }
            Tool::Pencil => {
                // Like MacPaint: starting on a black pixel draws white.
                self.pencil_black = !self.canvas.get(point.x, point.y);
                self.canvas.set(point.x, point.y, self.pencil_black);
            }
            Tool::Eraser => self.erase(point, point),
            Tool::Freeform | Tool::FilledFreeform => {
                if self.tool == Tool::FilledFreeform {
                    self.begin_preview();
                    self.points = vec![point];
                }
                let pen = self.pen_point(point);
                let pattern = self.line_pattern(modifiers);
                self.canvas
                    .stroke_line(pen.x, pen.y, pen.x, pen.y, self.line_width, pattern);
                self.preview_rect = self.preview_rect.union(&self.pen_bounds(point, point));
            }
            // Line and the shape tools.
            _ => {
                self.begin_preview();
                self.drag_to(point, modifiers);
            }
        }
    }

    pub fn mouse_moved(&mut self, point: Point, modifiers: Modifiers) {
        let mut point = self.tool_point(point);

        // The edge being placed follows the pointer, button down or not.
        if self.polygon_active {
            if modifiers.shift {
                point = constrain(*self.points.last().unwrap(), point, true);
            }
            self.draw_polygon_preview(point, modifiers);
            return;
        }

        if !self.tracking {
            return;
        }

        let last = self.last;
        match self.tool {
            Tool::Pencil => {
                if modifiers.shift {
                    point = self.axis_lock.apply(point);
                }
                self.canvas
                    .draw_line(last.x, last.y, point.x, point.y, self.pencil_black);
                self.last = point;
            }
            Tool::Brush | Tool::Spray => {
                // MacPaint's BrushPaint: the brush paints along the path,
                // the spray can only where the pointer has moved to.
                if modifiers.shift {
                    point = self.axis_lock.apply(point);
                }
                let moved = (point.x - last.x).abs() + (point.y - last.y).abs();
                if self.tool == Tool::Brush && moved > 1 {
                    for step in mask_stroke_points(last.x, last.y, point.x, point.y) {
                        self.stamp_brush(step);
                    }
                    self.last = point;
                } else if self.tool == Tool::Spray && moved > 2 {
                    self.canvas
                        .stamp_mask(&self.spray_mask, point.x, point.y, self.pattern);
                    self.last = point;
                }
            }
            Tool::Eraser => {
                self.erase(last, point);
                self.last = point;
            }
            Tool::Freeform | Tool::FilledFreeform => {
                // HollowCurve and GetRgn: add a segment once the pointer
                // has moved far enough.
                let far_enough = if self.tool == Tool::Freeform {
                    !near_point(last, point, 3)
                } else {
                    (point.x - last.x).abs() + (point.y - last.y).abs() > 1
                };
                if !far_enough || self.points.len() >= MAX_POLYGON_POINTS {
                    return;
                }
                let from = self.pen_point(last);
                let to = self.pen_point(point);
                let pattern = self.line_pattern(modifiers);
                self.canvas
                    .stroke_line(from.x, from.y, to.x, to.y, self.line_width, pattern);
                self.preview_rect = self.preview_rect.union(&self.pen_bounds(last, point));
                if self.tool == Tool::FilledFreeform {
                    self.points.push(point);
                }
                self.last = point;
            }
            Tool::PaintBucket => {}
            _ => self.drag_to(point, modifiers),
        }
    }

    pub fn mouse_up(&mut self, point: Point, modifiers: Modifiers) {
        let mut point = self.tool_point(point);
        if self.polygon_active {
            if modifiers.shift {
                point = constrain(*self.points.last().unwrap(), point, true);
            }
            self.polygon_click(point, modifiers);
            self.tracking = false;
            return;
        }

        if !self.tracking {
            return;
        }
        self.tracking = false;

        if self.tool == Tool::FilledFreeform {
            self.finish_filled_freeform(modifiers);
        } else if self.tool == Tool::Line || self.tool.is_shape() {
            self.drag_to(point, modifiers);
        }
        self.canvas.set_paint_mode(PaintMode::Copy);
    }

    /// Called before anything changes the canvas: saves the undo image.
    fn begin_operation(&mut self) {
        self.undo = self.canvas.clone();
        self.can_undo = true;
    }

    // Live previews: the canvas is restored from the undo image under the
    // previous preview before each new one is drawn, as MacPaint does with
    // its alternate buffer.

    fn begin_preview(&mut self) {
        self.preview_rect = Rect::default();
    }

    fn restore_preview(&mut self) {
        self.canvas.copy_rect(&self.undo, self.preview_rect);
    }

    fn end_preview(&mut self, drawn: Rect) {
        self.preview_rect = drawn;
    }

    fn drag_to(&mut self, mut point: Point, modifiers: Modifiers) {
        if self.tool == Tool::Line {
            // StraightLine: Shift snaps to horizontal, vertical or 45
            // degrees.
            if modifiers.shift {
                point = constrain(self.start, point, true);
            }
            self.draw_line(point, modifiers);
        } else {
            // CreateShape: Shift makes squares and circles.
            if modifiers.shift {
                point = constrain(self.start, point, false);
            }
            self.draw_shape(point, modifiers);
        }
    }

    fn draw_line(&mut self, end: Point, modifiers: Modifiers) {
        self.restore_preview();
        let from = self.pen_point(self.start);
        let to = self.pen_point(end);
        let pattern = self.line_pattern(modifiers);
        self.canvas
            .stroke_line(from.x, from.y, to.x, to.y, self.line_width, pattern);
        let drawn = self.pen_bounds(self.start, end);
        self.end_preview(drawn);
    }

    fn draw_shape(&mut self, end: Point, modifiers: Modifiers) {
        // CreateShape: grow the dragged rect so the border is centered on
        // it.
        let rect = Rect::from_points(self.start, end);
        let width = self.line_width;
        let half = width / 2;
        let rect = Rect::new(
            rect.left - half,
            rect.top - half,
            rect.right + width - half,
            rect.bottom + width - half,
        );

        // The shape tools come in hollow/filled pairs, in Shape order.
        let pair = self.tool as usize - Tool::Rect as usize;
        let shape = [Shape::Rect, Shape::RoundRect, Shape::Oval][pair / 2];
        let filled = pair % 2 == 1;

        self.restore_preview();
        if filled {
            self.canvas.fill_shape(shape, rect, self.pattern);
        }
        if self.border || !filled {
            let pattern = self.line_pattern(modifiers);
            self.canvas.frame_shape(shape, rect, width, pattern);
        }
        self.end_preview(rect);
    }

    fn polygon_click(&mut self, point: Point, modifiers: Modifiers) {
        // CreatePoly: done when a vertex lands near the start (closing the
        // polygon) or near the previous vertex (a double-click).
        let previous = *self.points.last().unwrap();
        self.points.push(point);

        let mut closed = false;
        let mut done = self.points.len() >= MAX_POLYGON_POINTS;
        if self.points.len() >= 3 {
            let tolerance = 4 * self.line_width;
            closed = near_point(point, self.points[0], tolerance);
            done = done || closed || near_point(point, previous, tolerance);
        }

        if done {
            self.finish_polygon_with(closed, modifiers);
        } else {
            self.draw_polygon_preview(point, modifiers);
        }
    }

    fn draw_polygon_preview(&mut self, end: Point, modifiers: Modifiers) {
        self.restore_preview();
        let mut pen = self.pen_points();
        pen.push(self.pen_point(end));
        let pattern = self.line_pattern(modifiers);
        self.canvas
            .stroke_polyline(&pen, false, self.line_width, pattern);

        let mut points = self.points.clone();
        points.push(end);
        let drawn = self.points_bounds(&points);
        self.end_preview(drawn);
    }

    /// Ends a polygon from outside the mouse handlers, where there are no
    /// modifiers to go by.
    fn finish_polygon(&mut self, closed: bool) {
        self.finish_polygon_with(closed, Modifiers::default());
    }

    fn finish_polygon_with(&mut self, closed: bool, modifiers: Modifiers) {
        self.restore_preview();
        let pen = self.pen_points();
        let line_pattern = self.line_pattern(modifiers);
        if self.tool == Tool::FilledPolygon {
            // Filled polygons are always closed; without a border the
            // outline is drawn in the fill pattern.
            self.canvas.fill_polygon(&self.points, self.pattern);
            let outline = if self.border {
                line_pattern
            } else {
                self.pattern
            };
            self.canvas
                .stroke_polyline(&pen, true, self.line_width, outline);
        } else {
            self.canvas
                .stroke_polyline(&pen, closed, self.line_width, line_pattern);
        }
        let drawn = self.points_bounds(&self.points);
        self.end_preview(drawn);

        self.polygon_active = false;
        self.points.clear();
        self.canvas.set_paint_mode(PaintMode::Copy);
    }

    fn finish_filled_freeform(&mut self, modifiers: Modifiers) {
        // GetRgn: close the curve, fill it, then redraw its outline.
        self.restore_preview();
        self.canvas.fill_polygon(&self.points, self.pattern);
        let pen = self.pen_points();
        let outline = if self.border {
            self.line_pattern(modifiers)
        } else {
            self.pattern
        };
        self.canvas
            .stroke_polyline(&pen, true, self.line_width, outline);
        let drawn = self.points_bounds(&self.points);
        self.end_preview(drawn);
        self.points.clear();
    }

    /// Stamps the brush at point and its mirror images (SymBrush: mirrors
    /// apply to the brush only).
    fn stamp_brush(&mut self, point: Point) {
        for mirrored in symmetric_points(point, self.mirror_center, self.symmetry) {
            self.canvas
                .stamp_mask(&self.brush_mask, mirrored.x, mirrored.y, self.pattern);
        }
    }

    fn erase(&mut self, from: Point, to: Point) {
        // Center the eraser square on the pointer.
        let half = ERASER_SIZE / 2;
        self.canvas.stroke_line(
            from.x - half,
            from.y - half,
            to.x - half,
            to.y - half,
            ERASER_SIZE,
            WHITE_PATTERN,
        );
    }

    /// The mouse point as the current tool sees it: on the grid, for the
    /// tools that use it.
    fn tool_point(&self, point: Point) -> Point {
        if self.grid && self.tool.uses_grid() {
            snap_to_grid(point)
        } else {
            point
        }
    }

    fn pen_point(&self, point: Point) -> Point {
        // MacPaint centers its pen on the pointer.
        let half = self.line_width / 2;
        Point::new(point.x - half, point.y - half)
    }

    fn pen_points(&self) -> Vec<Point> {
        self.points
            .iter()
            .map(|&point| self.pen_point(point))
            .collect()
    }

    fn pen_bounds(&self, a: Point, b: Point) -> Rect {
        let rect = Rect::from_points(a, b);
        let half = self.line_width / 2;
        Rect::new(
            rect.left - half,
            rect.top - half,
            rect.right - half + self.line_width,
            rect.bottom - half + self.line_width,
        )
    }

    fn points_bounds(&self, points: &[Point]) -> Rect {
        points.iter().fold(Rect::default(), |bounds, &point| {
            bounds.union(&self.pen_bounds(point, point))
        })
    }

    fn set_paint_mode(&mut self, modifiers: Modifiers) {
        // JamLine/JamFill: Command paints in patOr, for everything but the
        // tools that don't use patterns that way.
        let or_mode = modifiers.command
            && !matches!(self.tool, Tool::Eraser | Tool::Pencil | Tool::PaintBucket);
        self.canvas.set_paint_mode(if or_mode {
            PaintMode::Or
        } else {
            PaintMode::Copy
        });
    }

    fn line_pattern(&self, modifiers: Modifiers) -> Pattern {
        // JamLine: lines and borders are black, or the pattern with Option.
        if modifiers.option {
            self.pattern
        } else {
            BLACK_PATTERN
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::patterns::PATTERNS;

    const NONE: Modifiers = Modifiers {
        shift: false,
        option: false,
        command: false,
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        option: false,
        command: false,
    };

    fn black_count(view: &PageView) -> usize {
        view.canvas().pixels().iter().map(|&p| p as usize).sum()
    }

    fn view_with(tool: Tool) -> PageView {
        let mut view = PageView::new();
        view.set_tool(tool);
        view
    }

    fn drag(view: &mut PageView, points: &[(i32, i32)], modifiers: Modifiers) {
        let p = |&(x, y): &(i32, i32)| Point::new(x, y);
        view.mouse_down(p(&points[0]), modifiers);
        for point in &points[1..] {
            view.mouse_moved(p(point), modifiers);
        }
        view.mouse_up(p(points.last().unwrap()), modifiers);
    }

    fn click(view: &mut PageView, x: i32, y: i32) {
        drag(view, &[(x, y)], NONE);
    }

    #[test]
    fn pencil_draws_and_starting_on_black_erases() {
        let mut view = PageView::new();
        drag(&mut view, &[(10, 10), (19, 10)], NONE);
        assert_eq!(black_count(&view), 10);

        // Starting on the line draws white.
        drag(&mut view, &[(12, 10), (15, 10)], NONE);
        assert_eq!(black_count(&view), 6);
    }

    #[test]
    fn pencil_with_shift_locks_to_an_axis() {
        let mut view = PageView::new();
        drag(&mut view, &[(10, 10), (20, 11), (30, 40)], SHIFT);
        assert!(view.canvas().get(30, 10) && !view.canvas().get(30, 40));
    }

    #[test]
    fn line_preview_leaves_only_the_last_line() {
        let mut view = view_with(Tool::Line);
        drag(&mut view, &[(10, 10), (100, 100), (50, 10), (40, 10)], NONE);
        assert_eq!(black_count(&view), 31);
        assert!(!view.canvas().get(50, 50) && !view.canvas().get(50, 10));

        // A thick line is centered on the pointer; Shift snaps it flat.
        let mut view = view_with(Tool::Line);
        view.set_line_width(4, true);
        drag(&mut view, &[(100, 100), (200, 104)], SHIFT);
        assert!(view.canvas().get(98, 98) && view.canvas().get(201, 101));
        assert!(!view.canvas().get(98, 102) && !view.canvas().get(200, 104));
    }

    #[test]
    fn line_paints_the_pattern_with_option() {
        let mut view = view_with(Tool::Line);
        view.set_line_width(8, true);
        view.set_pattern(PATTERNS[3]);
        drag(
            &mut view,
            &[(100, 100), (200, 100)],
            Modifiers {
                option: true,
                ..NONE
            },
        );
        let black = black_count(&view);
        assert!(black > 0 && black < 108 * 8);
    }

    #[test]
    fn eraser_and_bucket() {
        let mut view = view_with(Tool::PaintBucket);
        click(&mut view, 5, 5);
        assert_eq!(black_count(&view), 576 * 720);

        view.set_tool(Tool::Eraser);
        drag(&mut view, &[(100, 100), (100, 100)], NONE);
        assert_eq!(black_count(&view), 576 * 720 - 16 * 16);
        assert!(!view.canvas().get(92, 92) && !view.canvas().get(107, 107));

        view.clear();
        assert_eq!(black_count(&view), 0);
    }

    #[test]
    fn shapes_frame_fill_and_constrain() {
        // A hollow rect's 1 pixel border covers the dragged corners.
        let mut view = view_with(Tool::Rect);
        drag(&mut view, &[(10, 10), (50, 50), (20, 30)], NONE);
        assert_eq!(black_count(&view), 2 * 11 + 2 * 19);
        assert!(view.canvas().get(10, 10) && view.canvas().get(20, 30));

        // Filled, without a border: just the pattern.
        let mut view = view_with(Tool::FilledRect);
        view.set_line_width(1, false);
        view.set_pattern(PATTERNS[0]);
        drag(&mut view, &[(10, 10), (19, 19)], NONE);
        assert_eq!(black_count(&view), 100);

        // Shift makes a circle out of an oval drag.
        let mut view = view_with(Tool::Oval);
        drag(&mut view, &[(100, 100), (160, 130)], SHIFT);
        assert!(view.canvas().get(115, 100) && !view.canvas().get(150, 100));
        assert!(view.canvas().get(100, 115) && view.canvas().get(130, 115));

        // Command paints a filled shape in OR mode.
        let mut view = view_with(Tool::PaintBucket);
        click(&mut view, 5, 5);
        view.set_tool(Tool::FilledRoundRect);
        view.set_pattern(PATTERNS[19]);
        drag(
            &mut view,
            &[(10, 10), (100, 100)],
            Modifiers {
                command: true,
                ..NONE
            },
        );
        assert_eq!(black_count(&view), 576 * 720);
    }

    #[test]
    fn polygons_close_near_the_start_or_end_on_a_double_click() {
        let mut view = view_with(Tool::Polygon);
        click(&mut view, 10, 10);
        click(&mut view, 100, 10);
        assert!(view.polygon_active());
        // The next edge follows the pointer with the button up.
        view.mouse_moved(Point::new(100, 100), NONE);
        assert!(view.canvas().get(100, 50));
        click(&mut view, 100, 100);
        click(&mut view, 12, 11);
        assert!(!view.polygon_active());
        // Closed: the last edge goes back to the start.
        assert!(view.canvas().get(56, 55) && view.canvas().get(12, 11));

        // A filled polygon, finished by clicking twice in one place.
        let mut view = view_with(Tool::FilledPolygon);
        for (x, y) in [(10, 10), (100, 10), (100, 100), (100, 101)] {
            click(&mut view, x, y);
        }
        assert!(!view.polygon_active() && view.canvas().get(80, 30));
        assert!(!view.canvas().get(30, 80));

        // Any key ends one too, open.
        let mut view = view_with(Tool::Polygon);
        click(&mut view, 10, 10);
        click(&mut view, 100, 10);
        click(&mut view, 100, 100);
        assert!(view.key_down() && !view.polygon_active());
        assert!(view.canvas().get(100, 50) && !view.canvas().get(56, 55));
    }

    #[test]
    fn freeform_follows_the_pointer_and_filled_freeform_fills() {
        let mut view = view_with(Tool::Freeform);
        drag(&mut view, &[(10, 10), (60, 10), (60, 60)], NONE);
        assert!(view.canvas().get(35, 10) && view.canvas().get(60, 35));
        assert!(!view.canvas().get(35, 35));

        let mut view = view_with(Tool::FilledFreeform);
        view.set_pattern(PATTERNS[0]);
        drag(&mut view, &[(10, 10), (60, 10), (60, 60)], NONE);
        assert!(view.canvas().get(50, 20) && view.canvas().get(35, 35));
        assert!(!view.canvas().get(20, 50));
    }

    #[test]
    fn brush_and_spray_paint_through_their_masks() {
        let mut view = view_with(Tool::Brush);
        drag(&mut view, &[(100, 100), (200, 100)], NONE);
        let black = black_count(&view);
        assert!(view.canvas().get(150, 100) && black > 100 * 3);

        let mut view = view_with(Tool::Spray);
        drag(&mut view, &[(100, 100), (101, 100), (110, 100)], NONE);
        let sprayed = black_count(&view);
        // A sparse nozzle: some pixels, far fewer than a solid stamp.
        assert!(sprayed > 0 && sprayed < 2 * 256);
    }

    #[test]
    fn undo_swaps_so_undoing_again_redoes() {
        let mut view = view_with(Tool::Line);
        view.undo();
        assert_eq!(black_count(&view), 0);
        drag(&mut view, &[(10, 10), (19, 10)], NONE);
        view.undo();
        assert_eq!(black_count(&view), 0);
        view.undo();
        assert_eq!(black_count(&view), 10);
    }
}
