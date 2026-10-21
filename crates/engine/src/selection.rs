//! Selections made with MacPaint's selection rectangle or lasso, and the
//! Edit menu operations on them.

use crate::canvas::{BLACK_PATTERN, Canvas, Pattern};
use crate::geometry::{Point, Rect};

/// A selected piece of the page, as made by MacPaint's selection rectangle
/// or lasso. The pixels are a copy; the page itself is only changed by
/// [`Selection::stamp`] and [`Selection::erase_under`].
#[derive(Clone, Debug)]
pub struct Selection {
    /// Where it is on the page.
    pub rect: Rect,
    /// Rect-sized.
    pub pixels: Canvas,
    /// Rect-sized; set where selected.
    pub mask: Canvas,
    /// Lasso selections are transparent outside the mask when moved.
    pub lasso: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            pixels: Canvas::new(0, 0),
            mask: Canvas::new(0, 0),
            lasso: false,
        }
    }
}

/// HaikuPaint's clipboard format: "HPSL", width and height (big endian 32
/// bit), the lasso flag, then one byte per pixel and one per mask pixel,
/// row by row.
const SELECTION_MAGIC: &[u8; 4] = b"HPSL";
const SELECTION_HEADER_SIZE: usize = 13;
const MAX_SELECTION_SIDE: u32 = 4096;

impl Selection {
    /// A rectangular selection of the page, clipped to it.
    pub fn from_rect(page: &Canvas, rect: Rect) -> Self {
        let clipped = Rect::new(
            rect.left.max(0),
            rect.top.max(0),
            rect.right.min(page.width()),
            rect.bottom.min(page.height()),
        );
        if clipped.is_empty() {
            return Self::default();
        }

        let mut mask = Canvas::new(clipped.width(), clipped.height());
        mask.fill_rect(
            0,
            0,
            clipped.width() - 1,
            clipped.height() - 1,
            BLACK_PATTERN,
        );
        Self {
            rect: clipped,
            pixels: copy_region(page, clipped),
            mask,
            lasso: false,
        }
    }

