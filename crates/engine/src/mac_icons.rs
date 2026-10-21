//! MacPaint's own icons, generated into `generated/mac_icons.rs` by
//! `tools/extract_macpaint_art.py`: 32 x 32, one bit deep, 4 bytes per row,
//! MSB leftmost (QuickDraw's ICON and ICN# formats).

pub const MAC_ICON_SIZE: usize = 32;
pub const MAC_ICON_BYTES: usize = MAC_ICON_SIZE * MAC_ICON_SIZE / 8;

include!("generated/mac_icons.rs");
