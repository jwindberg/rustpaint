//! The page: a 1-bit image, and QuickDraw-style painting on it.

use std::fmt;

use crate::geometry::{Point, Rect};

/// An 8 x 8 1-bit pattern, one byte per row, MSB is the leftmost pixel
/// (the same layout as a QuickDraw Pattern).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pattern {
    pub rows: [u8; 8],
}

impl Pattern {
    pub const fn new(rows: [u8; 8]) -> Self {
        Self { rows }
    }

    pub fn bit(&self, x: i32, y: i32) -> bool {
        (self.rows[(y & 7) as usize] >> (7 - (x & 7))) & 1 != 0
    }
}

pub const BLACK_PATTERN: Pattern = Pattern::new([0xff; 8]);
pub const WHITE_PATTERN: Pattern = Pattern::new([0x00; 8]);

/// A 16 x 16 1-bit mask (MacPaint's Bits16): one word per row, MSB leftmost.
/// Brushes and the spray can paint the current pattern through one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mask16 {
    pub rows: [u16; 16],
}

impl Mask16 {
    pub const fn new(rows: [u16; 16]) -> Self {
        Self { rows }
    }

    pub fn bit(&self, x: i32, y: i32) -> bool {
        (self.rows[y as usize] >> (15 - x)) & 1 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Rect,
    RoundRect,
    Oval,
}

/// How patterns are painted: Copy sets every pixel to the pattern; Or
/// (QuickDraw's patOr, MacPaint's Command key) paints only the pattern's
/// black pixels and leaves the rest alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PaintMode {
    #[default]
    Copy,
    Or,
}

/// MacPaint's corner: the diameter of a round rect's corner curves.
pub const ROUND_RECT_CORNER: i32 = 18;

/// A 1-bit image. true is black, false is white.
#[derive(Clone)]
pub struct Canvas {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
    mode: PaintMode,
}

impl Canvas {
    /// A full MacPaint document is 576 x 720 pixels (8 x 10 inches at
    /// 72 dpi).
    pub const DEFAULT_WIDTH: i32 = 576;
    pub const DEFAULT_HEIGHT: i32 = 720;

