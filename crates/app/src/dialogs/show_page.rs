//! Show Page: the whole page at a third of its size, with a gray frame for
//! the part in the window. Drag the frame to choose what to look at; drag
//! anywhere else to move the whole drawing on the page.

use rustpaint_engine::canvas::reduced;
use rustpaint_engine::geometry::AxisLock;
use rustpaint_engine::selection::map_rect;
use rustpaint_engine::{Canvas, Pattern, Point, Rect};

use super::{Button, Outcome, frame};

const SCALE: i32 = 3;
const MARGIN: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Window,
    Page,
}

pub struct ShowPage {
    small: Canvas,
    page_size: (i32, i32),
    visible: Rect,
    /// The window's part of the small page, and where it started.
    window: Rect,
    old_window: Rect,
    /// How far the drawing has been dragged, in small page pixels.
    shift: Point,
    drag: Drag,
    last: Point,
    lock: AxisLock,
}

impl ShowPage {
    pub fn new(page: &Canvas, visible: Rect) -> Self {
        let (width, height) = (page.width(), page.height());
        let window = map_rect(
            visible,
            Rect::new(0, 0, width, height),
            Rect::new(0, 0, width / SCALE, height / SCALE),
        );
        Self {
            // Three or more black pixels of each 3 x 3 make a black one,
            // which keeps light grays visible.
            small: reduced(page, SCALE, 3),
            page_size: (width, height),
            visible,
            window,
            old_window: window,
            shift: Point::default(),
            drag: Drag::None,
            last: Point::default(),
            lock: AxisLock::default(),
        }
    }

    pub fn size(&self) -> (i32, i32) {
        (
            self.small.width() + 2 * MARGIN + 2,
            self.small.height() + 2 * MARGIN + 51,
        )
    }

    pub fn buttons(&self) -> Vec<Button> {
        let (width, _) = self.size();
        let top = self.small.height() + 2 * MARGIN + 4;
        let middle = (width - 1) / 2;
        vec![
            Button::new(MARGIN, top, middle - 8, "OK", true),
            Button::new(middle + 8, top, width - 1 - MARGIN, "Cancel", false),
        ]
    }

    pub fn button(&self, index: usize) -> Outcome {
        if index != 0 {
            return Outcome::Cancelled;
        }
        let mut origin = Point::new(
            self.visible.left + SCALE * (self.window.left - self.old_window.left),
            self.visible.top + SCALE * (self.window.top - self.old_window.top),
        );

        // ShowPage pins the window to the edges when it's within 3 pixels
        // of them.
        let max_x = (self.page_size.0 - self.visible.width()).max(0);
        let max_y = (self.page_size.1 - self.visible.height()).max(0);
        if origin.x < 3 {
            origin.x = 0;
        }
        if origin.y < 3 {
            origin.y = 0;
        }
        if origin.x > max_x - 3 {
            origin.x = max_x;
        }
        if origin.y > max_y - 3 {
            origin.y = max_y;
        }

        // Shifts go in whole 8s, to keep patterns aligned.
        let shift = Point::new(
            (SCALE * self.shift.x) / 8 * 8,
            (SCALE * self.shift.y) / 8 * 8,
        );
        Outcome::ShowPage { shift, origin }
    }

    fn local(point: Point) -> Point {
        Point::new(point.x - MARGIN - 1, point.y - MARGIN - 1)
    }

    pub fn mouse_down(&mut self, point: Point) {
        let point = Self::local(point);
        let w = &self.window;
        self.drag =
            if point.x >= w.left && point.x < w.right && point.y >= w.top && point.y < w.bottom {
                Drag::Window
            } else if self.small.contains(point.x, point.y) {
                Drag::Page
            } else {
                return;
            };
        self.last = point;
        self.lock = AxisLock::new(point);
    }

