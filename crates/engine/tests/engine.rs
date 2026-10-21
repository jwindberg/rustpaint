//! The engine tests, ported from HaikuPaint's tests/test_canvas.cpp.

use rustpaint_engine::canvas::{
    BLACK_PATTERN, Canvas, Mask16, PaintMode, Pattern, Shape, WHITE_PATTERN, reduced,
};
use rustpaint_engine::dither::dither_gray;
use rustpaint_engine::geometry::{AxisLock, Point, Rect, constrain, snap_to_grid};
use rustpaint_engine::macpaint_file::{
    macpaint_data_offset, pack_bits, read_macpaint, unpack_bits, write_macpaint,
};
use rustpaint_engine::patterns::{DEFAULT_PATTERN_INDEX, PATTERNS, WHITE_PATTERN_INDEX};
use rustpaint_engine::selection::{Selection, map_rect};
use rustpaint_engine::symmetry::{NO_SYMMETRY, map_symmetry, symmetric_points};

fn count_black(canvas: &Canvas) -> usize {
    canvas.pixels().iter().map(|&pixel| pixel as usize).sum()
}

fn points(list: &[(i32, i32)]) -> Vec<Point> {
    list.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

const FULL_MASK: Mask16 = Mask16::new([0xffff; 16]);
const CHECKER: Pattern = Pattern::new([0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]);

#[test]
fn new_canvas_is_white() {
    let canvas = Canvas::default();
    assert_eq!(canvas.width(), 576);
    assert_eq!(canvas.height(), 720);
    assert_eq!(count_black(&canvas), 0);
}

#[test]
fn out_of_bounds_is_safe() {
    let mut canvas = Canvas::new(10, 10);
    canvas.set(-1, 0, true);
    canvas.set(10, 10, true);
    assert_eq!(count_black(&canvas), 0);
    assert!(!canvas.get(-5, -5));
}

#[test]
fn lines() {
    let mut canvas = Canvas::new(20, 20);
    canvas.draw_line(0, 0, 9, 0, true);
    assert_eq!(count_black(&canvas), 10);
    assert!(canvas.get(0, 0) && canvas.get(9, 0));

    canvas.clear();
    canvas.draw_line(9, 9, 0, 0, true);
    assert_eq!(count_black(&canvas), 10);
    assert!(canvas.get(5, 5));

    // A single point is still drawn.
    canvas.clear();
    canvas.draw_line(3, 4, 3, 4, true);
    assert_eq!(count_black(&canvas), 1);
}

#[test]
fn flood_fill_stops_at_border() {
    let mut canvas = Canvas::new(20, 20);
    // Box outline from (5,5) to (10,10): 6x6 border, 4x4 interior.
    canvas.draw_line(5, 5, 10, 5, true);
    canvas.draw_line(10, 5, 10, 10, true);
    canvas.draw_line(10, 10, 5, 10, true);
    canvas.draw_line(5, 10, 5, 5, true);
    let border = count_black(&canvas);
    assert_eq!(border, 20);

    canvas.flood_fill(7, 7, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), border + 16);
    assert!(!canvas.get(0, 0));
}

#[test]
fn flood_fill_with_pattern_terminates() {
    // A checkerboard contains both colors; a naive fill would never end.
    let mut canvas = Canvas::new(16, 16);
    canvas.flood_fill(0, 0, CHECKER);
    assert_eq!(count_black(&canvas), 16 * 16 / 2);
    assert!(canvas.get(0, 0) && !canvas.get(1, 0) && canvas.get(1, 1));
}

#[test]
fn fill_rect() {
    let mut canvas = Canvas::new(20, 20);
    // Corners in any order, inclusive.
    canvas.fill_rect(5, 6, 2, 3, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 4 * 4);
    assert!(canvas.get(2, 3) && canvas.get(5, 6) && !canvas.get(6, 6));

    // Clipped at the edges.
    canvas.clear();
    canvas.fill_rect(-10, -10, 1, 1, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 4);
}

