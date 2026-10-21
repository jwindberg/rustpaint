//! Edit Pattern (DLOG 2003): an 8 x 8 grid of fat bits and a sample. Click
//! or drag in the grid to set and clear bits.

use rustpaint_engine::{BLACK_PATTERN, Canvas, Pattern, Point, Rect};

use super::{Button, Outcome, frame};
use crate::art::paint_qd_rect;

pub const SIZE: (i32, i32) = (176, 128);

pub fn buttons() -> Vec<Button> {
    vec![
        Button::new(16, 92, 80, "OK", true),
        Button::new(96, 92, 160, "Cancel", false),
    ]
}

pub struct EditPattern {
    pattern: Pattern,
    tracking: bool,
    paint_white: bool,
}

impl EditPattern {
    pub fn new(pattern: Pattern) -> Self {
        Self {
            pattern,
            tracking: false,
            paint_white: false,
        }
    }

    pub fn button(&self, index: usize) -> Outcome {
        if index == 0 {
            Outcome::Pattern(self.pattern)
        } else {
            Outcome::Cancelled
        }
    }

    /// EditPat: the fat bits in (16, 16)-(80, 80), each 7 x 7 on an 8
    /// pixel grid. While dragging, points outside go to the nearest bit.
    fn bit_at(&self, point: Point) -> Option<Point> {
        let inside = (16..80).contains(&point.x) && (16..80).contains(&point.y);
        if !self.tracking && !inside {
            return None;
        }
        Some(Point::new(
            ((point.x - 16) / 8).clamp(0, 7),
            ((point.y - 16) / 8).clamp(0, 7),
        ))
    }

    fn set_bit(&mut self, bit: Point) {
        let mask = 0x80u8 >> bit.x;
        let row = &mut self.pattern.rows[bit.y as usize];
        if self.paint_white {
            *row &= !mask;
        } else {
            *row |= mask;
        }
    }

    pub fn mouse_down(&mut self, point: Point) {
        let Some(bit) = self.bit_at(point) else {
            return;
        };
        // Like MacPaint: start on a black bit to paint white.
        self.paint_white = self.pattern.bit(bit.x, bit.y);
        self.tracking = true;
        self.set_bit(bit);
    }

    pub fn mouse_moved(&mut self, point: Point) {
        if self.tracking
            && let Some(bit) = self.bit_at(point)
        {
            self.set_bit(bit);
        }
    }

    pub fn mouse_up(&mut self) {
        self.tracking = false;
    }

    pub fn render(&self, canvas: &mut Canvas) {
        frame(canvas, Rect::new(15, 15, 81, 81));
        for y in 0..8 {
            for x in 0..8 {
                if self.pattern.bit(x, y) {
                    let (left, top) = (16 + x * 8, 16 + y * 8);
                    paint_qd_rect(canvas, left, top, left + 7, top + 7, BLACK_PATTERN);
                }
            }
        }
        frame(canvas, Rect::new(95, 15, 161, 81));
        paint_qd_rect(canvas, 96, 16, 160, 80, self.pattern);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::WHITE_PATTERN;

    #[test]
    fn clicking_and_dragging_sets_and_clears_bits() {
        let mut editor = EditPattern::new(WHITE_PATTERN);
        // Drag along the top row: every bit black.
        editor.mouse_down(Point::new(18, 18));
        editor.mouse_moved(Point::new(78, 18));
        editor.mouse_moved(Point::new(200, 18));
        editor.mouse_up();
        // Bits 0 and 7 at least, and the ones the drag reached.
        assert!(editor.pattern.bit(0, 0) && editor.pattern.bit(7, 0));

        // Starting on a black bit clears.
        editor.mouse_down(Point::new(18, 18));
        editor.mouse_up();
        assert!(!editor.pattern.bit(0, 0) && editor.pattern.bit(7, 0));

        // Outside the grid, a press does nothing.
        editor.mouse_down(Point::new(120, 40));
        assert!(!editor.tracking);
    }
}
