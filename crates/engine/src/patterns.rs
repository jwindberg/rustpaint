//! The standard Macintosh pattern list (System PAT# 0), which MacPaint 1.x
//! shows in its pattern palette. The list is generated into
//! `generated/patterns.rs` by `tools/extract_macpaint_art.py`.

use crate::canvas::Pattern;

pub const PATTERN_COUNT: usize = 38;

/// MacPaint starts with solid black selected.
pub const DEFAULT_PATTERN_INDEX: usize = 0;
pub const WHITE_PATTERN_INDEX: usize = 19;

/// Rows top to bottom; MSB is the leftmost pixel. MacPaint shows 0-18 in
/// its top row and 19-37 in its bottom row.
pub static PATTERNS: [Pattern; PATTERN_COUNT] = include!("generated/patterns.rs");