#[test]
fn stroke_line_square_pen() {
    let mut canvas = Canvas::new(40, 40);
    // A 3x3 pen hangs down-right: a horizontal stroke from x 10 to 19
    // covers x 10..21, y 5..7.
    canvas.stroke_line(10, 5, 19, 5, 3, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 12 * 3);
    assert!(canvas.get(10, 5) && canvas.get(21, 7));
    assert!(!canvas.get(9, 5) && !canvas.get(10, 8));

    // Pen size 1 matches draw_line.
    let mut thin = Canvas::new(40, 40);
    thin.draw_line(1, 2, 30, 17, true);
    canvas.clear();
    canvas.stroke_line(1, 2, 30, 17, 1, BLACK_PATTERN);
    assert_eq!(canvas.pixels(), thin.pixels());
}

#[test]
fn eraser_stroke() {
    // Erasing is a stroke with the white pattern.
    let mut canvas = Canvas::new(40, 40);
    canvas.fill_rect(0, 0, 39, 39, BLACK_PATTERN);
    canvas.stroke_line(0, 0, 0, 0, 16, WHITE_PATTERN);
    assert_eq!(count_black(&canvas), 40 * 40 - 16 * 16);
    assert!(!canvas.get(15, 15) && canvas.get(16, 16));
}

#[test]
fn patterns_are_distinct() {
    assert_eq!(PATTERNS[DEFAULT_PATTERN_INDEX], BLACK_PATTERN);
    assert_eq!(PATTERNS[WHITE_PATTERN_INDEX], WHITE_PATTERN);

    // Also catches a botched extraction: missing entries would most likely
    // duplicate white.
    for (i, a) in PATTERNS.iter().enumerate() {
        for (j, b) in PATTERNS.iter().enumerate().skip(i + 1) {
            assert_ne!(a, b, "patterns {i} and {j} are identical");
        }
    }
}

#[test]
fn fill_and_frame_rect() {
    let mut canvas = Canvas::new(40, 40);
    // Exclusive right and bottom, as in QuickDraw.
    canvas.fill_shape(Shape::Rect, Rect::new(10, 10, 20, 15), BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 10 * 5);
    assert!(canvas.get(19, 14) && !canvas.get(20, 14) && !canvas.get(19, 15));

    // A 2 pixel frame stays inside the rect.
    canvas.clear();
    canvas.frame_shape(Shape::Rect, Rect::new(10, 10, 20, 20), 2, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 10 * 10 - 6 * 6);
    assert!(canvas.get(10, 10) && canvas.get(11, 11) && !canvas.get(12, 12));
    assert!(canvas.get(19, 19) && !canvas.get(20, 20));

    // A pen wider than half the rect fills it.
    canvas.clear();
    canvas.frame_shape(Shape::Rect, Rect::new(0, 0, 6, 6), 4, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 36);
}

#[test]
fn oval_and_round_rect() {
    let mut canvas = Canvas::new(60, 60);
    canvas.fill_shape(Shape::Oval, Rect::new(0, 0, 40, 20), BLACK_PATTERN);
    // Center filled, corners empty, extremes of the axes reached.
    assert!(canvas.get(20, 10) && !canvas.get(0, 0) && !canvas.get(39, 19));
    assert!(canvas.get(0, 10) && canvas.get(39, 9) && canvas.get(20, 0));
    // Symmetric left/right and top/bottom.
    for y in 0..20 {
        for x in 0..40 {
            assert_eq!(canvas.get(x, y), canvas.get(39 - x, y), "({x}, {y})");
            assert_eq!(canvas.get(x, y), canvas.get(x, 19 - y), "({x}, {y})");
        }
    }
    // Roughly pi * a * b.
    let area = count_black(&canvas);
    assert!(area > 600 && area < 660, "area {area}");

    // A hollow oval is a ring: the center stays white.
    canvas.clear();
    canvas.frame_shape(Shape::Oval, Rect::new(0, 0, 40, 40), 1, BLACK_PATTERN);
    assert!(!canvas.get(20, 20) && canvas.get(0, 20) && canvas.get(20, 39));

    // Round rect: square-cornered in the middle of each edge, rounded at
    // the corners.
    canvas.clear();
    canvas.fill_shape(Shape::RoundRect, Rect::new(0, 0, 50, 50), BLACK_PATTERN);
    assert!(!canvas.get(0, 0) && !canvas.get(49, 49));
    assert!(canvas.get(25, 0) && canvas.get(0, 25) && canvas.get(49, 25));
    assert!(canvas.get(9, 0) && canvas.get(4, 4));
}

