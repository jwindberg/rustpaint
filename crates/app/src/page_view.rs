//! The page and the tools that draw on it: a port of HaikuPaint's
//! CanvasView, which follows MacPaint's EditDoc and the tool procedures it
//! calls. All pixel logic belongs in the engine; this only turns mouse
//! input, in page coordinates, into engine calls.

use rustpaint_engine::geometry::{AxisLock, constrain, snap_to_grid};
use rustpaint_engine::{BLACK_PATTERN, Canvas, PaintMode, Pattern, Point, Rect, WHITE_PATTERN};

use crate::tool::Tool;

/// MacPaint's eraser is its 16 x 16 cursor box, centered on the hotspot.
const ERASER_SIZE: i32 = 16;

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
    /// The image before the current operation: MacPaint's undo buffer, and
    /// what live previews are restored from.
    undo: Canvas,
    dirty: bool,

    tool: Tool,
    pattern: Pattern,
    line_width: i32,
    border: bool,
    grid: bool,

    tracking: bool,
    pencil_black: bool,
    start: Point,
    last: Point,
    axis_lock: AxisLock,
    preview_rect: Rect,
}

impl PageView {
    pub fn new() -> Self {
        Self {
            canvas: Canvas::default(),
            undo: Canvas::default(),
            dirty: false,
            tool: Tool::Pencil,
            pattern: BLACK_PATTERN,
            line_width: 1,
            border: true,
            grid: false,
            tracking: false,
            pencil_black: true,
            start: Point::default(),
            last: Point::default(),
            axis_lock: AxisLock::default(),
            preview_rect: Rect::default(),
        }
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
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

    /// Clears the whole page, as double-clicking the eraser does.
    pub fn clear(&mut self) {
        self.begin_operation();
        self.canvas.clear();
    }

    pub fn mouse_down(&mut self, point: Point, modifiers: Modifiers) {
        // The other tools arrive in later milestones.
        if !matches!(
            self.tool,
            Tool::PaintBucket | Tool::Pencil | Tool::Eraser | Tool::Line
        ) {
            return;
        }

        let point = self.tool_point(point);
        self.tracking = true;
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
            Tool::Pencil => {
                // Like MacPaint: starting on a black pixel draws white.
                self.pencil_black = !self.canvas.get(point.x, point.y);
                self.canvas.set(point.x, point.y, self.pencil_black);
            }
            Tool::Eraser => self.erase(point, point),
            _ => {
                self.begin_preview();
                self.drag_to(point, modifiers);
            }
        }
    }

    pub fn mouse_moved(&mut self, point: Point, modifiers: Modifiers) {
        if !self.tracking {
            return;
        }
        let mut point = self.tool_point(point);

        match self.tool {
            Tool::Pencil => {
                if modifiers.shift {
                    point = self.axis_lock.apply(point);
                }
                let last = self.last;
                self.canvas
                    .draw_line(last.x, last.y, point.x, point.y, self.pencil_black);
                self.last = point;
            }
            Tool::Eraser => {
                self.erase(self.last, point);
                self.last = point;
            }
            _ => self.drag_to(point, modifiers),
        }
    }

    pub fn mouse_up(&mut self, point: Point, modifiers: Modifiers) {
        if !self.tracking {
            return;
        }
        self.tracking = false;

        if self.tool == Tool::Line {
            let point = self.tool_point(point);
            self.drag_to(point, modifiers);
        }
        self.canvas.set_paint_mode(PaintMode::Copy);
    }

    /// Called before anything changes the canvas: saves the undo image and
    /// marks the document dirty.
    fn begin_operation(&mut self) {
        self.undo = self.canvas.clone();
        self.dirty = true;
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
        // StraightLine: Shift snaps to horizontal, vertical or 45 degrees.
        if modifiers.shift {
            point = constrain(self.start, point, true);
        }
        self.draw_line(point, modifiers);
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

    fn drag(view: &mut PageView, points: &[(i32, i32)], modifiers: Modifiers) {
        let p = |&(x, y): &(i32, i32)| Point::new(x, y);
        view.mouse_down(p(&points[0]), modifiers);
        for point in &points[1..] {
            view.mouse_moved(p(point), modifiers);
        }
        view.mouse_up(p(points.last().unwrap()), modifiers);
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
        let mut view = PageView::new();
        view.set_tool(Tool::Line);
        drag(&mut view, &[(10, 10), (100, 100), (50, 10), (40, 10)], NONE);
        assert_eq!(black_count(&view), 31);
        assert!(!view.canvas().get(50, 50) && !view.canvas().get(50, 10));

        // A thick line is centered on the pointer; Shift snaps it flat.
        let mut view = PageView::new();
        view.set_tool(Tool::Line);
        view.set_line_width(4, true);
        drag(&mut view, &[(100, 100), (200, 104)], SHIFT);
        assert!(view.canvas().get(98, 98) && view.canvas().get(201, 101));
        assert!(!view.canvas().get(98, 102) && !view.canvas().get(200, 104));
    }

    #[test]
    fn line_paints_the_pattern_with_option() {
        let mut view = PageView::new();
        view.set_tool(Tool::Line);
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
        let mut view = PageView::new();
        view.set_tool(Tool::PaintBucket);
        drag(&mut view, &[(5, 5)], NONE);
        assert_eq!(black_count(&view), 576 * 720);

        view.set_tool(Tool::Eraser);
        drag(&mut view, &[(100, 100), (100, 100)], NONE);
        assert_eq!(black_count(&view), 576 * 720 - 16 * 16);
        assert!(!view.canvas().get(92, 92) && !view.canvas().get(107, 107));

        view.clear();
        assert_eq!(black_count(&view), 0);
    }

    #[test]
    fn unimplemented_tools_do_nothing() {
        let mut view = PageView::new();
        view.set_tool(Tool::Oval);
        drag(&mut view, &[(10, 10), (50, 50)], NONE);
        assert_eq!(black_count(&view), 0);
    }
}
