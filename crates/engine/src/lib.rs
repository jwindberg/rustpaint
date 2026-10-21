//! RustPaint's drawing engine: a port of HaikuPaint's `src/engine`, plus
//! MacPaint's artwork as extracted from the original disk. It has no GUI
//! dependencies, so it builds and is tested on any machine.

pub mod canvas;
pub mod dither;
pub mod geometry;
pub mod mac_icons;
pub mod macpaint_file;
pub mod paint_font;
pub mod patterns;
pub mod pdf;
pub mod selection;
pub mod symmetry;

pub use canvas::{BLACK_PATTERN, Canvas, Mask16, PaintMode, Pattern, Shape, WHITE_PATTERN};
pub use geometry::{Point, Rect};
pub use selection::Selection;