#[test]
fn fill_polygon() {
    let mut canvas = Canvas::new(30, 30);
    // A square given as a polygon covers the same pixels as the rect.
    canvas.fill_polygon(
        &points(&[(5, 5), (15, 5), (15, 15), (5, 15)]),
        BLACK_PATTERN,
    );
    assert_eq!(count_black(&canvas), 100);
    assert!(canvas.get(5, 5) && canvas.get(14, 14) && !canvas.get(15, 15));

    // A right triangle covers about half a square.
    canvas.clear();
    canvas.fill_polygon(&points(&[(0, 0), (20, 0), (0, 20)]), BLACK_PATTERN);
    let area = count_black(&canvas);
    assert!((190..=210).contains(&area), "area {area}");
    assert!(canvas.get(1, 1) && !canvas.get(18, 18));

    // Fewer than three points: nothing.
    canvas.clear();
    canvas.fill_polygon(&points(&[(0, 0), (10, 10)]), BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 0);
}

#[test]
fn stamp_and_stroke_mask() {
    let mut dot = Mask16::default();
    dot.rows[8] = 0x0080; // a single pixel at (8, 8)
    let mut canvas = Canvas::new(40, 40);
    canvas.stamp_mask(&dot, 20, 20, BLACK_PATTERN);
    assert!(count_black(&canvas) == 1 && canvas.get(20, 20));

    // Only masked pixels change: stamping white leaves the rest black.
    canvas.fill_rect(0, 0, 39, 39, BLACK_PATTERN);
    canvas.stamp_mask(&dot, 5, 5, WHITE_PATTERN);
    assert!(count_black(&canvas) == 40 * 40 - 1 && !canvas.get(5, 5));
    canvas.stamp_mask(&FULL_MASK, 20, 20, WHITE_PATTERN);
    assert!(!canvas.get(12, 12) && !canvas.get(27, 27));
    assert!(canvas.get(11, 11) && canvas.get(28, 28));

    // Clipped at the edges.
    canvas.clear();
    canvas.stamp_mask(&FULL_MASK, 0, 0, BLACK_PATTERN);
    assert_eq!(count_black(&canvas), 8 * 8);

    // A stroke stamps each step after the start: a horizontal stroke of a
    // single-pixel mask from x 10 to 20 sets x 11..20.
    canvas.clear();
    canvas.stroke_mask(&dot, 10, 30, 20, 30, BLACK_PATTERN);
    assert!(count_black(&canvas) == 10 && !canvas.get(10, 30) && canvas.get(20, 30));
    canvas.clear();
    canvas.stroke_mask(&dot, 20, 10, 10, 0, BLACK_PATTERN);
    assert!(count_black(&canvas) == 10 && canvas.get(10, 0) && canvas.get(15, 5));
}

#[test]
fn copy_rect_reduce_and_offset() {
    let mut snapshot = Canvas::new(20, 20);
    snapshot.fill_rect(0, 0, 19, 19, BLACK_PATTERN);
    let mut canvas = Canvas::new(20, 20);
    canvas.copy_rect(&snapshot, Rect::new(5, 5, 10, 8));
    assert_eq!(count_black(&canvas), 5 * 3);
    canvas.copy_rect(&snapshot, Rect::new(-5, -5, 100, 100));
    assert_eq!(count_black(&canvas), 400);

    // Reducing: blocks with enough black become black.
    let mut big = Canvas::new(24, 16);
    big.fill_rect(0, 0, 7, 7, BLACK_PATTERN);
    big.set(8, 0, true);
    big.set(9, 0, true);
    let small = reduced(&big, 8, 2);
    assert!(small.width() == 3 && small.height() == 2);
    assert!(small.get(0, 0) && small.get(1, 0) && !small.get(2, 0) && !small.get(0, 1));
    assert!(!reduced(&big, 8, 3).get(1, 0));

    // Offsetting loses what moves off the page and brings in white.
    let mut page = Canvas::new(20, 20);
    page.fill_rect(0, 0, 9, 9, BLACK_PATTERN);
    page.offset(5, -3);
    assert_eq!(count_black(&page), 10 * 7);
    assert!(page.get(5, 0) && page.get(14, 6) && !page.get(4, 0) && !page.get(5, 7));
}

