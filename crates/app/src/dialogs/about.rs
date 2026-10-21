//! About RustPaint, after MacPaint's About box (DLOG 2006): three lines of
//! text at the left and the "BILL ICON", Bill Atkinson's portrait, at
//! (290, 16). OK goes below the text, which is wider than MacPaint's.

use rustpaint_engine::Canvas;
use rustpaint_engine::mac_icons::{ABOUT_ICON, MAC_ICON_SIZE};

use super::{Button, Outcome};
use crate::i18n::{tr, tr_args};

pub const SIZE: (i32, i32) = (356, 112);

pub fn buttons() -> Vec<Button> {
    vec![Button::new(274, 84, 346, "ok", true)]
}

pub fn button(_index: usize) -> Outcome {
    Outcome::Cancelled
}

/// The text, as (x, baseline) and line.
pub fn lines() -> Vec<(i32, i32, String)> {
    [
        tr_args("about-version", &[("version", env!("CARGO_PKG_VERSION"))]),
        tr("about-credit"),
        tr("about-copyright"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, line)| (6, 14 + i as i32 * 24 + 11, line))
    .collect()
}

pub fn render(canvas: &mut Canvas) {
    let size = MAC_ICON_SIZE as i32;
    for y in 0..size {
        for x in 0..size {
            let byte = ABOUT_ICON[(y * size / 8 + x / 8) as usize];
            if (byte >> (7 - x % 8)) & 1 != 0 {
                canvas.set(290 + x, 16 + y, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bill_icon_and_the_credits() {
        let mut canvas = Canvas::new(SIZE.0, SIZE.1);
        render(&mut canvas);
        let black: usize = canvas.pixels().iter().map(|&p| p as usize).sum();
        assert!(black > 100);
        assert!((16..48).all(|y| (0..290).all(|x| !canvas.get(x, y))));
        assert!(lines()[1].2.contains("Bill Atkinson"));
    }
}
