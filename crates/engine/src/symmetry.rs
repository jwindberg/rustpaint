//! MacPaint's Brush Mirrors. Four mirror lines through a center point:
//! vertical, horizontal and the two diagonals. MapSym turns the chosen ones
//! into a byte saying which of the eight reflections of a point to paint;
//! combinations it has no entry for paint all eight.

use crate::geometry::Point;

/// Just the point itself.
pub const NO_SYMMETRY: u8 = 0x80;

pub fn map_symmetry(vertical: bool, horizontal: bool, diagonal: bool, anti_diagonal: bool) -> u8 {
    // MapSym's table, indexed by vh * 8 + hv * 4 + v * 2 + h. (MacPaint's
    // hSym mirrors across the vertical line, vSym across the horizontal.)
    const TABLE: [u8; 16] = [
        0x80, 0xC0, 0xA0, 0xF0, //
        0x88, 0xFF, 0xFF, 0xFF, //
        0x81, 0xFF, 0xFF, 0xFF, //
        0x99, 0xFF, 0xFF, 0xFF,
    ];
    let index = (anti_diagonal as usize) << 3
        | (diagonal as usize) << 2
        | (horizontal as usize) << 1
        | vertical as usize;
    TABLE[index]
}

/// The point and its reflections around center that symmetry selects, in
/// MacPaint's SymBrush order, starting with the point itself.
pub fn symmetric_points(point: Point, center: Point, symmetry: u8) -> Vec<Point> {
    let h = point.x - center.x;
    let v = point.y - center.y;

    // One reflection per bit, from bit 7 (the point itself) down.
    let reflections = [
        (h, v),
        (-h, v),
        (h, -v),
        (-h, -v),
        (v, h),
        (-v, h),
        (v, -h),
        (-v, -h),
    ];

    let mut points = vec![point];
    for (bit, (dx, dy)) in reflections.into_iter().enumerate().skip(1) {
        if symmetry & (0x80 >> bit) != 0 {
            points.push(Point::new(center.x + dx, center.y + dy));
        }
    }
    points
}
