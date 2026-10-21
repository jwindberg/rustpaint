//! Bringing color images into MacPaint's black and white, for opening
//! other formats and pasting from other apps.

use rustpaint_engine::Canvas;
use rustpaint_engine::dither::dither_gray;

/// Converts an RGBA image to black and white into the top left of canvas,
/// clearing the rest: transparent counts as white, and grays are dithered.
/// Cropped to the canvas. Returns false if the data is short.
pub fn dither_rgba(rgba: &[u8], width: usize, height: usize, canvas: &mut Canvas) -> bool {
    if rgba.len() < width * height * 4 {
        return false;
    }
    // Only the part that fits matters.
    let fit_width = width.min(canvas.width().max(0) as usize);
    let fit_height = height.min(canvas.height().max(0) as usize);
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
    dither_gray(&gray, fit_width as i32, fit_height as i32, canvas);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_is_white_and_the_rest_is_cleared() {
        // 3 x 1: black, transparent, white.
        let rgba = [0, 0, 0, 255, 0, 0, 0, 0, 255, 255, 255, 255];
        let mut canvas = Canvas::new(4, 2);
        canvas.set(3, 1, true);
        assert!(dither_rgba(&rgba, 3, 1, &mut canvas));
        assert!(canvas.get(0, 0) && !canvas.get(1, 0) && !canvas.get(2, 0));
        assert!(!canvas.get(3, 1));

        // Cropped, and short data refused.
        let mut small = Canvas::new(2, 1);
        assert!(dither_rgba(&rgba, 3, 1, &mut small) && small.get(0, 0));
        assert!(!dither_rgba(&rgba[..8], 3, 1, &mut small));
    }
}
