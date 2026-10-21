//! MacPaint's Print Catalog, after HaikuPaint's: every MacPaint document
//! in a folder as a framed eighth-size picture with its name, 5 across and
//! 5 down on each 576 x 720 page, under a heading.

use std::path::Path;

use rustpaint_engine::canvas::reduced;
use rustpaint_engine::{Canvas, Point};

use crate::art::frame_qd_rect;
use crate::documents::{is_macpaint_document, load_document};
use crate::typesetter::{Alignment, TextStyle, Typesetter};

// From MacPaint's PrintCatalog and GetCatPic: bands of 144 pixels, five to
// a page, each with five pictures 72 x 90 at 112 pixel steps from x 28,
// 32 pixels down, with the name centered 16 pixels below.
const PER_ROW: usize = 5;
const ROWS_PER_PAGE: usize = 5;
const BAND_HEIGHT: i32 = 144;
const REDUCTION: i32 = 8;

/// Draws text in black with its first baseline at `at`.
fn draw_text(page: &mut Canvas, typesetter: &Typesetter, style: &TextStyle, text: &str, at: Point) {
    let layout = typesetter.render(&[text.to_string()], style, at, false);
    for y in 0..layout.mask.height() {
        for x in 0..layout.mask.width() {
            if layout.mask.get(x, y) {
                page.set(layout.bounds.left + x, layout.bounds.top + y, true);
            }
        }
    }
}

/// The catalog's pages, none if the folder has no MacPaint documents.
pub fn catalog_pages(folder: &Path, heading: &str, typesetter: &Typesetter) -> Vec<Canvas> {
    // The folder's MacPaint documents, by name.
    let mut paths: Vec<_> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_macpaint_document(path))
        .collect();
    paths.sort_by_key(|path| path.file_name().map(|n| n.to_string_lossy().to_lowercase()));

    let style = typesetter.default_style();
    let centered = TextStyle {
        alignment: Alignment::Middle,
        ..style.clone()
    };
    let mut pages: Vec<Canvas> = Vec::new();
    let mut slot = 0;
    for path in paths {
        let Ok(document) = load_document(&path) else {
            continue;
        };
        if !document.is_macpaint {
            continue;
        }
        let on_page = slot % (PER_ROW * ROWS_PER_PAGE);
        if on_page == 0 {
            let mut page = Canvas::default();
            draw_text(&mut page, typesetter, &style, heading, Point::new(28, 10));
            pages.push(page);
        }
        let page = pages.last_mut().expect("a page");
        let left = 28 + 112 * (on_page % PER_ROW) as i32;
        let top = (on_page / PER_ROW) as i32 * BAND_HEIGHT + 32;
        let small = reduced(&document.canvas, REDUCTION, REDUCTION * REDUCTION / 4);
        for y in 0..small.height() {
            for x in 0..small.width() {
                page.set(left + x, top + y, small.get(x, y));
            }
        }
        frame_qd_rect(page, left, top, left + small.width(), top + small.height());
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let below = Point::new(left + small.width() / 2, top + small.height() + 16);
        draw_text(page, typesetter, &centered, &name, below);
        slot += 1;
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::save_macpaint;
    use rustpaint_engine::BLACK_PATTERN;
    use rustpaint_engine::patterns::PATTERNS;

    #[test]
    fn documents_go_five_by_five_with_their_names() {
        let dir = std::env::temp_dir().join(format!("rustpaint-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut drawing = Canvas::default();
        drawing.fill_rect(0, 0, 575, 719, BLACK_PATTERN);
        for i in 0..27 {
            save_macpaint(
                &dir.join(format!("drawing {i:02}.mac")),
                &drawing,
                &PATTERNS,
            )
            .unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "not a picture").unwrap();

        let typesetter = Typesetter::from_data(crate::text::INTER);
        let pages = catalog_pages(&dir, "Catalog", &typesetter);
        assert_eq!(pages.len(), 2);
        // The first picture: black, 72 x 90, framed, at (28, 32).
        assert!(pages[0].get(28, 32) && pages[0].get(60, 80) && pages[0].get(99, 121));
        assert!(!pages[0].get(100, 80));
        // Names below the pictures, and the heading at the top.
        assert!((136..140).any(|y| (40..90).any(|x| pages[0].get(x, y))));
        assert!((0..12).any(|y| (28..80).any(|x| pages[0].get(x, y))));
        // Two left over on the second page.
        assert!(pages[1].get(28 + 112, 32) && !pages[1].get(28 + 224, 40));

        assert!(catalog_pages(&dir.join("missing"), "Catalog", &typesetter).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
