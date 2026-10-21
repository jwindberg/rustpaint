//! The clipboard. Other apps get a plain image, with unselected lasso
//! pixels white. RustPaint also keeps the selection itself, so a lasso
//! selection pastes back transparent, as long as the image on the system
//! clipboard is still the one it put there (HaikuPaint keeps its own
//! clipboard format for the same reason).

use std::borrow::Cow;

use rustpaint_engine::dither::dither_gray;
use rustpaint_engine::{Canvas, Rect, Selection};

pub struct Clipboard {
    system: Option<arboard::Clipboard>,
    /// What was last copied, and the image that went with it.
    own: Option<(Selection, Vec<u8>)>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            system: arboard::Clipboard::new().ok(),
            own: None,
        }
    }

    pub fn copy(&mut self, selection: &Selection) {
        let rgba = selection_rgba(selection);
        if let Some(system) = &mut self.system {
            let image = arboard::ImageData {
                width: selection.rect.width() as usize,
                height: selection.rect.height() as usize,
                bytes: Cow::Borrowed(&rgba),
            };
            // Copying still works inside RustPaint if the system refuses.
            let _ = system.set_image(image);
        }
        self.own = Some((selection.clone(), rgba));
    }

    /// What to paste, placed at (0, 0), cropped to fit a page of the given
    /// size.
    pub fn paste(&mut self, page_width: i32, page_height: i32) -> Option<Selection> {
        let Some(system) = &mut self.system else {
            return self.own.as_ref().map(|(selection, _)| selection.clone());
        };
        let image = system.get_image().ok()?;
        if let Some((selection, rgba)) = &self.own
            && *rgba == *image.bytes
            && image.width == selection.rect.width() as usize
        {
            let mut selection = selection.clone();
            selection.rect = Rect::new(0, 0, selection.rect.width(), selection.rect.height());
            return Some(selection);
        }
        let pixels = canvas_from_rgba(
            &image.bytes,
            image.width,
            image.height,
            page_width,
            page_height,
        )?;
        Some(Selection::from_rect(
            &pixels,
            Rect::new(0, 0, pixels.width(), pixels.height()),
        ))
    }
}

/// The selection as an RGBA image, unselected pixels white.
fn selection_rgba(selection: &Selection) -> Vec<u8> {
    let (width, height) = (selection.rect.width(), selection.rect.height());
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let black = selection.mask.get(x, y) && selection.pixels.get(x, y);
            let value = if black { 0 } else { 255 };
            rgba.extend([value, value, value, 255]);
        }
    }
    rgba
}

/// Converts an RGBA image to black and white, cropped to the page:
/// transparent counts as white, and grays are dithered. None if the image
/// is empty or its data is short.
fn canvas_from_rgba(
    rgba: &[u8],
    width: usize,
    height: usize,
    page_width: i32,
    page_height: i32,
) -> Option<Canvas> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return None;
    }
    // Only the part that fits matters.
    let fit_width = width.min(page_width as usize);
    let fit_height = height.min(page_height as usize);
    let mut gray = Vec::with_capacity(fit_width * fit_height);
    for y in 0..fit_height {
        for x in 0..fit_width {
            let pixel = &rgba[(y * width + x) * 4..][..4];
            let luminance =
                (299 * pixel[0] as u32 + 587 * pixel[1] as u32 + 114 * pixel[2] as u32) / 1000;
            gray.push(if pixel[3] >= 128 {
                luminance as u8
            } else {
                255
            });
        }
    }
    let mut canvas = Canvas::new(fit_width as i32, fit_height as i32);
    dither_gray(&gray, fit_width as i32, fit_height as i32, &mut canvas);
    Some(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::BLACK_PATTERN;

    #[test]
    fn lasso_pixels_outside_the_mask_go_white() {
        let mut page = Canvas::new(20, 20);
        page.fill_rect(0, 0, 19, 19, BLACK_PATTERN);
        let mut selection = Selection::from_rect(&page, Rect::new(0, 0, 2, 1));
        selection.mask.set(1, 0, false);
        assert_eq!(
            selection_rgba(&selection),
            [0, 0, 0, 255, 255, 255, 255, 255]
        );
    }

    #[test]
    fn foreign_images_are_dithered_and_cropped() {
        // 3 x 1: black, transparent, white.
        let rgba = [0, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 255];
        let canvas = canvas_from_rgba(&rgba, 3, 1, 576, 720).unwrap();
        assert!(canvas.get(0, 0) && !canvas.get(1, 0) && !canvas.get(2, 0));

        let cropped = canvas_from_rgba(&rgba, 3, 1, 2, 720).unwrap();
        assert_eq!(cropped.width(), 2);
        assert!(canvas_from_rgba(&rgba[..8], 3, 1, 576, 720).is_none());
    }
}
