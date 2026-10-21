//! The text tool's typesetting, after HaikuPaint's TextRenderer: lines of
//! text in one of the system's fonts, laid out as MacPaint's UpdateText
//! does and drawn one bit deep. Glyphs are hinted, so they fit the pixel
//! grid, before they're cut to black and white.

use std::cell::RefCell;

use fontdb::{Database, Family, Query, Stretch, Style, Weight};
use rustpaint_engine::geometry::trunc_to_grid;
use rustpaint_engine::{BLACK_PATTERN, Canvas, Point, Rect, Selection};
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Angle, Format, Transform};

/// Coverage at or above this (of 255) makes a pixel black. Lower than half
/// keeps thin strokes of small sizes from breaking up.
const THRESHOLD: u8 = 96;

/// MacPaint's FontSize menu, in points (a point is a pixel, at 72 dpi).
pub const FONT_SIZES: [u32; 9] = [9, 10, 12, 14, 18, 24, 36, 48, 72];

/// Families to start with, the first one installed: classic Mac faces,
/// then common sans serifs on each platform.
const PREFERRED_FAMILIES: [&str; 8] = [
    "Geneva",
    "Helvetica",
    "Arial",
    "DejaVu Sans",
    "Liberation Sans",
    "Segoe UI",
    "Noto Sans",
    "Inter",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Alignment {
    #[default]
    Left,
    Middle,
    Right,
}

/// MacPaint's Font, FontSize and Style menus, applied to the text tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextStyle {
    pub family: String,
    pub size: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub outline: bool,
    pub shadow: bool,
    pub alignment: Alignment,
}

/// A block of text as MacPaint's UpdateText draws it: line by line from the
/// click point, which is the first baseline, aligned around it
/// horizontally.
pub struct TextLayout {
    /// On the page.
    pub bounds: Rect,
    /// Bounds-sized; set where the text is black.
    pub mask: Canvas,
    /// The top of the caret, after the last line.
    pub caret: Point,
    pub caret_height: i32,
}

pub struct Typesetter {
    fonts: Database,
    families: Vec<String>,
    context: RefCell<ScaleContext>,
}

impl Typesetter {
    /// The fonts installed on the system.
    pub fn system() -> Self {
        let mut fonts = Database::new();
        fonts.load_system_fonts();
        // Inter, bundled for the menus, makes sure there's always one.
        fonts.load_font_data(crate::text::INTER.to_vec());
        Self::with(fonts)
    }

    /// Just the fonts given, for tests.
    #[cfg(test)]
    pub fn from_data(data: &[u8]) -> Self {
        let mut fonts = Database::new();
        fonts.load_font_data(data.to_vec());
        Self::with(fonts)
    }

    fn with(fonts: Database) -> Self {
        let mut families: Vec<String> = fonts
            .faces()
            .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
            // Hidden system faces start with a dot.
            .filter(|name| !name.starts_with('.'))
            .collect();
        families.sort_by_key(|name| name.to_lowercase());
        families.dedup();
        Self {
            fonts,
            families,
            context: RefCell::new(ScaleContext::new()),
        }
    }

    /// The font families, sorted, for the Font menu.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    /// The style the text tool starts with: 12 point plain, in a classic
    /// face if there is one.
    pub fn default_style(&self) -> TextStyle {
        let family = PREFERRED_FAMILIES
            .iter()
            .find(|name| self.families.iter().any(|family| family == *name))
            .map(|name| name.to_string())
            .or_else(|| self.families.first().cloned())
            .unwrap_or_default();
        TextStyle {
            family,
            size: 12,
            bold: false,
            italic: false,
            underline: false,
            outline: false,
            shadow: false,
            alignment: Alignment::Left,
        }
    }

