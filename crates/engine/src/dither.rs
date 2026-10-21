//! Dithered import: grayscale to black and white.

use crate::canvas::Canvas;

/// Converts a grayscale image (0 black ... 255 white, width * height bytes,
/// row by row) to black and white with Floyd-Steinberg error diffusion,
/// scanning alternate rows in opposite directions. Pure black and white
/// pass through unchanged, so line art stays crisp. The result goes in the
/// top left of canvas, cropped to it; the rest of canvas is cleared.
pub fn dither_gray(gray: &[u8], width: i32, height: i32, canvas: &mut Canvas) {
    canvas.clear();
    if width <= 0 || height <= 0 || gray.len() < width as usize * height as usize {
        return;
    }

    // Work in 1/16ths so the 7, 3, 5 and 1 weights stay integers. Two rows
    // of error, with a pixel of margin on each side.
    let mut current = vec![0i32; width as usize + 2];
    let mut next = vec![0i32; width as usize + 2];

    for y in 0..height {
        let left_to_right = y % 2 == 0;
        let step = if left_to_right { 1 } else { -1 };
        next.fill(0);

        for i in 0..width {
            let x = if left_to_right { i } else { width - 1 - i };
            let at = |offset: i32| (x + 1 + offset) as usize;
            let value = gray[(y * width + x) as usize] as i32 * 16 + current[at(0)];
            let black = value < 128 * 16;
            let error = value - if black { 0 } else { 255 * 16 };
            if x < canvas.width() && y < canvas.height() {
                canvas.set(x, y, black);
            }

            // Spread the error ahead on this row and onto the next one.
            current[at(step)] += error * 7 / 16;
            next[at(-step)] += error * 3 / 16;
            next[at(0)] += error * 5 / 16;
            next[at(step)] += error / 16;
        }
        std::mem::swap(&mut current, &mut next);
    }
}