    /// MacPaint's Lasso: selects what the loop encloses, shrunk to fit.
    /// Black pixels and the white areas they fully enclose are selected;
    /// white background connected to the outside of the loop is not.
    /// Returns None if nothing would be selected.
    pub fn from_lasso(page: &Canvas, points: &[Point]) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }

        // The pixels the loop covers, grown by one so the corner is outside
        // it.
        let mut bounds = Rect::new(points[0].x, points[0].y, points[0].x, points[0].y);
        for point in points {
            bounds.left = bounds.left.min(point.x);
            bounds.top = bounds.top.min(point.y);
            bounds.right = bounds.right.max(point.x);
            bounds.bottom = bounds.bottom.max(point.y);
        }
        let bounds = Rect::new(
            bounds.left - 1,
            bounds.top - 1,
            bounds.right + 2,
            bounds.bottom + 2,
        );

        // The region inside the loop, including the loop itself.
        let local: Vec<Point> = points
            .iter()
            .map(|p| Point::new(p.x - bounds.left, p.y - bounds.top))
            .collect();
        let mut region = Canvas::new(bounds.width(), bounds.height());
        region.fill_polygon(&local, BLACK_PATTERN);
        region.stroke_polyline(&local, true, 1, BLACK_PATTERN);

        // The page inside the region, on white.
        let mut work = Canvas::new(bounds.width(), bounds.height());
        for y in 0..work.height() {
            for x in 0..work.width() {
                if region.get(x, y) {
                    work.set(x, y, page.get(bounds.left + x, bounds.top + y));
                }
            }
        }

        // CalcMask: flood the white reachable from the corner; everything it
        // can't reach is selected. Filling black from the corner marks it.
        let mut reached = work.clone();
        reached.flood_fill(0, 0, BLACK_PATTERN);
        let selected = |x, y| !reached.get(x, y) || work.get(x, y);

        let mut trimmed = Rect::new(work.width(), work.height(), 0, 0);
        for y in 0..work.height() {
            for x in 0..work.width() {
                if selected(x, y) {
                    trimmed.left = trimmed.left.min(x);
                    trimmed.top = trimmed.top.min(y);
                    trimmed.right = trimmed.right.max(x + 1);
                    trimmed.bottom = trimmed.bottom.max(y + 1);
                }
            }
        }
        if trimmed.is_empty() {
            return None;
        }

        let rect = Rect::new(
            bounds.left + trimmed.left,
            bounds.top + trimmed.top,
            bounds.left + trimmed.right,
            bounds.top + trimmed.bottom,
        );
        let mut mask = Canvas::new(trimmed.width(), trimmed.height());
        for y in 0..trimmed.height() {
            for x in 0..trimmed.width() {
                mask.set(x, y, selected(trimmed.left + x, trimmed.top + y));
            }
        }
        Some(Self {
            rect,
            pixels: copy_region(page, rect),
            mask,
            lasso: true,
        })
    }

    /// Whether the point (page coordinates) is on the selection.
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.rect.left
            && point.x < self.rect.right
            && point.y >= self.rect.top
            && point.y < self.rect.bottom
            && self
                .mask
                .get(point.x - self.rect.left, point.y - self.rect.top)
    }

    /// The pixels MacPaint marches its ants over: the inner edge of the
    /// mask, which for a rectangle is its frame. Page coordinates.
    pub fn edges(&self) -> Vec<Point> {
        let mask = &self.mask;
        let mut edges = Vec::new();
        for y in 0..mask.height() {
            for x in 0..mask.width() {
                if mask.get(x, y)
                    && (!mask.get(x - 1, y)
                        || !mask.get(x + 1, y)
                        || !mask.get(x, y - 1)
                        || !mask.get(x, y + 1))
                {
                    edges.push(Point::new(self.rect.left + x, self.rect.top + y));
                }
            }
        }
        edges
    }

    /// Paints the selection onto the page: rect selections opaquely, lasso
    /// selections only where the mask is set.
    pub fn stamp(&self, page: &mut Canvas) {
        for y in 0..self.rect.height() {
            for x in 0..self.rect.width() {
                if self.mask.get(x, y) {
                    page.set(self.rect.left + x, self.rect.top + y, self.pixels.get(x, y));
                }
            }
        }
    }

    /// Whitens the page under the selection's mask: the hole left behind
    /// when a selection is moved or cleared.
    pub fn erase_under(&self, page: &mut Canvas) {
        for y in 0..self.rect.height() {
            for x in 0..self.rect.width() {
                if self.mask.get(x, y) {
                    page.set(self.rect.left + x, self.rect.top + y, false);
                }
            }
        }
    }

    // Edit menu operations, on the selection's pixels (and mask where it
    // matters).

    pub fn invert(&mut self) {
        for y in 0..self.rect.height() {
            for x in 0..self.rect.width() {
                if self.mask.get(x, y) {
                    let black = self.pixels.get(x, y);
                    self.pixels.set(x, y, !black);
                }
            }
        }
    }

    pub fn fill(&mut self, pattern: Pattern) {
        // Aligned to the page, like every other pattern fill.
        for y in 0..self.rect.height() {
            for x in 0..self.rect.width() {
                if self.mask.get(x, y) {
                    self.pixels
                        .set(x, y, pattern.bit(self.rect.left + x, self.rect.top + y));
                }
            }
        }
    }

    pub fn flip_horizontal(&mut self) {
        let (width, height) = (self.rect.width(), self.rect.height());
        let map = |x, y| Point::new(width - 1 - x, y);
        self.pixels = remapped(&self.pixels, width, height, map);
        self.mask = remapped(&self.mask, width, height, map);
    }

    pub fn flip_vertical(&mut self) {
        let (width, height) = (self.rect.width(), self.rect.height());
        let map = |x, y| Point::new(x, height - 1 - y);
        self.pixels = remapped(&self.pixels, width, height, map);
        self.mask = remapped(&self.mask, width, height, map);
    }

    /// The selection keeps its center when rotated.
    pub fn rotate_left(&mut self) {
        // Counterclockwise, as MacPaint's Rotate: the right edge becomes the
        // top. New pixel (x, y) comes from old (width - 1 - y, x).
        let (width, height) = (self.rect.width(), self.rect.height());
        let map = |x, y| Point::new(width - 1 - y, x);
        self.pixels = remapped(&self.pixels, height, width, map);
        self.mask = remapped(&self.mask, height, width, map);

        let left = self.rect.left + (width - height) / 2;
        let top = self.rect.top + (height - width) / 2;
        self.rect = Rect::new(left, top, left + height, top + width);
    }

    /// MacPaint's Trace Edges: outlines black areas by smearing them one
    /// pixel each way and removing the original. shadow (Shift in MacPaint)
    /// smears two pixels right and down instead, for an outline with a drop
    /// shadow.
    pub fn trace_edges(&mut self, shadow: bool) {
        let delta = if shadow { 3 } else { 2 };

        if self.lasso {
            // Only what's selected gets outlined, and the outline needs room
            // around it.
            for y in 0..self.rect.height() {
                for x in 0..self.rect.width() {
                    if !self.mask.get(x, y) {
                        self.pixels.set(x, y, false);
                    }
                }
            }
            self.pad(1, 1, delta - 1, delta - 1);
        }

        let width = self.rect.width();
        let height = self.rect.height();
        let source = Rect::new(1, 1, width - delta + 1, height - delta + 1);
        if source.is_empty() {
            return;
        }

        let smears = [(-1, 0), (delta - 1, 0), (0, -1), (0, delta - 1)];
        let original = self.pixels.clone();
        for (dx, dy) in smears {
            smear(&mut self.pixels, source, dx, dy);
        }

        // Punch out the original, leaving the outline.
        for y in source.top..source.bottom {
            for x in source.left..source.right {
                if original.get(x, y) {
                    self.pixels.set(x, y, false);
                }
            }
        }

        if self.lasso {
            for (dx, dy) in smears {
                smear(&mut self.mask, source, dx, dy);
            }
        }
    }

    /// The selection scaled to fill rect (nearest neighbor), for stretching.
    pub fn stretched(&self, rect: Rect) -> Selection {
        if rect.is_empty() || self.rect.is_empty() {
            return Selection::default();
        }

        let source_width = self.rect.width() as i64;
        let source_height = self.rect.height() as i64;
        let width = rect.width() as i64;
        let height = rect.height() as i64;
        let map = |x: i32, y: i32| {
            Point::new(
                ((2 * x as i64 + 1) * source_width / (2 * width)) as i32,
                ((2 * y as i64 + 1) * source_height / (2 * height)) as i32,
            )
        };
        Selection {
            rect,
            pixels: remapped(&self.pixels, rect.width(), rect.height(), map),
            mask: remapped(&self.mask, rect.width(), rect.height(), map),
            lasso: self.lasso,
        }
    }

    /// HaikuPaint's clipboard format: keeps the mask, so lasso selections
    /// paste transparent. The position isn't stored; pasting places it.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = SELECTION_MAGIC.to_vec();
        out.extend_from_slice(&(self.rect.width() as u32).to_be_bytes());
        out.extend_from_slice(&(self.rect.height() as u32).to_be_bytes());
        out.push(self.lasso as u8);
        out.extend_from_slice(self.pixels.pixels());
        out.extend_from_slice(self.mask.pixels());
        out
    }

    /// Reads [`Selection::encode`]'s format, placed at (0, 0). None if the
    /// data isn't a valid selection.
    pub fn decode(data: &[u8]) -> Option<Selection> {
        if data.len() < SELECTION_HEADER_SIZE || !data.starts_with(SELECTION_MAGIC) {
            return None;
        }

        let width = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let height = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        if width == 0
            || height == 0
            || width > MAX_SELECTION_SIDE
            || height > MAX_SELECTION_SIDE
            || data.len() != SELECTION_HEADER_SIZE + 2 * width as usize * height as usize
        {
            return None;
        }

        let (width, height) = (width as i32, height as i32);
        let area = width as usize * height as usize;
        let pixel_data = &data[SELECTION_HEADER_SIZE..SELECTION_HEADER_SIZE + area];
        let mask_data = &data[SELECTION_HEADER_SIZE + area..];
        let mut pixels = Canvas::new(width, height);
        let mut mask = Canvas::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let index = (y * width + x) as usize;
                pixels.set(x, y, pixel_data[index] != 0);
                mask.set(x, y, mask_data[index] != 0);
            }
        }
        Some(Selection {
            rect: Rect::new(0, 0, width, height),
            pixels,
            mask,
            lasso: data[12] != 0,
        })
    }

    /// Grows the selection by the given margins, unselected and white.
    fn pad(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        let old = self.rect;
        let rect = Rect::new(
            old.left - left,
            old.top - top,
            old.right + right,
            old.bottom + bottom,
        );
        let mut pixels = Canvas::new(rect.width(), rect.height());
        let mut mask = Canvas::new(rect.width(), rect.height());
        for y in 0..old.height() {
            for x in 0..old.width() {
                pixels.set(left + x, top + y, self.pixels.get(x, y));
                mask.set(left + x, top + y, self.mask.get(x, y));
            }
        }
        self.rect = rect;
        self.pixels = pixels;
        self.mask = mask;
    }
}