#[test]
fn constraints() {
    // Square drag.
    let start = Point::new(10, 10);
    assert_eq!(
        constrain(start, Point::new(30, 15), false),
        Point::new(15, 15)
    );
    assert_eq!(
        constrain(start, Point::new(0, 40), false),
        Point::new(0, 20)
    );

    // With allow_hv, mostly-horizontal snaps to horizontal.
    assert_eq!(
        constrain(start, Point::new(40, 12), true),
        Point::new(40, 10)
    );
    assert_eq!(
        constrain(start, Point::new(11, 50), true),
        Point::new(10, 50)
    );
    assert_eq!(
        constrain(start, Point::new(20, 18), true),
        Point::new(18, 18)
    );

    // AxisLock waits until the stroke has moved more than a pixel, then
    // locks to that axis.
    let mut lock = AxisLock::new(start);
    assert_eq!(lock.apply(Point::new(11, 11)), Point::new(10, 10));
    assert_eq!(lock.apply(Point::new(15, 12)), Point::new(15, 10));
    assert_eq!(lock.apply(Point::new(15, 40)), Point::new(15, 10));
}

fn round_trips(data: &[u8]) -> bool {
    let packed = pack_bits(data);
    let mut unpacked = vec![0; data.len()];
    let mut position = 0;
    unpack_bits(&packed, &mut position, &mut unpacked)
        && position == packed.len()
        && unpacked == data
}

#[test]
fn pack_bits_round_trips() {
    // MacPaint's BlankDoc writes each blank scanline as -71, 0.
    let packed = pack_bits(&[0; 72]);
    assert!(packed.len() == 2 && packed[0] as i8 == -71 && packed[1] == 0);

    // The classic example from Apple's Technical Note TN1023.
    let sample: [u8; 24] = [
        0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0xAA, 0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0x22, 0xAA,
        0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
    ];
    let tn_packed: [u8; 15] = [
        0xFE, 0xAA, 0x02, 0x80, 0x00, 0x2A, 0xFD, 0xAA, 0x03, 0x80, 0x00, 0x2A, 0x22, 0xF7, 0xAA,
    ];
    let mut out = [0; 24];
    let mut position = 0;
    assert!(unpack_bits(&tn_packed, &mut position, &mut out));
    assert_eq!(out, sample);
    assert!(round_trips(&sample));

    // Runs and literals longer than one packet, and random data.
    assert!(round_trips(&[0x55; 300]));
    let mut random = XorShift(1984);
    for trial in 0..200 {
        let length = random.next() % 400;
        let data: Vec<u8> = (0..length)
            .map(|_| {
                if random.next().is_multiple_of(4) {
                    0
                } else {
                    random.next() as u8
                }
            })
            .collect();
        assert!(
            round_trips(&data),
            "PackBits round trip failed, trial {trial}"
        );
    }

    // Truncated input fails instead of reading past the end.
    position = 0;
    assert!(!unpack_bits(&tn_packed[..5], &mut position, &mut out));
}

/// A small deterministic generator, so the tests need no dependencies.
struct XorShift(u32);

impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