    /// Renders the lines in black, one bit deep. With grid, line spacing
    /// rounds up to a multiple of 8, as in MacPaint.
    pub fn render(
        &self,
        lines: &[String],
        style: &TextStyle,
        origin: Point,
        grid: bool,
    ) -> TextLayout {
        let empty = TextLayout {
            bounds: Rect::default(),
            mask: Canvas::new(0, 0),
            caret: origin,
            caret_height: style.size as i32,
        };
        let query = Query {
            families: &[Family::Name(&style.family), Family::SansSerif],
            weight: if style.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            },
            stretch: Stretch::Normal,
            style: if style.italic {
                Style::Italic
            } else {
                Style::Normal
            },
        };
        let Some(id) = self.fonts.query(&query) else {
            return empty;
        };
        let Some(face) = self.fonts.face(id) else {
            return empty;
        };
        // Faces the family doesn't have are made, as QuickDraw made them.
        let fake_bold = style.bold && face.weight.0 < 600;
        let fake_italic = style.italic && face.style == Style::Normal;

        let layout = self.fonts.with_face_data(id, |data, index| {
            let font = FontRef::from_index(data, index as usize)?;
            Some(self.layout(font, lines, style, origin, grid, fake_bold, fake_italic))
        });
        let Some(Some(mut layout)) = layout else {
            return empty;
        };