/// QuickDraw's MapRect: rect mapped from one frame of reference to another.
pub fn map_rect(rect: Rect, from: Rect, to: Rect) -> Rect {
    if from.width() == 0 || from.height() == 0 {
        return rect;
    }

    let map_x = |x: i32| to.left + (x - from.left) * to.width() / from.width();
    let map_y = |y: i32| to.top + (y - from.top) * to.height() / from.height();
    Rect::new(
        map_x(rect.left),
        map_y(rect.top),
        map_x(rect.right),
        map_y(rect.bottom),
    )
}

/// A copy of rect from the page; white where rect is off the page.
fn copy_region(page: &Canvas, rect: Rect) -> Canvas {
    let mut region = Canvas::new(rect.width(), rect.height());
    for y in 0..rect.height() {
        for x in 0..rect.width() {
            region.set(x, y, page.get(rect.left + x, rect.top + y));
        }
    }
    region
}

/// The canvas with pixel (x, y) taken from map(x, y) of the source.
fn remapped(source: &Canvas, width: i32, height: i32, map: impl Fn(i32, i32) -> Point) -> Canvas {
    let mut result = Canvas::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let from = map(x, y);
            result.set(x, y, source.get(from.x, from.y));
        }
    }
    result
}

/// One of TraceEdges' CopyBits(..., srcOr) smears: ORs the source area of
/// the canvas into itself, offset by (dx, dy).
fn smear(canvas: &mut Canvas, source: Rect, dx: i32, dy: i32) {
    let copy = canvas.clone();
    for y in source.top..source.bottom {
        for x in source.left..source.right {
            if copy.get(x, y) {
                canvas.set(x + dx, y + dy, true);
            }
        }
    }
}