#[test]
fn macpaint_file() {
    let mut canvas = Canvas::default();
    canvas.fill_shape(Shape::Oval, Rect::new(100, 100, 300, 250), PATTERNS[3]);
    canvas.draw_line(0, 0, 575, 719, true);
    canvas.set(575, 0, true);

    let file = write_macpaint(&canvas, &PATTERNS);
    assert!(file.len() > 512 && file[3] == 2);
    let pattern_bytes: Vec<u8> = PATTERNS.iter().flat_map(|pattern| pattern.rows).collect();
    assert_eq!(&file[4..4 + 38 * 8], &pattern_bytes[..]);

    let loaded = read_macpaint(&file).expect("reads back");
    assert_eq!(loaded.canvas.pixels(), canvas.pixels());

    // The document's own patterns come back; version 1 files have none.
    let mut custom = PATTERNS;
    custom[5] = Pattern::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let mut custom_file = write_macpaint(&canvas, &custom);
    assert_eq!(read_macpaint(&custom_file).unwrap().patterns, Some(custom));
    custom_file[3] = 1;
    assert_eq!(read_macpaint(&custom_file).unwrap().patterns, None);

    // A blank document is exactly BlankDoc's 512 + 720 * 2 bytes.
    assert_eq!(
        write_macpaint(&Canvas::default(), &PATTERNS).len(),
        512 + 720 * 2
    );

    // With a MacBinary header in front.
    let mut mac_binary = vec![0; 128];
    mac_binary[1] = 5;
    mac_binary[2..7].copy_from_slice(b"Hello");
    mac_binary[65..73].copy_from_slice(b"PNTGMPNT");
    mac_binary.extend_from_slice(&file);
    assert_eq!(macpaint_data_offset(&mac_binary), 128);
    let from_mac_binary = read_macpaint(&mac_binary).expect("reads MacBinary");
    assert_eq!(from_mac_binary.canvas.pixels(), canvas.pixels());
    assert_eq!(macpaint_data_offset(&file), 0);

    // Damaged and foreign files are rejected.
    let error = read_macpaint(&file[..file.len() / 2]).unwrap_err();
    assert!(!error.to_string().is_empty());
    assert!(read_macpaint(&file[..100]).is_err());
    let mut png = file.clone();
    png[..4].copy_from_slice(b"\x89PNG");
    assert!(read_macpaint(&png).is_err());
}

#[test]
fn rect_selection() {
    let mut page = Canvas::new(40, 40);
    page.fill_rect(10, 10, 14, 14, BLACK_PATTERN);

    // Clipped to the page; everything inside is selected, white too.
    let selection = Selection::from_rect(&page, Rect::new(8, 8, 50, 20));
    assert!(selection.rect.left == 8 && selection.rect.right == 40);
    assert!(!selection.lasso && selection.contains(Point::new(30, 15)));
    assert!(selection.pixels.get(2, 2) && !selection.pixels.get(0, 0));

    // Moving: erase under, then stamp at the new place, opaquely.
    let mut moved = page.clone();
    let mut small = Selection::from_rect(&page, Rect::new(10, 10, 15, 15));
    small.erase_under(&mut moved);
    assert_eq!(count_black(&moved), 0);
    small.rect = Rect::new(20, 20, 25, 25);
    small.stamp(&mut moved);
    assert!(count_black(&moved) == 25 && moved.get(20, 20) && moved.get(24, 24));

    // Edges of a rect selection are its frame.
    assert_eq!(small.edges().len(), 16);

    assert!(
        Selection::from_rect(&page, Rect::new(50, 50, 60, 60))
            .rect
            .is_empty()
    );
}

#[test]
fn lasso_shrinks_to_fit() {
    // A ring with a white hole, a separate dot, on white.
    let mut page = Canvas::new(60, 60);
    page.frame_shape(Shape::Rect, Rect::new(10, 10, 20, 20), 2, BLACK_PATTERN);
    page.set(40, 40, true);

    // A loose loop around the ring only.
    let selection = Selection::from_lasso(&page, &points(&[(2, 2), (30, 2), (30, 30), (2, 30)]))
        .expect("selects the ring");
    assert!(selection.lasso);
    // Shrunk to the ring's bounds.
    assert_eq!(selection.rect, Rect::new(10, 10, 20, 20));
    // The enclosed white hole is selected too; the dot outside is not.
    assert!(selection.contains(Point::new(15, 15)) && selection.contains(Point::new(10, 10)));
    assert!(!selection.contains(Point::new(40, 40)));

    // Moving it is transparent outside the mask: a lasso selection of a
    // diagonal line doesn't carry white over its neighbors.
    let mut diagonal = Canvas::new(30, 30);
    diagonal.draw_line(0, 0, 9, 9, true);
    let mut line = Selection::from_lasso(&diagonal, &points(&[(0, 0), (12, 0), (12, 12), (0, 12)]))
        .expect("selects the line");
    let mut target = Canvas::new(30, 30);
    target.fill_rect(0, 0, 29, 29, BLACK_PATTERN);
    line.rect = Rect::new(15, 15, 15 + line.rect.width(), 15 + line.rect.height());
    line.stamp(&mut target);
    assert_eq!(count_black(&target), 30 * 30);

    // A loop around nothing selects nothing; a click isn't a loop.
    let empty = Canvas::new(30, 30);
    assert!(Selection::from_lasso(&empty, &points(&[(1, 1), (20, 1), (20, 20)])).is_none());
    assert!(Selection::from_lasso(&page, &points(&[(5, 5)])).is_none());
}

