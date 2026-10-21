//! Smooth text for the drawn menu bar, as PhotoCraft draws its own: Inter,
//! anti-aliased at the screen's full resolution. The page and palettes
//! stay 1-bit.

use std::cell::RefCell;
use std::collections::HashMap;

pub static INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

struct Glyph {
    left: i32,
    /// From the baseline up to the bitmap's top row.
    top: i32,
    width: usize,
    height: usize,
    coverage: Vec<u8>,
}

pub struct TextFont {
    font: fontdue::Font,
    /// In logical pixels.
    size: f32,
    /// Rasterized glyphs, by character and scale.
    glyphs: RefCell<HashMap<(char, u32), Glyph>>,
}

impl TextFont {
    /// The font in data at a size in logical pixels. Panics on a bad font,
    /// which can only be a bundled one.
    pub fn new(data: &[u8], size: f32) -> Self {
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .expect("a bundled font");
        Self {
            font,
            size,
            glyphs: RefCell::new(HashMap::new()),
        }
    }

    fn advance(&self, c: char, scale: u32) -> f32 {
        self.font.metrics(c, self.size * scale as f32).advance_width
    }

    /// The width text takes, in logical pixels.
    pub fn width(&self, text: &str) -> i32 {
        text.chars().map(|c| self.advance(c, 1)).sum::<f32>().ceil() as i32
    }

    /// Draws text into a buffer of physical pixels (0RGB, `scale` physical
    /// pixels per logical one), blending color over what's there. The
    /// baseline starts at logical (x, baseline).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        scale: u32,
        x: i32,
        baseline: i32,
        text: &str,
        color: u32,
    ) {
        let mut pen = (x * scale as i32) as f32;
        let baseline = baseline * scale as i32;
        let mut glyphs = self.glyphs.borrow_mut();
        for c in text.chars() {
            let glyph = glyphs.entry((c, scale)).or_insert_with(|| {
                let (metrics, coverage) = self.font.rasterize(c, self.size * scale as f32);
                Glyph {
                    left: metrics.xmin,
                    top: metrics.ymin + metrics.height as i32,
                    width: metrics.width,
                    height: metrics.height,
                    coverage,
                }
            });
            let left = pen.round() as i32 + glyph.left;
            let top = baseline - glyph.top;
            for gy in 0..glyph.height {
                let y = top + gy as i32;
                if y < 0 || y as usize >= height {
                    continue;
                }
                for gx in 0..glyph.width {
                    let x = left + gx as i32;
                    let alpha = glyph.coverage[gy * glyph.width + gx];
                    if alpha == 0 || x < 0 || x as usize >= width {
                        continue;
                    }
                    let pixel = &mut buffer[y as usize * width + x as usize];
                    *pixel = blend(*pixel, color, alpha);
                }
            }
            pen += self.advance(c, scale);
        }
    }
}

/// color over background, alpha of 255.
fn blend(background: u32, color: u32, alpha: u8) -> u32 {
    let alpha = alpha as u32;
    let channel = |shift: u32| {
        let b = (background >> shift) & 0xff;
        let c = (color >> shift) & 0xff;
        ((c * alpha + b * (255 - alpha) + 127) / 255) << shift
    };
    channel(16) | channel(8) | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_measured_and_blended_in() {
        let font = TextFont::new(INTER, 13.0);
        let width = font.width("File");
        assert!((15..30).contains(&width), "width {width}");
        assert_eq!(font.width(""), 0);

        // Black text on white, at twice the resolution: dark pixels, some
        // in between, nothing left of the pen or below the baseline.
        let (w, h) = (80, 40);
        let mut buffer = vec![0xffffff; w * h];
        font.draw(&mut buffer, w, h, 2, 2, 14, "File", 0x000000);
        assert!(buffer.contains(&0x000000));
        assert!(buffer.iter().any(|&p| p != 0 && p != 0xffffff));
        assert!((0..h).all(|y| buffer[y * w] == 0xffffff));
        assert!((0..w).all(|x| buffer[30 * w + x] == 0xffffff));
    }

    #[test]
    fn blending_mixes_by_coverage() {
        assert_eq!(blend(0xffffff, 0x000000, 255), 0x000000);
        assert_eq!(blend(0xffffff, 0x000000, 0), 0xffffff);
        assert_eq!(blend(0x000000, 0xffffff, 128), 0x808080);
    }
}