        // QuickDraw's outline and shadow styles are hollow letters, which
        // is what Trace Edges makes.
        if (style.outline || style.shadow) && !layout.bounds.is_empty() {
            let mut text = Selection::from_rect(
                &layout.mask,
                Rect::new(0, 0, layout.mask.width(), layout.mask.height()),
            );
            text.trace_edges(style.shadow);
            layout.mask = text.pixels;
        }
        layout
    }

    #[allow(clippy::too_many_arguments)]
    fn layout(
        &self,
        font: FontRef,
        lines: &[String],
        style: &TextStyle,
        origin: Point,
        grid: bool,
        fake_bold: bool,
        fake_italic: bool,
    ) -> TextLayout {
        let size = style.size as f32;
        let metrics = font.metrics(&[]).scale(size);
        let ascent = metrics.ascent.ceil() as i32;
        let descent = metrics.descent.ceil() as i32;
        let mut line_height = ascent + descent + metrics.leading.ceil() as i32;
        if grid {
            line_height = trunc_to_grid(line_height + 7);
        }

        let charmap = font.charmap();
        let glyph_metrics = font.glyph_metrics(&[]).scale(size);
        // QuickDraw's bold adds a pixel to each character.
        let extra = if fake_bold { 1 } else { 0 };
        let advance = |c: char| glyph_metrics.advance_width(charmap.map(c)).round() as i32 + extra;

        // Where each line starts, aligned around the click point.
        let widths: Vec<i32> = lines
            .iter()
            .map(|line| line.chars().map(advance).sum())
            .collect();
        let lefts: Vec<i32> = widths
            .iter()
            .map(|width| match style.alignment {
                Alignment::Left => origin.x,
                Alignment::Middle => origin.x - width / 2,
                Alignment::Right => origin.x - width,
            })
            .collect();

        let count = lines.len() as i32;
        let last_baseline = origin.y + (count - 1).max(0) * line_height;
        let caret = Point::new(
            lefts
                .last()
                .zip(widths.last())
                .map_or(origin.x, |(left, width)| left + width),
            last_baseline - ascent,
        );
        let caret_height = ascent + descent;
        if lines.is_empty() {
            return TextLayout {
                bounds: Rect::default(),
                mask: Canvas::new(0, 0),
                caret,
                caret_height,
            };
        }

        // Room for italic overhang, bold, and the outline and shadow
        // smears.
        let pad = style.size as i32 / 2 + 4;
        let left = lefts.iter().copied().min().unwrap_or(origin.x);
        let right = lefts
            .iter()
            .zip(&widths)
            .map(|(l, w)| l + w)
            .max()
            .unwrap_or(left);
        let bounds = Rect::new(
            left - pad,
            origin.y - ascent - pad,
            right + pad,
            last_baseline + descent + pad,
        );
        let mut mask = Canvas::new(bounds.width(), bounds.height());

        let mut context = self.context.borrow_mut();
        let mut scaler = context.builder(font).size(size).hint(true).build();
        let mut render = Render::new(&[Source::Outline]);
        render.format(Format::Alpha);
        if fake_bold {
            render.embolden(0.5);
        }
        if fake_italic {
            render.transform(Some(Transform::skew(
                Angle::from_degrees(12.0),
                Angle::ZERO,
            )));
        }

        for (index, line) in lines.iter().enumerate() {
            let baseline = origin.y + index as i32 * line_height - bounds.top;
            let mut pen = lefts[index] - bounds.left;
            for c in line.chars() {
                if let Some(image) = render.render(&mut scaler, charmap.map(c)) {
                    let placement = image.placement;
                    let width = placement.width as usize;
                    for (i, &alpha) in image.data.iter().enumerate() {
                        if alpha >= THRESHOLD {
                            let x = pen + placement.left + (i % width) as i32;
                            let y = baseline - placement.top + (i / width) as i32;
                            mask.set(x, y, true);
                        }
                    }
                }
                pen += advance(c);
            }
            if style.underline {
                let left = lefts[index] - bounds.left;
                let y = baseline + 1 + style.size as i32 / 24;
                mask.fill_rect(left, y, left + widths[index] - 1, y, BLACK_PATTERN);
            }
        }

        TextLayout {
            bounds,
            mask,
            caret,
            caret_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typesetter() -> Typesetter {
        Typesetter::from_data(crate::text::INTER)
    }

    fn black(layout: &TextLayout) -> usize {
        layout.mask.pixels().iter().map(|&p| p as usize).sum()
    }

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|line| line.to_string()).collect()
    }

    #[test]
    fn families_and_the_default_style() {
        let typesetter = typesetter();
        assert_eq!(typesetter.families(), ["Inter"]);
        let style = typesetter.default_style();
        assert_eq!((style.family.as_str(), style.size), ("Inter", 12));
    }

    #[test]
    fn text_sits_on_the_click_point_and_aligns_around_it() {
        let typesetter = typesetter();
        let style = typesetter.default_style();
        let origin = Point::new(100, 100);
        let layout = typesetter.render(&lines(&["Hello"]), &style, origin, false);
        assert!(black(&layout) > 30);
        // Black above the baseline, starting at the click.
        let bounds = layout.bounds;
        let rows_with_ink: Vec<i32> = (0..bounds.height())
            .filter(|&y| (0..bounds.width()).any(|x| layout.mask.get(x, y)))
            .map(|y| bounds.top + y)
            .collect();
        assert!(*rows_with_ink.last().unwrap() < 101 && *rows_with_ink.first().unwrap() > 88);
        // The caret is after the text, as tall as the font.
        assert!(layout.caret.x > 120 && layout.caret.y < 100 && layout.caret_height >= 12);

        let right = Typesetter::render(
            &typesetter,
            &lines(&["Hello"]),
            &TextStyle {
                alignment: Alignment::Right,
                ..style.clone()
            },
            origin,
            false,
        );
        assert_eq!(right.caret.x, 100);

        // A second line goes below; the grid rounds the spacing to 8s.
        let two = typesetter.render(&lines(&["A", "B"]), &style, origin, true);
        assert_eq!(two.caret.y - layout.caret.y, 16);
    }

    #[test]
    fn styles_change_the_letters() {
        let typesetter = typesetter();
        let plain = typesetter.default_style();
        let render = |style: &TextStyle| {
            typesetter.render(&lines(&["Hello"]), style, Point::new(50, 50), false)
        };
        let base = black(&render(&plain));
        let bold = black(&render(&TextStyle {
            bold: true,
            ..plain.clone()
        }));
        let underline = black(&render(&TextStyle {
            underline: true,
            ..plain.clone()
        }));
        let outline = render(&TextStyle {
            outline: true,
            ..plain.clone()
        });
        assert!(bold > base && underline > base);
        assert_ne!(outline.mask.pixels(), render(&plain).mask.pixels());
        let italic = render(&TextStyle {
            italic: true,
            ..plain.clone()
        });
        assert_ne!(italic.mask.pixels(), render(&plain).mask.pixels());
    }
}
