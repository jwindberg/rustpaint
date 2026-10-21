//! Points, rects, and the grid and Shift constraints MacPaint applies to
//! drags.

/// A pixel position on the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// QuickDraw semantics: right and bottom are exclusive, so a rect from
/// (10, 10) to (20, 20) covers 10 x 10 pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// QuickDraw's Pt2Rect: the rect spanned by two corners, in any order.
    pub fn from_points(a: Point, b: Point) -> Self {
        Self::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    pub fn inset_by(&self, dx: i32, dy: i32) -> Self {
        Self::new(
            self.left + dx,
            self.top + dy,
            self.right - dx,
            self.bottom - dy,
        )
    }

    pub fn union(&self, other: &Rect) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Self::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }
}

/// MacPaint's Trunc8: rounds down to a multiple of 8.
pub fn trunc_to_grid(value: i32) -> i32 {
    if value >= 0 {
        value & !7
    } else {
        -((-value + 7) & !7)
    }
}

/// MacPaint's GridPoint, with Goodies > Grid on: the nearest point on the
/// 8 pixel grid (Trunc8(pt + 4)).
pub fn snap_to_grid(point: Point) -> Point {
    Point::new(trunc_to_grid(point.x + 4), trunc_to_grid(point.y + 4))
}

/// MacPaint's NearPt: within tolerance horizontally and vertically.
pub fn near_point(a: Point, b: Point, tolerance: i32) -> bool {
    (a.x - b.x).abs() <= tolerance && (a.y - b.y).abs() <= tolerance
}

/// MacPaint's Constrain, used with Shift held: makes the drag square (for
/// shapes) or diagonal. With `allow_hv`, a drag that is mostly horizontal or
/// vertical snaps to that axis instead (for lines and polygon edges).
pub fn constrain(start: Point, p: Point, allow_hv: bool) -> Point {
    let dx = p.x - start.x;
    let dy = p.y - start.y;
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();
    let delta = abs_dx.min(abs_dy);
    let mut result = Point::new(
        start.x + if dx < 0 { -delta } else { delta },
        start.y + if dy < 0 { -delta } else { delta },
    );

    if allow_hv {
        if abs_dx > 2 * abs_dy {
            result = Point::new(p.x, start.y);
        }
        if abs_dy > 2 * abs_dx {
            result = Point::new(start.x, p.y);
        }
    }
    result
}

/// MacPaint's InitConstrain/HVConstrain, used with Shift held by the pencil
/// and brushes: locks to whichever axis the stroke first moves along.
#[derive(Clone, Copy, Debug)]
pub struct AxisLock {
    start: Point,
    horizontal: bool,
    vertical: bool,
}

impl AxisLock {
    pub fn new(start: Point) -> Self {
        Self {
            start,
            horizontal: true,
            vertical: true,
        }
    }

    pub fn apply(&mut self, mut p: Point) -> Point {
        if self.horizontal && self.vertical {
            // Still choosing a direction.
            let dx = (p.x - self.start.x).abs();
            let dy = (p.y - self.start.y).abs();
            if dx > dy && dx > 1 {
                self.vertical = false;
            }
            if dy > dx && dy > 1 {
                self.horizontal = false;
            }
        }
        if self.horizontal {
            p.y = self.start.y;
        }
        if self.vertical {
            p.x = self.start.x;
        }
        p
    }
}

impl Default for AxisLock {
    fn default() -> Self {
        Self::new(Point::default())
    }
}