    pub fn mouse_moved(&mut self, point: Point, shift: bool) {
        if self.drag == Drag::None {
            return;
        }
        let mut point = Self::local(point);
        if shift {
            point = self.lock.apply(point);
        }
        let (dx, dy) = (point.x - self.last.x, point.y - self.last.y);
        self.last = point;
        if self.drag == Drag::Window {
            // Pinned inside the small page.
            let (width, height) = (self.window.width(), self.window.height());
            let left = (self.window.left + dx).clamp(0, (self.small.width() - width).max(0));
            let top = (self.window.top + dy).clamp(0, (self.small.height() - height).max(0));
            self.window = Rect::new(left, top, left + width, top + height);
        } else {
            self.shift.x += dx;
            self.shift.y += dy;
        }
    }

    pub fn mouse_up(&mut self) {
        self.drag = Drag::None;
    }

    pub fn render(&self, canvas: &mut Canvas) {
        let (left, top) = (MARGIN + 1, MARGIN + 1);
        let (width, height) = (self.small.width(), self.small.height());
        frame(
            canvas,
            Rect::new(MARGIN, MARGIN, left + width + 1, top + height + 1),
        );

        // The page, moved by the shift so far, clipped to its box.
        for y in 0..height {
            for x in 0..width {
                canvas.set(
                    left + x,
                    top + y,
                    self.small.get(x - self.shift.x, y - self.shift.y),
                );
            }
        }

        // The window's part, framed in gray drawn in XOR.
        let gray = Pattern::new([0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]);
        let w = Rect::new(
            left + self.window.left,
            top + self.window.top,
            left + self.window.right,
            top + self.window.bottom,
        );
        let mut flip = |x: i32, y: i32| {
            if gray.bit(x, y) {
                let black = canvas.get(x, y);
                canvas.set(x, y, !black);
            }
        };
        for x in w.left..w.right {
            flip(x, w.top);
            flip(x, w.bottom - 1);
        }
        for y in w.top + 1..w.bottom - 1 {
            flip(w.left, y);
            flip(w.right - 1, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::BLACK_PATTERN;

    fn page() -> Canvas {
        let mut page = Canvas::default();
        page.fill_rect(0, 0, 29, 29, BLACK_PATTERN);
        page
    }

    #[test]
    fn dragging_the_frame_scrolls_and_pins_to_the_edges() {
        let visible = Rect::new(0, 0, 300, 300);
        let mut dialog = ShowPage::new(&page(), visible);
        assert_eq!(dialog.window, Rect::new(0, 0, 100, 100));
        assert_eq!(dialog.size(), (192 + 34, 240 + 83));

        // Drag the frame 20 small pixels right and down: 60 page pixels.
        dialog.mouse_down(Point::new(MARGIN + 1 + 50, MARGIN + 1 + 50));
        dialog.mouse_moved(Point::new(MARGIN + 1 + 70, MARGIN + 1 + 70), false);
        dialog.mouse_up();
        assert_eq!(
            dialog.button(0),
            Outcome::ShowPage {
                shift: Point::new(0, 0),
                origin: Point::new(60, 60)
            }
        );

        // All the way: pinned inside, and to the page's far edges.
        dialog.mouse_down(Point::new(MARGIN + 1 + 70, MARGIN + 1 + 70));
        dialog.mouse_moved(Point::new(MARGIN + 1 + 500, MARGIN + 1 + 500), false);
        assert_eq!(
            dialog.button(0),
            Outcome::ShowPage {
                shift: Point::new(0, 0),
                origin: Point::new(276, 420)
            }
        );
    }

    #[test]
    fn dragging_the_page_shifts_the_drawing_in_eights() {
        let mut dialog = ShowPage::new(&page(), Rect::new(0, 0, 100, 100));
        dialog.mouse_down(Point::new(MARGIN + 1 + 150, MARGIN + 1 + 150));
        dialog.mouse_moved(Point::new(MARGIN + 1 + 160, MARGIN + 1 + 153), false);
        // 10 and 3 small pixels: 30 and 9 page pixels, to 24 and 8.
        let Outcome::ShowPage { shift, .. } = dialog.button(0) else {
            panic!()
        };
        assert_eq!(shift, Point::new(24, 8));

        let mut canvas = Canvas::new(dialog.size().0, dialog.size().1);
        dialog.render(&mut canvas);
        // The drawing moved in the small view.
        assert!(canvas.get(MARGIN + 1 + 11, MARGIN + 1 + 5));
    }
}
