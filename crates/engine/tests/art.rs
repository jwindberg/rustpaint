//! Sanity checks on the artwork the extractor generates.

use rustpaint_engine::mac_icons::{
    ABOUT_ICON, APP_ICON, APP_ICON_MASK, DOCUMENT_ICON, DOCUMENT_ICON_MASK,
};
use rustpaint_engine::paint_font::{
    BRUSH_COUNT, GLYPH_CHECK, GLYPH_FIRST_BRUSH, GLYPH_FIRST_TOOL, GLYPH_SPRAY, paint_font_glyph,
};

#[test]
fn paint_font_has_every_glyph_macpaint_uses() {
    let mut codes = vec![GLYPH_CHECK, GLYPH_SPRAY];
    codes.extend((0..20).map(|tool| GLYPH_FIRST_TOOL + tool));
    codes.extend((0..BRUSH_COUNT).map(|brush| GLYPH_FIRST_BRUSH + brush));
    for code in codes {
        let glyph = paint_font_glyph(code).unwrap_or_else(|| panic!("no glyph {code}"));
        assert!(
            glyph
                .rows
                .iter()
                .all(|row| row.len() == glyph.width as usize)
        );
        assert!(
            glyph.rows.iter().any(|row| row.contains('#')),
            "glyph {code} is blank"
        );
    }
    assert!(paint_font_glyph(0).is_none());
}

#[test]
fn icons_are_not_blank() {
    for icon in [
        &APP_ICON,
        &APP_ICON_MASK,
        &DOCUMENT_ICON,
        &DOCUMENT_ICON_MASK,
        &ABOUT_ICON,
    ] {
        assert!(icon.iter().any(|&byte| byte != 0));
    }
}