    pub fn new(width: i32, height: i32) -> Self {
        let size = width.max(0) as usize * height.max(0) as usize;
        Self {
            width,
            height,
            pixels: vec![0; size],
            mode: PaintMode::Copy,
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    /// Out-of-bounds reads return white; out-of-bounds writes are ignored,
    /// so tools can draw past the edges safely.
    pub fn get(&self, x: i32, y: i32) -> bool {
        self.contains(x, y) && self.pixels[self.index(x, y)] != 0
    }

    pub fn set(&mut self, x: i32, y: i32, black: bool) {
        if self.contains(x, y) {
            let index = self.index(x, y);
            self.pixels[index] = black as u8;
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    /// Applies to everything below that paints a pattern, except
    /// [`Canvas::flood_fill`], which always copies.
    pub fn set_paint_mode(&mut self, mode: PaintMode) {
        self.mode = mode;
    }

    pub fn paint_mode(&self) -> PaintMode {
        self.mode
    }

    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, black: bool) {
        for_each_line_point(x0, y0, x1, y1, |x, y| self.set(x, y, black));
    }

    /// Fills the rectangle (inclusive coordinates, in any order).
    pub fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, pattern: Pattern) {
        let left = x0.min(x1).max(0);
        let right = x0.max(x1).min(self.width - 1);
        let top = y0.min(y1).max(0);
        let bottom = y0.max(y1).min(self.height - 1);

        for y in top..=bottom {
            for x in left..=right {
                self.paint(self.index(x, y), pattern.bit(x, y));
            }
        }
    }

    /// Drags a `pen_size` x `pen_size` square pen along the line. Like a
    /// QuickDraw pen, it hangs down and to the right of each point.
    pub fn stroke_line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        pen_size: i32,
        pattern: Pattern,
    ) {
        if pen_size < 1 {
            return;
        }

        for_each_line_point(x0, y0, x1, y1, |x, y| {
            self.fill_rect(x, y, x + pen_size - 1, y + pen_size - 1, pattern);
        });
    }

    /// Fills the 4-connected region of same-colored pixels containing
    /// (x, y). The pattern is aligned to canvas coordinates, as in MacPaint,
    /// so adjacent fills line up seamlessly.
    pub fn flood_fill(&mut self, x: i32, y: i32, pattern: Pattern) {
        if !self.contains(x, y) {
            return;
        }

        // Find the whole region first, then paint it. Painting while
        // searching would loop forever when the pattern contains the seed
        // color.
        let target = self.pixels[self.index(x, y)];
        let mut in_region = vec![false; self.pixels.len()];
        let mut stack = vec![(x, y)];

        while let Some((px, py)) = stack.pop() {
            if !self.contains(px, py) {
                continue;
            }
            let index = self.index(px, py);
            if in_region[index] || self.pixels[index] != target {
                continue;
            }
            in_region[index] = true;
            stack.extend([(px + 1, py), (px - 1, py), (px, py + 1), (px, py - 1)]);
        }

        for py in 0..self.height {
            for px in 0..self.width {
                let index = self.index(px, py);
                if in_region[index] {
                    self.pixels[index] = pattern.bit(px, py) as u8;
                }
            }
        }
    }

    /// QuickDraw's PaintRect/PaintRoundRect/PaintOval: fills the shape
    /// inscribed in rect.
    pub fn fill_shape(&mut self, shape: Shape, rect: Rect, pattern: Pattern) {
        let clip = self.clip(rect);
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                if inside_shape(shape, &rect, ROUND_RECT_CORNER, x, y) {
                    self.paint(self.index(x, y), pattern.bit(x, y));
                }
            }
        }
    }

    /// QuickDraw's FrameRect/FrameRoundRect/FrameOval: the shape minus the
    /// same shape inset by `pen_size`, so the border lies inside rect.
    pub fn frame_shape(&mut self, shape: Shape, rect: Rect, pen_size: i32, pattern: Pattern) {
        // As in QuickDraw, the inner shape's corners shrink with the pen.
        let inner = rect.inset_by(pen_size, pen_size);
        let inner_corner = (ROUND_RECT_CORNER - 2 * pen_size).max(0);

        let clip = self.clip(rect);
        for y in clip.top..clip.bottom {
            for x in clip.left..clip.right {
                if inside_shape(shape, &rect, ROUND_RECT_CORNER, x, y)
                    && !inside_shape(shape, &inner, inner_corner, x, y)
                {
                    self.paint(self.index(x, y), pattern.bit(x, y));
                }
            }
        }
    }

    /// Fills the polygon (implicitly closed) using the even-odd rule,
    /// testing pixel centers.
    pub fn fill_polygon(&mut self, points: &[Point], pattern: Pattern) {
        if points.len() < 3 {
            return;
        }

        let top = points.iter().map(|p| p.y).min().unwrap_or(0).max(0);
        let bottom = points
            .iter()
            .map(|p| p.y)
            .max()
            .unwrap_or(0)
            .min(self.height);

        let mut crossings: Vec<f64> = Vec::new();
        for y in top..bottom {
            // Where the scanline through the pixel centers crosses each edge.
            let center = y as f64 + 0.5;
            crossings.clear();
            for (i, a) in points.iter().enumerate() {
                let b = points[(i + 1) % points.len()];
                if (a.y as f64 <= center) != (b.y as f64 <= center) {
                    crossings.push(
                        a.x as f64
                            + (center - a.y as f64) * (b.x - a.x) as f64 / (b.y - a.y) as f64,
                    );
                }
            }
            crossings.sort_by(f64::total_cmp);

            // Even-odd: fill pixels whose centers lie between pairs.
            let (pairs, _) = crossings.as_chunks::<2>();
            for &[start, end] in pairs {
                let from = ((start - 0.5).ceil() as i32).max(0);
                let to = ((end - 0.5).ceil() as i32).min(self.width);
                for x in from..to {
                    self.paint(self.index(x, y), pattern.bit(x, y));
                }
            }
        }
    }

    /// [`Canvas::stroke_line`] through consecutive points; also from the
    /// last point back to the first when closed.
    pub fn stroke_polyline(
        &mut self,
        points: &[Point],
        closed: bool,
        pen_size: i32,
        pattern: Pattern,
    ) {
        match points {
            [] => {}
            [only] => self.stroke_line(only.x, only.y, only.x, only.y, pen_size, pattern),
            [first, .., last] => {
                for pair in points.windows(2) {
                    self.stroke_line(
                        pair[0].x, pair[0].y, pair[1].x, pair[1].y, pen_size, pattern,
                    );
                }
                if closed {
                    self.stroke_line(last.x, last.y, first.x, first.y, pen_size, pattern);
                }
            }
        }
    }

    /// MacPaint's DrawBrush: paints the pattern wherever the mask is set,
    /// with the mask centered on (center_x, center_y), i.e. its top left at
    /// (center_x - 8, center_y - 8). Other pixels are left alone.
    pub fn stamp_mask(&mut self, mask: &Mask16, center_x: i32, center_y: i32, pattern: Pattern) {
        let left = center_x - 8;
        let top = center_y - 8;
        for y in 0..16 {
            for x in 0..16 {
                if mask.bit(x, y) && self.contains(left + x, top + y) {
                    self.paint(
                        self.index(left + x, top + y),
                        pattern.bit(left + x, top + y),
                    );
                }
            }
        }
    }

    /// MacPaint's BrushLine: stamps along the line from (x0, y0) to
    /// (x1, y1), one stamp per step, not including the start.
    pub fn stroke_mask(
        &mut self,
        mask: &Mask16,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        pattern: Pattern,
    ) {
        for point in mask_stroke_points(x0, y0, x1, y1) {
            self.stamp_mask(mask, point.x, point.y, pattern);
        }
    }

    /// Copies the pixels of rect from source, which must be the same size.
    /// Used to restore a snapshot under a live preview.
    pub fn copy_rect(&mut self, source: &Canvas, rect: Rect) {
        if source.width != self.width || source.height != self.height {
            return;
        }

        let clip = self.clip(rect);
        if clip.is_empty() {
            return;
        }
        for y in clip.top..clip.bottom {
            let start = self.index(clip.left, y);
            let end = start + clip.width() as usize;
            self.pixels[start..end].copy_from_slice(&source.pixels[start..end]);
        }
    }

    /// Moves the whole image by (dx, dy), as Show Page's page shift does:
    /// what moves off the page is lost, and white comes in.
    pub fn offset(&mut self, dx: i32, dy: i32) {
        let mut moved = vec![0; self.pixels.len()];
        for y in 0..self.height {
            let from_y = y - dy;
            if from_y < 0 || from_y >= self.height {
                continue;
            }
            for x in 0..self.width {
                let from_x = x - dx;
                if from_x >= 0 && from_x < self.width {
                    moved[self.index(x, y)] = self.pixels[self.index(from_x, from_y)];
                }
            }
        }
        self.pixels = moved;
    }

    /// One byte per pixel, row by row: 1 is black, 0 is white.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn index(&self, x: i32, y: i32) -> usize {
        y as usize * self.width as usize + x as usize
    }

    fn clip(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.left.max(0),
            rect.top.max(0),
            rect.right.min(self.width),
            rect.bottom.min(self.height),
        )
    }

    fn paint(&mut self, index: usize, black: bool) {
        if black || self.mode == PaintMode::Copy {
            self.pixels[index] = black as u8;
        }
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT)
    }
}

