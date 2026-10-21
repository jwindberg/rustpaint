//! Brush Mirrors (DLOG 2005): four mirror lines through a square; click
//! one to toggle it. None turns them all off.

use rustpaint_engine::geometry::constrain;
use rustpaint_engine::{BLACK_PATTERN, Canvas, Point, Rect};

use super::{Button, Outcome, frame};

// Brush Mirrors' four lines, as MacPaint's hSym, vSym, hvSym and vhSym.
pub const MIRROR_VERTICAL: u8 = 1;
pub const MIRROR_HORIZONTAL: u8 = 2;
/// Top left to bottom right.
pub const MIRROR_DIAGONAL: u8 = 4;
pub const MIRROR_ANTI_DIAGONAL: u8 = 8;

pub const SIZE: (i32, i32) = (176, 176);
/// Symmetry: a 112 pixel square at (32, 16).
const LEFT: i32 = 32;
const TOP: i32 = 16;
const SQUARE: i32 = 112;

pub fn buttons() -> Vec<Button> {
    vec![
        Button::new(16, 140, 80, "OK", true),
        Button::new(96, 140, 160, "None", false),
    ]
}

pub struct Mirrors {
    mirrors: u8,
}

impl Mirrors {
    pub fn new(mirrors: u8) -> Self {
        Self { mirrors }
    }

    pub fn button(&self, index: usize) -> Outcome {
        // MacPaint's None turns every mirror off.
        Outcome::Mirrors(if index == 0 { self.mirrors } else { 0 })
    }

    pub fn mouse_down(&mut self, click: Point) {
        if !(LEFT..LEFT + SQUARE).contains(&click.x) || !(TOP..TOP + SQUARE).contains(&click.y) {
            return;
        }
        // Snap the click to the nearest of the four lines.
        let middle = Point::new(LEFT + SQUARE / 2, TOP + SQUARE / 2);
        let snapped = constrain(middle, click, true);
        self.mirrors ^= if snapped.x == middle.x {
            MIRROR_VERTICAL
        } else if snapped.y == middle.y {
            MIRROR_HORIZONTAL
        } else if snapped.x - middle.x == snapped.y - middle.y {
            MIRROR_DIAGONAL
        } else {
            MIRROR_ANTI_DIAGONAL
        };
    }

    pub fn render(&self, canvas: &mut Canvas) {
        let mut lines = Canvas::new(canvas.width(), canvas.height());
        let (middle_x, middle_y) = (LEFT + SQUARE / 2, TOP + SQUARE / 2);
        let (right, bottom) = (LEFT + SQUARE, TOP + SQUARE);
        let mut line = |mirror: u8, from: (i32, i32), to: (i32, i32)| {
            // A mirror that's on is 3 pixels thick (PenSize(3, 3), moved
            // up and left by one).
            if self.mirrors & mirror != 0 {
                lines.stroke_line(from.0 - 1, from.1 - 1, to.0 - 1, to.1 - 1, 3, BLACK_PATTERN);
            } else {
                lines.draw_line(from.0, from.1, to.0, to.1, true);
            }
        };
        line(MIRROR_VERTICAL, (middle_x, TOP), (middle_x, bottom));
        line(MIRROR_HORIZONTAL, (LEFT, middle_y), (right, middle_y));
        line(MIRROR_DIAGONAL, (LEFT, TOP), (right, bottom));
        line(MIRROR_ANTI_DIAGONAL, (LEFT, bottom), (right, TOP));

        // The lines are clipped to inside the frame.
        frame(canvas, Rect::new(LEFT, TOP, right, bottom));
        canvas.copy_rect(&lines, Rect::new(LEFT + 1, TOP + 1, right - 1, bottom - 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_toggle_the_nearest_line() {
        let mut mirrors = Mirrors::new(0);
        mirrors.mouse_down(Point::new(88, 30));
        assert_eq!(mirrors.mirrors, MIRROR_VERTICAL);
        mirrors.mouse_down(Point::new(40, 73));
        assert_eq!(mirrors.mirrors, MIRROR_VERTICAL | MIRROR_HORIZONTAL);
        mirrors.mouse_down(Point::new(50, 34));
        mirrors.mouse_down(Point::new(50, 110));
        assert_eq!(mirrors.mirrors, 15);
        mirrors.mouse_down(Point::new(88, 30));
        assert_eq!(mirrors.mirrors, 14);
        // Outside the square: nothing.
        mirrors.mouse_down(Point::new(5, 5));
        assert_eq!(mirrors.mirrors, 14);

        assert_eq!(mirrors.button(0), Outcome::Mirrors(14));
        assert_eq!(mirrors.button(1), Outcome::Mirrors(0));
    }
}
