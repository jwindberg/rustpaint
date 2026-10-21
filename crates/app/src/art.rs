//! Drawing MacPaint's interface into 1-bit canvases: PaintFont glyphs,
//! QuickDraw-style rects, palette boxes, brush masks and cursors. A port of
//! HaikuPaint's BitmapUtils.

use rustpaint_engine::paint_font::{Glyph, paint_font_glyph};
use rustpaint_engine::{BLACK_PATTERN, Canvas, Mask16, Pattern};

pub const CURSOR_SIZE: usize = 16;

/// Draws a PaintFont glyph with the pen at (pen_x, top); see
/// `rustpaint_engine::paint_font`. Like QuickDraw's srcOr, or srcXor when
/// invert is true.
pub fn draw_glyph(canvas: &mut Canvas, glyph: &Glyph, pen_x: i32, top: i32, invert: bool) {
    let left = pen_x + glyph.offset;
    for (y, row) in glyph.rows.iter().enumerate() {
        for (x, bit) in row.bytes().enumerate() {
            if bit != b'#' {
                continue;
            }
            let (x, y) = (left + x as i32, top + y as i32);
            let black = if invert { !canvas.get(x, y) } else { true };
            canvas.set(x, y, black);
        }
    }
}

// Rectangle helpers with QuickDraw semantics: right and bottom are
// exclusive, so MacPaint's coordinates can be used as they are.

pub fn paint_qd_rect(
    canvas: &mut Canvas,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    pattern: Pattern,
) {
    if right > left && bottom > top {
        canvas.fill_rect(left, top, right - 1, bottom - 1, pattern);
    }
}

pub fn frame_qd_rect(canvas: &mut Canvas, left: i32, top: i32, right: i32, bottom: i32) {
    canvas.draw_line(left, top, right - 1, top, true);
    canvas.draw_line(left, bottom - 1, right - 1, bottom - 1, true);
    canvas.draw_line(left, top, left, bottom - 1, true);
    canvas.draw_line(right - 1, top, right - 1, bottom - 1, true);
}

pub fn invert_qd_rect(canvas: &mut Canvas, left: i32, top: i32, right: i32, bottom: i32) {
    for y in top..bottom {
        for x in left..right {
            let black = canvas.get(x, y);
            canvas.set(x, y, !black);
        }
    }
}

/// A palette's white box with a black frame and MacPaint's one pixel drop
/// shadow; the canvas must be one pixel larger than the box each way.
pub fn draw_palette_box(canvas: &mut Canvas) {
    // Shadow first, then the box over it, then its frame.
    let right = canvas.width() - 1;
    let bottom = canvas.height() - 1;
    paint_qd_rect(canvas, 1, 1, right + 1, bottom + 1, BLACK_PATTERN);
    paint_qd_rect(canvas, 0, 0, right, bottom, rustpaint_engine::WHITE_PATTERN);
    frame_qd_rect(canvas, 0, 0, right, bottom);
}

/// A glyph drawn at pen (0, 0) into a 16 x 16 canvas, the way MacPaint
/// builds its cursors. Empty if PaintFont has no such character.
fn glyph_canvas(code: i32) -> Canvas {
    let mut canvas = Canvas::new(CURSOR_SIZE as i32, CURSOR_SIZE as i32);
    if let Some(glyph) = paint_font_glyph(code) {
        draw_glyph(&mut canvas, glyph, 0, 0, false);
    }
    canvas
}

/// A PaintFont glyph as a 16 x 16 mask; brushes and the spray can paint
/// through these.
pub fn mask_from_glyph(code: i32) -> Mask16 {
    let canvas = glyph_canvas(code);
    let mut mask = Mask16::default();
    for y in 0..CURSOR_SIZE {
        for x in 0..CURSOR_SIZE {
            if canvas.get(x as i32, y as i32) {
                mask.rows[y] |= 0x8000 >> x;
            }
        }
    }
    mask
}

/// A cursor image built from PaintFont glyphs as MacPaint's SetToolCursor
/// does, as RGBA, scaled up by a whole number. Pixels set in data are
/// black; those set only in mask are white; the rest are transparent.
/// (Pixels in data but not in mask inverted the screen on the Mac; they
/// show black here.) A mask code PaintFont lacks means no mask.
pub fn glyph_cursor_rgba(data_code: i32, mask_code: i32, scale: usize) -> Vec<u8> {
    let data = glyph_canvas(data_code);
    let mask = glyph_canvas(mask_code);
    let side = CURSOR_SIZE * scale;
    let mut rgba = vec![0; side * side * 4];
    for y in 0..side {
        for x in 0..side {
            let (cx, cy) = ((x / scale) as i32, (y / scale) as i32);
            let pixel: [u8; 4] = if data.get(cx, cy) {
                [0, 0, 0, 255]
            } else if mask.get(cx, cy) {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba[(y * side + x) * 4..][..4].copy_from_slice(&pixel);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::paint_font::{
        DEFAULT_BRUSH, GLYPH_FIRST_BRUSH, GLYPH_SPRAY, PAINT_FONT_HEIGHT,
    };

    #[test]
    fn glyph_height_fits_a_cursor() {
        assert_eq!(PAINT_FONT_HEIGHT, CURSOR_SIZE);
    }

    #[test]
    fn masks_and_cursors_come_from_glyphs() {
        let brush = mask_from_glyph(GLYPH_FIRST_BRUSH + DEFAULT_BRUSH);
        assert!(brush.rows.iter().any(|&row| row != 0));
        assert_eq!(mask_from_glyph(0), Mask16::default());

        let spray = glyph_cursor_rgba(GLYPH_SPRAY, 0, 2);
        assert_eq!(spray.len(), 32 * 32 * 4);
        // Black where the glyph is, transparent elsewhere, never white
        // without a mask.
        assert!(spray.chunks(4).any(|p| p == [0, 0, 0, 255]));
        assert!(spray.chunks(4).all(|p| p != [255, 255, 255, 255]));
    }

    #[test]
    fn palette_box_has_a_shadow() {
        let mut canvas = Canvas::new(10, 8);
        draw_palette_box(&mut canvas);
        // Frame, white inside, shadow along the right and bottom, and the
        // two corners the shadow doesn't reach left white.
        assert!(canvas.get(0, 0) && !canvas.get(4, 4) && canvas.get(9, 7));
        assert!(canvas.get(9, 1) && !canvas.get(9, 0) && !canvas.get(0, 7));
    }
}