impl fmt::Debug for Canvas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Canvas")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

/// Calls plot(x, y) for each point of a Bresenham line, all octants.
fn for_each_line_point(mut x0: i32, mut y0: i32, x1: i32, y1: i32, mut plot: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        plot(x0, y0);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Whether the center of pixel (x, y) is inside the ellipse inscribed in
/// rect.
fn inside_ellipse(rect: &Rect, x: i32, y: i32) -> bool {
    let a = rect.width() as f64 / 2.0;
    let b = rect.height() as f64 / 2.0;
    let dx = (x as f64 + 0.5 - (rect.left as f64 + a)) / a;
    let dy = (y as f64 + 0.5 - (rect.top as f64 + b)) / b;
    dx * dx + dy * dy <= 1.0
}

/// Whether pixel (x, y) is inside the shape inscribed in rect. corner is
/// the round rect corner diameter.
fn inside_shape(shape: Shape, rect: &Rect, corner: i32, x: i32, y: i32) -> bool {
    if rect.is_empty() || x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom {
        return false;
    }

    match shape {
        Shape::Rect => true,
        Shape::Oval => inside_ellipse(rect, x, y),
        Shape::RoundRect => {
            let corner_width = corner.min(rect.width());
            let corner_height = corner.min(rect.height());
            if corner_width <= 0 || corner_height <= 0 {
                return true;
            }

            let left = x < rect.left + corner_width / 2;
            let right = x >= rect.right - corner_width / 2;
            let top = y < rect.top + corner_height / 2;
            let bottom = y >= rect.bottom - corner_height / 2;
            if !(left || right) || !(top || bottom) {
                return true;
            }

            // In one of the four corners: test against that corner's oval.
            let corner_left = if left {
                rect.left
            } else {
                rect.right - corner_width
            };
            let corner_top = if top {
                rect.top
            } else {
                rect.bottom - corner_height
            };
            inside_ellipse(
                &Rect::new(
                    corner_left,
                    corner_top,
                    corner_left + corner_width,
                    corner_top + corner_height,
                ),
                x,
                y,
            )
        }
    }
}

/// The points MacPaint's BrushLine stamps a brush at going from (x0, y0) to
/// (x1, y1): one per step, not including the start.
pub fn mask_stroke_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<Point> {
    // MacPaint steps in Fixed point from the center of the start pixel and
    // takes the integer part (HiWord) of each step.
    let (x0, y0) = (x0 as i64, y0 as i64);
    let dx = x1 as i64 - x0;
    let dy = y1 as i64 - y0;
    let steps = dx.abs().max(dy.abs());
    (1..=steps)
        .map(|step| {
            Point::new(
                (2 * x0 * steps + steps + 2 * dx * step).div_euclid(2 * steps) as i32,
                (2 * y0 * steps + steps + 2 * dy * step).div_euclid(2 * steps) as i32,
            )
        })
        .collect()
}

/// The canvas shrunk by factor in each direction: each factor x factor
/// block becomes black when at least `minimum_black` of its pixels are. For
/// Show Page (a third) and Print Catalog (an eighth).
pub fn reduced(source: &Canvas, factor: i32, minimum_black: i32) -> Canvas {
    let mut result = Canvas::new(source.width() / factor, source.height() / factor);
    for y in 0..result.height() {
        for x in 0..result.width() {
            let mut black = 0;
            for dy in 0..factor {
                for dx in 0..factor {
                    black += source.get(x * factor + dx, y * factor + dy) as i32;
                }
            }
            result.set(x, y, black >= minimum_black);
        }
    }
    result
}