#[test]
fn selection_transforms() {
    // An L shape in a 4 x 3 selection:
    //   #...
    //   #...
    //   ##..
    let mut page = Canvas::new(20, 20);
    page.set(0, 0, true);
    page.set(0, 1, true);
    page.set(0, 2, true);
    page.set(1, 2, true);
    let base = Selection::from_rect(&page, Rect::new(0, 0, 4, 3));

    let mut flipped = base.clone();
    flipped.flip_horizontal();
    assert!(flipped.pixels.get(3, 0) && flipped.pixels.get(2, 2) && !flipped.pixels.get(0, 0));
    flipped.flip_vertical();
    assert!(flipped.pixels.get(3, 2) && flipped.pixels.get(2, 0));

    // Rotating left: 4 x 3 becomes 3 x 4 around the same center, and the
    // bottom row (##..) becomes the left column, read upward.
    let mut rotated = base.clone();
    rotated.rotate_left();
    assert!(rotated.rect.width() == 3 && rotated.rect.height() == 4);
    assert!(
        rotated.pixels.get(0, 3)
            && rotated.pixels.get(1, 3)
            && rotated.pixels.get(2, 3)
            && rotated.pixels.get(2, 2)
    );
    assert!(!rotated.pixels.get(0, 0));
    let mut four_times = base.clone();
    for _ in 0..4 {
        four_times.rotate_left();
    }
    assert_eq!(four_times.pixels.pixels(), base.pixels.pixels());

    let mut inverted = base.clone();
    inverted.invert();
    assert!(!inverted.pixels.get(0, 0) && inverted.pixels.get(3, 0));

    let mut filled = base.clone();
    filled.fill(BLACK_PATTERN);
    assert_eq!(count_black(&filled.pixels), 12);

    // Stretching doubles each pixel.
    let stretched = base.stretched(Rect::new(0, 0, 8, 6));
    assert!(
        stretched.pixels.get(0, 0)
            && stretched.pixels.get(1, 1)
            && stretched.pixels.get(3, 5)
            && !stretched.pixels.get(4, 5)
    );

    let mapped = map_rect(
        Rect::new(10, 10, 20, 20),
        Rect::new(0, 0, 10, 10),
        Rect::new(0, 0, 20, 40),
    );
    assert_eq!(mapped, Rect::new(20, 40, 40, 80));
}

#[test]
fn trace_edges() {
    // A 2 x 2 black square in the middle of a rect selection becomes a
    // ring around where it was.
    let mut page = Canvas::new(10, 10);
    page.fill_rect(4, 4, 5, 5, BLACK_PATTERN);
    let mut selection = Selection::from_rect(&page, Rect::new(0, 0, 10, 10));
    selection.trace_edges(false);
    let pixels = &selection.pixels;
    assert!(!pixels.get(4, 4) && !pixels.get(5, 5));
    assert!(pixels.get(3, 4) && pixels.get(6, 5) && pixels.get(4, 3) && pixels.get(5, 6));
    assert!(!pixels.get(7, 4) && !pixels.get(2, 4));

    // The shadow version reaches two pixels right and down.
    let mut shadowed = Selection::from_rect(&page, Rect::new(0, 0, 10, 10));
    shadowed.trace_edges(true);
    assert!(shadowed.pixels.get(7, 4) && shadowed.pixels.get(4, 7));
    assert!(!shadowed.pixels.get(2, 4));

    // A lasso selection grows to make room for the outline.
    let mut lasso = Selection::from_lasso(&page, &points(&[(1, 1), (8, 1), (8, 8), (1, 8)]))
        .expect("selects the square");
    assert_eq!(lasso.rect.width(), 2);
    lasso.trace_edges(false);
    assert!(
        lasso.rect.width() == 4
            && lasso.contains(Point::new(3, 4))
            && lasso.contains(Point::new(6, 5))
    );
}

