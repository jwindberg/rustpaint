//! Reading and writing documents, as HaikuPaint's DocumentIO does.
//! RustPaint's own format is MacPaint's; other images open converted to
//! black and white.

use std::path::Path;

use image::ImageFormat;
use rustpaint_engine::macpaint_file::{
    MacPaintError, macpaint_data_offset, read_macpaint, write_macpaint,
};
use rustpaint_engine::patterns::{PATTERN_COUNT, PATTERNS};
use rustpaint_engine::{Canvas, Pattern};

use crate::i18n::{tr, tr_args};
use crate::images::dither_rgba;

/// The file name extensions MacPaint documents go by.
pub const MACPAINT_EXTENSIONS: [&str; 3] = ["mac", "pntg", "pnt"];

/// Larger than any real MacPaint file, which is under 60 KB.
const MAX_MACPAINT_FILE_SIZE: usize = 1024 * 1024;

pub struct Document {
    /// 576 x 720.
    pub canvas: Canvas,
    pub patterns: [Pattern; PATTERN_COUNT],
    /// Whether it was a MacPaint document, so saving may overwrite it.
    pub is_macpaint: bool,
}

/// Whether the file's name or MacBinary header says MacPaint.
fn looks_like_macpaint(path: &Path, data: &[u8]) -> bool {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    MACPAINT_EXTENSIONS.contains(&extension.as_str()) || macpaint_data_offset(data) != 0
}

/// A quick check, without reading the whole file: whether its name or
/// MacBinary header says it's a MacPaint document.
pub fn is_macpaint_document(path: &Path) -> bool {
    use std::io::Read;
    // Enough for a MacBinary header and the MacPaint header after it.
    let mut start = vec![0; 128 + 512];
    let read = std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut start))
        .unwrap_or(0);
    start.truncate(read);
    looks_like_macpaint(path, &start)
}

/// Loads a document. MacPaint files are read as they are (with the
/// standard patterns if they have none of their own); any other image is
/// converted to black and white and placed at the top left, cropped to
/// fit, with the standard patterns.
pub fn load_document(path: &Path) -> Result<Document, String> {
    let data = std::fs::read(path)
        .map_err(|error| tr_args("error-open", &[("error", &error.to_string())]))?;

    let macpaint = |data: &[u8]| {
        read_macpaint(data).map(|document| Document {
            canvas: document.canvas,
            patterns: document.patterns.unwrap_or(PATTERNS),
            is_macpaint: true,
        })
    };

    let mut reason = None;
    let small = data.len() <= MAX_MACPAINT_FILE_SIZE;
    if small && looks_like_macpaint(path, &data) {
        match macpaint(&data) {
            Ok(document) => return Ok(document),
            Err(error) => reason = Some(macpaint_error(error)),
        }
    }

    if let Ok(image) = image::load_from_memory(&data) {
        let image = image.to_rgba8();
        let mut canvas = Canvas::default();
        let (width, height) = (image.width() as usize, image.height() as usize);
        if dither_rgba(image.as_raw(), width, height, &mut canvas) {
            return Ok(Document {
                canvas,
                patterns: PATTERNS,
                is_macpaint: false,
            });
        }
    }

    // Files that lost their name, e.g. copied off an old disk.
    if reason.is_none()
        && small
        && let Ok(document) = macpaint(&data)
    {
        return Ok(document);
    }

    Err(reason.unwrap_or_else(|| tr("error-unreadable")))
}

/// Writes the image and its patterns as a MacPaint document, creating or
/// replacing the file.
pub fn save_macpaint(
    path: &Path,
    canvas: &Canvas,
    patterns: &[Pattern; PATTERN_COUNT],
) -> Result<(), String> {
    std::fs::write(path, write_macpaint(canvas, patterns))
        .map_err(|error| tr_args("error-save", &[("error", &error.to_string())]))
}

fn export_error(reason: &str) -> String {
    tr_args("error-export", &[("error", reason)])
}

/// The engine's reason a MacPaint document can't be read, translated.
fn macpaint_error(error: MacPaintError) -> String {
    tr(match error {
        MacPaintError::TooShort => "error-too-short",
        MacPaintError::NotMacPaint => "error-not-macpaint",
        MacPaintError::Damaged => "error-damaged",
    })
}

/// Writes the image as a black and white PNG.
pub fn export_png(path: &Path, canvas: &Canvas) -> Result<(), String> {
    let gray = canvas
        .pixels()
        .iter()
        .map(|&black| if black != 0 { 0 } else { 255 })
        .collect();
    let image = image::GrayImage::from_raw(canvas.width() as u32, canvas.height() as u32, gray)
        .ok_or_else(|| export_error("the page has no pixels"))?;
    image
        .save_with_format(path, ImageFormat::Png)
        .map_err(|error| export_error(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::BLACK_PATTERN;

    /// A fresh directory for one test's files.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustpaint-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn drawing() -> Canvas {
        let mut canvas = Canvas::default();
        canvas.fill_rect(10, 10, 99, 49, BLACK_PATTERN);
        canvas
    }

    #[test]
    fn macpaint_documents_round_trip_with_their_patterns() {
        let dir = scratch("macpaint");
        let path = dir.join("drawing.mac");
        let mut patterns = PATTERNS;
        patterns[3] = Pattern::new([1, 2, 3, 4, 5, 6, 7, 8]);
        save_macpaint(&path, &drawing(), &patterns).unwrap();

        let document = load_document(&path).unwrap();
        assert!(document.is_macpaint);
        assert_eq!(document.canvas.pixels(), drawing().pixels());
        assert_eq!(document.patterns, patterns);

        // Without its extension it's still recognized, by its contents.
        let renamed = dir.join("drawing");
        std::fs::rename(&path, &renamed).unwrap();
        assert!(load_document(&renamed).unwrap().is_macpaint);
    }

    #[test]
    fn png_exports_and_opens_as_an_image() {
        let dir = scratch("png");
        let path = dir.join("drawing.png");
        export_png(&path, &drawing()).unwrap();

        let document = load_document(&path).unwrap();
        assert!(!document.is_macpaint);
        assert_eq!(document.canvas.pixels(), drawing().pixels());
        assert_eq!(document.patterns, PATTERNS);
    }

    #[test]
    fn unreadable_files_say_why() {
        let dir = scratch("bad");
        let damaged = dir.join("damaged.mac");
        std::fs::write(&damaged, vec![0; 600]).unwrap();
        assert_eq!(
            load_document(&damaged).err().unwrap(),
            "The MacPaint document is damaged or incomplete."
        );

        let text = dir.join("notes.txt");
        std::fs::write(&text, "hello").unwrap();
        assert_eq!(
            load_document(&text).err().unwrap(),
            "RustPaint can't read this kind of file."
        );
        assert!(
            load_document(&dir.join("missing.mac"))
                .err()
                .unwrap()
                .starts_with("Couldn't open")
        );
    }
}
