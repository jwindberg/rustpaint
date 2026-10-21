//! MacPaint drew its tool icons, cursors, check mark and brush shapes as
//! characters of a private bitmap font, PaintFont. Its glyphs are generated
//! into `generated/paint_font.rs` by `tools/extract_macpaint_art.py`.
//!
//! Glyphs hang below the pen position (the font's ascent is 0), so a glyph
//! drawn with the pen at (x, y) covers x + offset .. x + offset + width - 1
//! and y .. y + PAINT_FONT_HEIGHT - 1.

pub const PAINT_FONT_HEIGHT: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub code: i32,
    /// Length of each row string.
    pub width: i32,
    /// X offset from the pen position.
    pub offset: i32,
    /// Pen advance (QuickDraw's CharWidth).
    pub advance: i32,
    /// `PAINT_FONT_HEIGHT` rows, '#' is black.
    pub rows: [&'static str; PAINT_FONT_HEIGHT],
}

// Character codes, as MacPaint's DrawTool, SetToolCursor and InvLnSize use
// them.
pub const GLYPH_CHECK: i32 = 65;
/// 20 tool icons, two per palette row.
pub const GLYPH_FIRST_TOOL: i32 = 69;
/// Line tool crosshairs for sizes 1..8.
pub const GLYPH_LINE_CURSOR_1: i32 = 89;
pub const GLYPH_LINE_CURSOR_2: i32 = 90;
pub const GLYPH_LINE_CURSOR_4: i32 = 91;
pub const GLYPH_LINE_CURSOR_8: i32 = 92;
pub const GLYPH_FILL_MASK: i32 = 93;
/// The spray can cursor, also its nozzle mask.
pub const GLYPH_SPRAY: i32 = 111;
pub const GLYPH_PENCIL_MASK: i32 = 116;
pub const GLYPH_ERASER_BOX: i32 = 119;
pub const GLYPH_ALL_ONES: i32 = 120;
/// 32 brush shapes, `GLYPH_FIRST_BRUSH` + the brush number.
pub const GLYPH_FIRST_BRUSH: i32 = 120;

pub const BRUSH_COUNT: i32 = 32;
/// The small round brush.
pub const DEFAULT_BRUSH: i32 = 7;

/// None if PaintFont has no such character.
pub fn paint_font_glyph(code: i32) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|glyph| glyph.code == code)
}

include!("generated/paint_font.rs");