#[test]
fn selection_clipboard_format() {
    let mut page = Canvas::new(30, 30);
    page.frame_shape(Shape::Oval, Rect::new(2, 2, 20, 14), 1, BLACK_PATTERN);
    let selection = Selection::from_lasso(&page, &points(&[(0, 0), (25, 0), (25, 20), (0, 20)]))
        .expect("selects the oval");

    let mut data = selection.encode();
    let decoded = Selection::decode(&data).expect("decodes");
    assert!(
        decoded.lasso
            && decoded.rect.left == 0
            && decoded.rect.width() == selection.rect.width()
            && decoded.rect.height() == selection.rect.height()
    );
    assert_eq!(decoded.pixels.pixels(), selection.pixels.pixels());
    assert_eq!(decoded.mask.pixels(), selection.mask.pixels());

    assert!(Selection::decode(&data[..data.len() - 1]).is_none());
    data[0] = b'X';
    assert!(Selection::decode(&data).is_none());
}

#[test]
fn symmetry_and_grid() {
    assert_eq!(map_symmetry(false, false, false, false), NO_SYMMETRY);
    assert_eq!(map_symmetry(true, true, false, false), 0xF0);
    assert_eq!(map_symmetry(true, false, true, false), 0xFF);

    let center = Point::new(100, 100);
    let point = Point::new(110, 103);
    assert_eq!(symmetric_points(point, center, NO_SYMMETRY), vec![point]);

    // The vertical mirror reflects left/right.
    let vertical = symmetric_points(point, center, map_symmetry(true, false, false, false));
    assert!(vertical.len() == 2 && vertical.contains(&Point::new(90, 103)));

    // Both axes: four copies.
    let both = symmetric_points(point, center, map_symmetry(true, true, false, false));
    assert!(
        both.len() == 4
            && both.contains(&Point::new(90, 97))
            && both.contains(&Point::new(110, 97))
    );

    // The diagonal swaps the offsets.
    let diagonal = symmetric_points(point, center, map_symmetry(false, false, true, false));
    assert!(diagonal.len() == 2 && diagonal.contains(&Point::new(103, 110)));

    // All eight.
    assert_eq!(symmetric_points(point, center, 0xFF).len(), 8);

    // Grid: the nearest multiple of 8.
    assert_eq!(snap_to_grid(Point::new(3, 4)), Point::new(0, 8));
    assert_eq!(snap_to_grid(Point::new(12, 11)), Point::new(16, 8));
    assert_eq!(snap_to_grid(Point::new(-3, -5)), Point::new(0, -8));
}

#[test]
fn or_mode() {
    // Or mode paints only the pattern's black pixels: a 50% gray over black
    // leaves black, over white it's the gray.
    let mut canvas = Canvas::new(16, 8);
    canvas.fill_rect(0, 0, 7, 7, BLACK_PATTERN);
    canvas.set_paint_mode(PaintMode::Or);
    canvas.fill_rect(0, 0, 15, 7, CHECKER);
    assert_eq!(count_black(&canvas), 64 + 32);
    canvas.fill_shape(Shape::Rect, Rect::new(0, 0, 16, 8), WHITE_PATTERN);
    assert_eq!(count_black(&canvas), 64 + 32);

    // Copy mode, the default, replaces.
    canvas.set_paint_mode(PaintMode::Copy);
    canvas.fill_rect(0, 0, 15, 7, CHECKER);
    assert_eq!(count_black(&canvas), 64);

    // Brush stamps follow the mode too, like MacPaint's DrawBrush in Or
    // mode.
    let mut stamped = Canvas::new(16, 16);
    stamped.fill_rect(0, 0, 15, 15, BLACK_PATTERN);
    stamped.set_paint_mode(PaintMode::Or);
    stamped.stamp_mask(&FULL_MASK, 8, 8, WHITE_PATTERN);
    assert_eq!(count_black(&stamped), 256);
}

