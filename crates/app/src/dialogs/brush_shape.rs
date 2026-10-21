//! Brush Shape (MacPaint's ChooseBrush, DLOG 2004): the 32 brushes from
//! PaintFont, the current one framed. A click chooses one and closes.

use rustpaint_engine::paint_font::{GLYPH_FIRST_BRUSH, paint_font_glyph};
use rustpaint_engine::{Canvas, Point};

use super::Outcome;
use crate::art::{draw_glyph, invert_qd_rect};

pub const SIZE: (i32, i32) = (288, 160);
const LEFT: i32 = 16;
const TOP: i32 = 16;
const SPACE: i32 = 32;
const ROWS: i32 = 4;
const COLUMNS: i32 = 8;

pub struct BrushShape {
    brush: i32,
}

impl BrushShape {
    pub fn new(brush: i32) -> Self {
        Self { brush }
    }

    /// The brush at the click, or the nearest one.
    pub fn mouse_down(&mut self, point: Point) -> Outcome {
        let row = ((point.y - TOP) / SPACE).clamp(0, ROWS - 1);
        let column = ((point.x - LEFT) / SPACE).clamp(0, COLUMNS - 1);
        Outcome::Brush(row + column * ROWS)
    }

    pub fn render(&self, canvas: &mut Canvas) {
        for column in 0..COLUMNS {
            for row in 0..ROWS {
                if let Some(glyph) = paint_font_glyph(GLYPH_FIRST_BRUSH + row + column * ROWS) {
                    let x = LEFT + SPACE / 2 - 8 + column * SPACE;
                    let y = TOP + SPACE / 2 - 8 + row * SPACE;
                    draw_glyph(canvas, glyph, x, y, false);
                }
            }
        }

        // InvBrush: a 2 pixel XOR frame around the current brush's cell.
        let left = LEFT + (self.brush / ROWS) * SPACE;
        let top = TOP + (self.brush % ROWS) * SPACE;
        let (right, bottom) = (left + SPACE, top + SPACE);
        invert_qd_rect(canvas, left, top, right, top + 2);
        invert_qd_rect(canvas, left, bottom - 2, right, bottom);
        invert_qd_rect(canvas, left, top + 2, left + 2, bottom - 2);
        invert_qd_rect(canvas, right - 2, top + 2, right, bottom - 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brushes_go_down_the_columns() {
        let mut chooser = BrushShape::new(7);
        assert_eq!(chooser.mouse_down(Point::new(20, 20)), Outcome::Brush(0));
        assert_eq!(chooser.mouse_down(Point::new(20, 60)), Outcome::Brush(1));
        assert_eq!(chooser.mouse_down(Point::new(60, 20)), Outcome::Brush(4));
        // Outside the grid: the nearest brush.
        assert_eq!(chooser.mouse_down(Point::new(500, 500)), Outcome::Brush(31));

        // The current brush is framed.
        let mut canvas = Canvas::new(SIZE.0, SIZE.1);
        chooser.render(&mut canvas);
        let (left, top) = (LEFT + SPACE, TOP + 3 * SPACE);
        assert!(canvas.get(left, top) && canvas.get(left + 1, top + 1));
    }
}
