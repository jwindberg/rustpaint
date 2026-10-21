//! MacPaint documents (Mac file type 'PNTG'), laid out the way MacPaint's
//! ReadDoc and BlankDoc read and write them:
//!   - a 512-byte header: version (4 bytes, big endian; 2 and up carry
//!     patterns), the document's 38 patterns (304 bytes), then zeros;
//!   - 720 scanlines of 72 bytes (576 pixels, 1 = black), each compressed
//!     separately with PackBits.
//!
//! Files copied off a Mac often carry a 128-byte MacBinary header first;
//! reading skips it.

use std::fmt;

use crate::canvas::{Canvas, Pattern};
use crate::patterns::PATTERN_COUNT;

pub const MACPAINT_WIDTH: i32 = 576;
pub const MACPAINT_HEIGHT: i32 = 720;
pub const MACPAINT_ROW_BYTES: usize = MACPAINT_WIDTH as usize / 8;
pub const MACPAINT_HEADER_SIZE: usize = 512;

const MACBINARY_HEADER_SIZE: usize = 128;

/// Why data isn't a usable MacPaint document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacPaintError {
    TooShort,
    NotMacPaint,
    Damaged,
}

impl fmt::Display for MacPaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooShort => "The file is too short to be a MacPaint document.",
            Self::NotMacPaint => "This doesn't look like a MacPaint document.",
            Self::Damaged => "The MacPaint document is damaged or incomplete.",
        })
    }
}

impl std::error::Error for MacPaintError {}

/// A MacPaint document as read from a file.
#[derive(Clone, Debug)]
pub struct MacPaintDocument {
    /// 576 x 720.
    pub canvas: Canvas,
    /// The document's own 38 patterns; version 2 and up have them.
    pub patterns: Option<[Pattern; PATTERN_COUNT]>,
}

/// Reads a MacPaint document, with or without a MacBinary header.
pub fn read_macpaint(data: &[u8]) -> Result<MacPaintDocument, MacPaintError> {
    let offset = macpaint_data_offset(data);
    if data.len() < offset + MACPAINT_HEADER_SIZE {
        return Err(MacPaintError::TooShort);
    }

    // The version is 0 to 3 in real files; anything else means this is
    // some other kind of file.
    let version = u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    if version > 3 {
        return Err(MacPaintError::NotMacPaint);
    }

    // The scanlines run continuously; MacPaint packs each one separately,
    // but some writers didn't, and unpacking straight through handles both.
    let mut bits = vec![0; MACPAINT_ROW_BYTES * MACPAINT_HEIGHT as usize];
    let mut position = offset + MACPAINT_HEADER_SIZE;
    if !unpack_bits(data, &mut position, &mut bits) {
        return Err(MacPaintError::Damaged);
    }

    let mut canvas = Canvas::new(MACPAINT_WIDTH, MACPAINT_HEIGHT);
    for y in 0..MACPAINT_HEIGHT {
        for x in 0..MACPAINT_WIDTH {
            let byte = bits[y as usize * MACPAINT_ROW_BYTES + x as usize / 8];
            canvas.set(x, y, (byte >> (7 - x % 8)) & 1 != 0);
        }
    }

    // As in ReadDoc, versions before 2 are the old format, without
    // patterns.
    let patterns = (version >= 2).then(|| {
        std::array::from_fn(|i| {
            let start = offset + 4 + i * 8;
            let mut rows = [0; 8];
            rows.copy_from_slice(&data[start..start + 8]);
            Pattern::new(rows)
        })
    });
    Ok(MacPaintDocument { canvas, patterns })
}

/// Encodes canvas (576 x 720) as a version 2 MacPaint document with the
/// given 38 patterns.
pub fn write_macpaint(canvas: &Canvas, patterns: &[Pattern; PATTERN_COUNT]) -> Vec<u8> {
    let mut out = vec![0; MACPAINT_HEADER_SIZE];
    out[3] = 2; // version 2: has patterns
    for (i, pattern) in patterns.iter().enumerate() {
        out[4 + i * 8..12 + i * 8].copy_from_slice(&pattern.rows);
    }

    let mut row = [0u8; MACPAINT_ROW_BYTES];
    for y in 0..MACPAINT_HEIGHT {
        row.fill(0);
        for x in 0..MACPAINT_WIDTH.min(canvas.width()) {
            if canvas.get(x, y) {
                row[x as usize / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend(pack_bits(&row));
    }
    out
}

/// Where the MacPaint data starts: 128 for a MacBinary file of type PNTG,
/// otherwise 0.
pub fn macpaint_data_offset(data: &[u8]) -> usize {
    // MacBinary: byte 0 is zero, byte 1 the file name length (1-63), and
    // bytes 65-68 the Mac file type.
    if data.len() >= MACBINARY_HEADER_SIZE + MACPAINT_HEADER_SIZE
        && data[0] == 0
        && (1..=63).contains(&data[1])
        && &data[65..69] == b"PNTG"
    {
        MACBINARY_HEADER_SIZE
    } else {
        0
    }
}

/// Apple's PackBits.
pub fn pack_bits(data: &[u8]) -> Vec<u8> {
    let size = data.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < size {
        // Length of the run of identical bytes starting here.
        let mut run = 1;
        while i + run < size && run < 128 && data[i + run] == data[i] {
            run += 1;
        }

        if run >= 2 {
            // Repeat: -(count - 1), then the byte.
            out.push((1 - run as i32) as u8);
            out.push(data[i]);
            i += run;
            continue;
        }

        // Literal: count - 1, then the bytes, up to the next run of two.
        let start = i;
        while i < size && i - start < 128 {
            if i + 1 < size && data[i + 1] == data[i] {
                break;
            }
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&data[start..i]);
    }
    out
}

/// Unpacks into exactly `out.len()` bytes from data, starting at *position
/// and advancing it. Returns false if the data ends early.
pub fn unpack_bits(data: &[u8], position: &mut usize, out: &mut [u8]) -> bool {
    let mut written = 0;
    while written < out.len() {
        if *position >= data.len() {
            return false;
        }
        let flag = data[*position] as i8;
        *position += 1;
        if flag == -128 {
            continue;
        }

        if flag >= 0 {
            let count = flag as usize + 1;
            if *position + count > data.len() {
                return false;
            }
            for &byte in &data[*position..*position + count] {
                if written == out.len() {
                    break;
                }
                out[written] = byte;
                written += 1;
            }
            *position += count;
        } else {
            let count = (1 - flag as i32) as usize;
            if *position >= data.len() {
                return false;
            }
            let value = data[*position];
            *position += 1;
            for _ in 0..count {
                if written == out.len() {
                    break;
                }
                out[written] = value;
                written += 1;
            }
        }
    }
    true
}
