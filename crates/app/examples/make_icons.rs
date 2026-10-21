//! Writes RustPaint's application icon, MacPaint's own (ICN# 128), as PNGs
//! for packaging: icon_<size>.png for 16 to 1024 pixels, scaled up in
//! whole pixels, transparent outside the icon's mask.
//!
//! Usage: cargo run -p rustpaint --example make_icons -- <output folder>

use rustpaint_engine::mac_icons::{APP_ICON, APP_ICON_MASK, MAC_ICON_SIZE};

fn bit(data: &[u8], x: usize, y: usize) -> bool {
    (data[y * MAC_ICON_SIZE / 8 + x / 8] >> (7 - x % 8)) & 1 != 0
}

fn main() {
    let folder = std::env::args().nth(1).expect("an output folder");
    std::fs::create_dir_all(&folder).expect("creating the folder");
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let image = image::RgbaImage::from_fn(size, size, |x, y| {
            // Nearest neighbor, so MacPaint's pixels stay square.
            let (ix, iy) = (
                x as usize * MAC_ICON_SIZE / size as usize,
                y as usize * MAC_ICON_SIZE / size as usize,
            );
            if !bit(&APP_ICON_MASK, ix, iy) {
                image::Rgba([0, 0, 0, 0])
            } else if bit(&APP_ICON, ix, iy) {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        });
        let path = format!("{folder}/icon_{size}.png");
        image.save(&path).expect("writing an icon");
    }
}
