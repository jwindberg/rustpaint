//! MacPaint's three palettes, rendered pixel by pixel as HaikuPaint's
//! ToolPalette, LineWidthPalette and PatternPalette do. Each renders into a
//! canvas one pixel larger than its box each way, for the drop shadow, and
//! hit-tests points relative to its top left corner.

use rustpaint_engine::paint_font::{GLYPH_CHECK, GLYPH_FIRST_TOOL, paint_font_glyph};
use rustpaint_engine::patterns::PATTERN_COUNT;
use rustpaint_engine::{BLACK_PATTERN, Canvas, Pattern};

use crate::art::{draw_glyph, draw_palette_box, frame_qd_rect, invert_qd_rect, paint_qd_rect};
use crate::tool::{TOOL_COUNT, Tool};

/// MacPaint's tool palette: 20 icons from PaintFont in two columns, with
/// the selected tool inverted.
pub mod tools {
    use super::*;

    // MacPaint's toolLeft/Right/Top/Bottom/Mid and toolSpace, relative to
    // the palette's top left corner.
    pub const WIDTH: i32 = 53;
    pub const HEIGHT: i32 = 221;
    const MID: i32 = 26;
    const ROW_SPACE: i32 = 22;
    const ROWS: i32 = TOOL_COUNT as i32 / 2;

    /// Follows MacPaint's DrawTool and InvTool.
    pub fn render(selected: Tool) -> Canvas {
        let mut canvas = Canvas::new(WIDTH + 1, HEIGHT + 1);
        draw_palette_box(&mut canvas);
        canvas.draw_line(MID, 0, MID, HEIGHT, true);

        let mut v = 0;
        let mut code = GLYPH_FIRST_TOOL;
        for row in 0..ROWS {
            for column in 0..2 {
                if let Some(glyph) = paint_font_glyph(code) {
                    let cell_left = if column == 0 { 0 } else { MID };
                    draw_glyph(
                        &mut canvas,
                        glyph,
                        cell_left + 14 - glyph.advance / 2,
                        v + 4,
                        false,
                    );
                }
                code += 1;
            }
            v += ROW_SPACE;
            if row != ROWS - 1 {
                canvas.draw_line(0, v, WIDTH - 1, v, true);
            }
        }

        let selected = selected as i32;
        let top = 1 + (selected / 2) * ROW_SPACE;
        let bottom = top + ROW_SPACE - 1;
        if selected % 2 == 1 {
            invert_qd_rect(&mut canvas, MID + 1, top, WIDTH - 1, bottom);
        } else {
            invert_qd_rect(&mut canvas, 1, top, MID, bottom);
        }
        canvas
    }

    pub fn hit(x: i32, y: i32) -> Option<Tool> {
        if !(1..WIDTH - 1).contains(&x) || !(1..HEIGHT - 1).contains(&y) {
            return None;
        }
        let row = (y - 1) / ROW_SPACE;
        Tool::from_index((2 * row + (x > MID) as i32) as usize)
    }
}

/// MacPaint's line palette: "no border" (a gray sample, for the shape
/// tools) and sizes 1, 2, 4 and 8, with a check mark at the current choice.
pub mod line_widths {
    use super::*;

    // MacPaint's lineLeft/Right/Top/Bottom, relative to the palette.
    pub const WIDTH: i32 = 53;
    pub const HEIGHT: i32 = 77;

    /// QuickDraw's ltGray, used for the "no border" sample.
    const LIGHT_GRAY: Pattern = Pattern::new([0x88, 0x22, 0x88, 0x22, 0x88, 0x22, 0x88, 0x22]);

    /// One row per choice, from MacPaint's DrawLine, ChooseLine and
    /// InvLnSize.
    struct LineChoice {
        size: i32,
        border: bool,
        sample_top: i32,
        sample_height: i32,
        check_top: i32,
    }

    const CHOICES: [LineChoice; 5] = [
        // No border.
        LineChoice {
            size: 1,
            border: false,
            sample_top: 12,
            sample_height: 1,
            check_top: 5,
        },
        LineChoice {
            size: 1,
            border: true,
            sample_top: 23,
            sample_height: 1,
            check_top: 15,
        },
        LineChoice {
            size: 2,
            border: true,
            sample_top: 33,
            sample_height: 2,
            check_top: 26,
        },
        LineChoice {
            size: 4,
            border: true,
            sample_top: 44,
            sample_height: 4,
            check_top: 38,
        },
        LineChoice {
            size: 8,
            border: true,
            sample_top: 57,
            sample_height: 8,
            check_top: 52,
        },
    ];

    pub fn render(width: i32, border: bool) -> Canvas {
        let mut canvas = Canvas::new(WIDTH + 1, HEIGHT + 1);
        draw_palette_box(&mut canvas);

        let check = paint_font_glyph(GLYPH_CHECK);
        for choice in &CHOICES {
            let pattern = if choice.border {
                BLACK_PATTERN
            } else {
                LIGHT_GRAY
            };
            let bottom = choice.sample_top + choice.sample_height;
            paint_qd_rect(
                &mut canvas,
                17,
                choice.sample_top,
                WIDTH - 7,
                bottom,
                pattern,
            );

            let selected = choice.border == border && (!border || choice.size == width);
            if let (true, Some(check)) = (selected, check) {
                draw_glyph(&mut canvas, check, 2, choice.check_top, true);
            }
        }
        canvas
    }