#[test]
fn dither() {
    const SIZE: i32 = 64;
    let pixels = (SIZE * SIZE) as usize;
    let mut canvas = Canvas::new(SIZE, SIZE);

    // Solid black and white are exact.
    dither_gray(&vec![0; pixels], SIZE, SIZE, &mut canvas);
    assert_eq!(count_black(&canvas), pixels);
    dither_gray(&vec![255; pixels], SIZE, SIZE, &mut canvas);
    assert_eq!(count_black(&canvas), 0);

    // Grays come out in proportion.
    dither_gray(&vec![128; pixels], SIZE, SIZE, &mut canvas);
    let half = count_black(&canvas);
    assert!(
        half > pixels * 45 / 100 && half < pixels * 55 / 100,
        "half {half}"
    );
    dither_gray(&vec![191; pixels], SIZE, SIZE, &mut canvas);
    let quarter = count_black(&canvas);
    assert!(
        quarter > pixels * 20 / 100 && quarter < pixels * 30 / 100,
        "quarter {quarter}"
    );

    // Line art passes through unchanged.
    let mut art = vec![255; pixels];
    for i in 0..SIZE as usize {
        art[i * SIZE as usize + i] = 0;
    }
    dither_gray(&art, SIZE, SIZE, &mut canvas);
    assert!(count_black(&canvas) == SIZE as usize && canvas.get(10, 10) && !canvas.get(11, 10));

    // Cropped to the canvas, the rest cleared.
    let mut small = Canvas::new(10, 10);
    small.fill_rect(0, 0, 9, 9, BLACK_PATTERN);
    dither_gray(&[0; 4 * 40], 4, 40, &mut small);
    assert!(count_black(&small) == 4 * 10 && !small.get(4, 0));
}

/// Where needle first occurs in haystack, from start.
fn find(haystack: &[u8], needle: &[u8], start: usize) -> usize {
    start
        + haystack[start..]
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("found")
}

#[test]
fn pdf_pages() {
    use rustpaint_engine::macpaint_file::unpack_bits;
    use rustpaint_engine::pdf::write_pdf;

    let mut first = Canvas::default();
    first.fill_rect(0, 0, 7, 0, BLACK_PATTERN);
    let pdf = write_pdf(&[first, Canvas::default()]);
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.starts_with("%PDF-1.4") && text.ends_with("%%EOF\n"));
    assert!(text.contains("/Count 2") && text.matches("/Type /Page ").count() == 2);
    // 576 x 720 points, centered on Letter paper.
    assert!(text.contains("q 576.00 0 0 720.00 18.00 36.00 cm /Im0 Do Q"));

    // Every object is where the cross-reference table says.
    let startxref = find(&pdf, b"startxref\n", 0) + 10;
    let end = find(&pdf, b"\n", startxref);
    let xref: usize = std::str::from_utf8(&pdf[startxref..end])
        .unwrap()
        .parse()
        .unwrap();
    let table = std::str::from_utf8(&pdf[xref..]).unwrap();
    for (number, line) in table.lines().skip(3).take(8).enumerate() {
        let offset: usize = line[..10].parse().unwrap();
        assert!(pdf[offset..].starts_with(format!("{} 0 obj", number + 1).as_bytes()));
    }

    // The first image's data unpacks to the page, black bits first.
    let image = find(&pdf, b"/RunLengthDecode", 0);
    let mut position = find(&pdf, b"stream\n", image) + 7;
    let mut bits = vec![0; 72 * 720];
    assert!(unpack_bits(&pdf, &mut position, &mut bits));
    assert_eq!(bits[0], 0xff);
    assert!(bits[1..].iter().all(|&b| b == 0));
    assert_eq!(pdf[position], 128);
}