    /// MacPaint's ChooseLine: the size and whether shapes get a border.
    pub fn hit(x: i32, y: i32) -> Option<(i32, bool)> {
        if !(0..WIDTH).contains(&x) || !(0..HEIGHT).contains(&y) {
            return None;
        }
        let size = match y {
            ..=28 => 1,
            29..=39 => 2,
            40..=52 => 4,
            _ => 8,
        };
        Some((size, y > 18))
    }
}

/// MacPaint's pattern palette: a sample of the current pattern, then the
/// document's 38 patterns in two rows of 19.
pub mod patterns {
    use super::*;

    // MacPaint's patLeft/Right/Top/Bottom/Mid, patSpace, patRow and
    // patStart, relative to the palette.
    pub const WIDTH: i32 = 423;
    pub const HEIGHT: i32 = 35;
    const MID: i32 = 17;
    const SPACE: i32 = 20;
    const PER_ROW: usize = 19;
    const START: i32 = 43;

    const _: () = assert!(2 * PER_ROW == PATTERN_COUNT, "two rows of patterns");

    /// Follows MacPaint's DrawPat.
    pub fn render(patterns: &[Pattern; PATTERN_COUNT], selected: usize) -> Canvas {
        let mut canvas = Canvas::new(WIDTH + 1, HEIGHT + 1);
        draw_palette_box(&mut canvas);

        frame_qd_rect(&mut canvas, 8, 6, START - 7, HEIGHT - 6);
        paint_qd_rect(&mut canvas, 9, 7, START - 8, HEIGHT - 7, patterns[selected]);

        paint_qd_rect(
            &mut canvas,
            START - 1,
            1,
            WIDTH - 1,
            HEIGHT - 1,
            BLACK_PATTERN,
        );
        let mut h = START;
        for i in 0..PER_ROW {
            paint_qd_rect(&mut canvas, h, 1, h + SPACE - 1, MID, patterns[i]);
            paint_qd_rect(
                &mut canvas,
                h,
                MID + 1,
                h + SPACE - 1,
                HEIGHT - 1,
                patterns[i + PER_ROW],
            );
            h += SPACE;
        }
        canvas
    }

    /// The index of the pattern at (x, y).
    pub fn hit(x: i32, y: i32) -> Option<usize> {
        if !(START..START + PER_ROW as i32 * SPACE).contains(&x) || !(1..HEIGHT - 1).contains(&y) {
            return None;
        }
        Some(((x - START) / SPACE) as usize + if y > MID { PER_ROW } else { 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::patterns::PATTERNS;

    #[test]
    fn tool_palette_hits_two_per_row() {
        assert_eq!(tools::hit(5, 5), Some(Tool::Lasso));
        assert_eq!(tools::hit(40, 5), Some(Tool::Select));
        assert_eq!(tools::hit(40, 1 + 4 * 22 + 3), Some(Tool::Eraser));
        assert_eq!(tools::hit(40, 1 + 3 * 22 + 3), Some(Tool::Pencil));
        assert_eq!(tools::hit(40, 219), Some(Tool::FilledPolygon));
        assert_eq!(tools::hit(0, 5), None);
        assert_eq!(tools::hit(5, 221), None);
    }

    #[test]
    fn tool_palette_inverts_the_selection() {
        let pencil = tools::render(Tool::Pencil);
        let line = tools::render(Tool::Line);
        assert_eq!((pencil.width(), pencil.height()), (54, 222));
        // The pencil's cell (row 3, on the right) is mostly black when
        // selected, white when not.
        let black_in =
            |canvas: &Canvas| (28..52).filter(|&x| canvas.get(x, 1 + 3 * 22 + 1)).count();
        assert!(black_in(&pencil) > 20 && black_in(&line) < 3);
    }

    #[test]
    fn line_palette_follows_choose_line() {
        assert_eq!(line_widths::hit(20, 10), Some((1, false)));
        assert_eq!(line_widths::hit(20, 20), Some((1, true)));
        assert_eq!(line_widths::hit(20, 30), Some((2, true)));
        assert_eq!(line_widths::hit(20, 45), Some((4, true)));
        assert_eq!(line_widths::hit(20, 60), Some((8, true)));
        assert_eq!(line_widths::hit(20, 80), None);

        // The check mark moves with the choice.
        let one = line_widths::render(1, true);
        let eight = line_widths::render(8, true);
        let check_at =
            |canvas: &Canvas, top| (top..top + 10).any(|y| (2..14).any(|x| canvas.get(x, y)));
        assert!(check_at(&one, 15) && !check_at(&one, 52));
        assert!(check_at(&eight, 52) && !check_at(&eight, 15));
    }

    #[test]
    fn pattern_palette_hits_two_rows() {
        assert_eq!(patterns::hit(43, 5), Some(0));
        assert_eq!(patterns::hit(43 + 20 * 18 + 19, 5), Some(18));
        assert_eq!(patterns::hit(43, 20), Some(19));
        assert_eq!(patterns::hit(43 + 20 * 18, 30), Some(37));
        assert_eq!(patterns::hit(20, 20), None);

        // The sample shows the selected pattern: black, then white.
        let black = patterns::render(&PATTERNS, 0);
        let white = patterns::render(&PATTERNS, 19);
        assert!(black.get(20, 17) && !white.get(20, 17));
    }
}
