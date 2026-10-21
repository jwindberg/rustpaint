//! MacPaint's Goodies dialogs, drawn as modal boxes in the window the way
//! the Mac drew dialogs on its one screen. Each is laid out as HaikuPaint
//! lays it out from MacPaint's DLOG and DITL resources: the content area
//! is the DLOG's rect, inside a double frame. The boxes are 1-bit; button
//! labels are smooth text, like the menus'.

mod about;
mod brush_shape;
mod edit_pattern;
mod mirrors;
mod show_page;

use rustpaint_engine::{BLACK_PATTERN, Canvas, Pattern, Point, Rect, Shape};

pub use mirrors::{MIRROR_ANTI_DIAGONAL, MIRROR_DIAGONAL, MIRROR_HORIZONTAL, MIRROR_VERTICAL};

use crate::art::{frame_qd_rect, paint_qd_rect};
use crate::i18n::tr;
use crate::text::TextFont;

/// The double frame around a dialog's content: black, white, then a
/// thicker black.
const FRAME: i32 = 4;
/// MacPaint's buttons are 18 pixels tall.
const BUTTON_HEIGHT: i32 = 18;

/// What a dialog decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Still open.
    Open,
    /// Closed without changing anything.
    Cancelled,
    /// Edit Pattern's OK.
    Pattern(Pattern),
    /// A brush was clicked in Brush Shape.
    Brush(i32),
    /// Brush Mirrors' OK or None: the `MIRROR_*` flags that are on.
    Mirrors(u8),
    /// Show Page's OK: move the drawing by shift, and show the page from
    /// origin (page pixels).
    ShowPage { shift: Point, origin: Point },
}

/// A push button, in content coordinates.
struct Button {
    rect: Rect,
    label: String,
    /// Return chooses it; it has the thick outline.
    default: bool,
}

impl Button {
    /// A button labelled with the message `label` (see crate::i18n).
    fn new(left: i32, top: i32, right: i32, label: &str, default: bool) -> Self {
        Self {
            rect: Rect::new(left, top, right, top + BUTTON_HEIGHT),
            label: tr(label),
            default,
        }
    }

    fn contains(&self, point: Point) -> bool {
        let r = &self.rect;
        point.x >= r.left && point.x < r.right && point.y >= r.top && point.y < r.bottom
    }
}

enum Kind {
    About,
    EditPattern(edit_pattern::EditPattern),
    BrushShape(brush_shape::BrushShape),
    Mirrors(mirrors::Mirrors),
    ShowPage(show_page::ShowPage),
}

pub struct Dialog {
    kind: Kind,
    buttons: Vec<Button>,
    /// The button being pressed, and whether the pointer is still on it.
    pressed: Option<(usize, bool)>,
}

impl Dialog {
    pub fn about() -> Self {
        Self::new(Kind::About)
    }

    pub fn edit_pattern(pattern: Pattern) -> Self {
        Self::new(Kind::EditPattern(edit_pattern::EditPattern::new(pattern)))
    }

    pub fn brush_shape(brush: i32) -> Self {
        Self::new(Kind::BrushShape(brush_shape::BrushShape::new(brush)))
    }

    pub fn mirrors(mirrors: u8) -> Self {
        Self::new(Kind::Mirrors(mirrors::Mirrors::new(mirrors)))
    }

    /// Show Page for the page, with visible the part in the window.
    pub fn show_page(page: &Canvas, visible: Rect) -> Self {
        Self::new(Kind::ShowPage(show_page::ShowPage::new(page, visible)))
    }

    fn new(kind: Kind) -> Self {
        let buttons = match &kind {
            Kind::About => about::buttons(),
            Kind::EditPattern(_) => edit_pattern::buttons(),
            Kind::BrushShape(_) => Vec::new(),
            Kind::Mirrors(_) => mirrors::buttons(),
            Kind::ShowPage(show_page) => show_page.buttons(),
        };
        Self {
            kind,
            buttons,
            pressed: None,
        }
    }

    fn content_size(&self) -> (i32, i32) {
        match &self.kind {
            Kind::About => about::SIZE,
            Kind::EditPattern(_) => edit_pattern::SIZE,
            Kind::BrushShape(_) => brush_shape::SIZE,
            Kind::Mirrors(_) => mirrors::SIZE,
            Kind::ShowPage(show_page) => show_page.size(),
        }
    }

    /// The dialog's size, frame included.
    pub fn size(&self) -> (i32, i32) {
        let (width, height) = self.content_size();
        (width + 2 * FRAME, height + 2 * FRAME)
    }

    pub fn render(&self) -> Canvas {
        let (width, height) = self.size();
        let mut canvas = Canvas::new(width, height);
        paint_qd_rect(&mut canvas, 0, 0, width, height, BLACK_PATTERN);
        paint_qd_rect(
            &mut canvas,
            1,
            1,
            width - 1,
            height - 1,
            rustpaint_engine::WHITE_PATTERN,
        );
        paint_qd_rect(&mut canvas, 2, 2, width - 2, height - 2, BLACK_PATTERN);
        paint_qd_rect(
            &mut canvas,
            FRAME,
            FRAME,
            width - FRAME,
            height - FRAME,
            rustpaint_engine::WHITE_PATTERN,
        );

        let (content_width, content_height) = self.content_size();
        let mut content = Canvas::new(content_width, content_height);
        match &self.kind {
            Kind::About => about::render(&mut content),
            Kind::EditPattern(dialog) => dialog.render(&mut content),
            Kind::BrushShape(dialog) => dialog.render(&mut content),
            Kind::Mirrors(dialog) => dialog.render(&mut content),
            Kind::ShowPage(dialog) => dialog.render(&mut content),
        }
        for (index, button) in self.buttons.iter().enumerate() {
            let rect = button.rect;
            if self.pressed == Some((index, true)) {
                content.fill_shape(Shape::RoundRect, rect, BLACK_PATTERN);
            } else {
                content.frame_shape(Shape::RoundRect, rect, 1, BLACK_PATTERN);
            }
            if button.default {
                content.frame_shape(Shape::RoundRect, rect.inset_by(-4, -4), 3, BLACK_PATTERN);
            }
        }
        for y in 0..content_height {
            for x in 0..content_width {
                canvas.set(FRAME + x, FRAME + y, content.get(x, y));
            }
        }
        canvas
    }

    /// Draws the button labels, the dialog being at origin (logical), into
    /// a buffer of physical pixels, `scale` per logical one.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &self,
        font: &TextFont,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        scale: u32,
        origin: Point,
    ) {
        if let Kind::About = self.kind {
            for (x, baseline, line) in about::lines() {
                let (x, baseline) = (origin.x + FRAME + x, origin.y + FRAME + baseline);
                font.draw(buffer, width, height, scale, x, baseline, &line, 0x000000);
            }
        }
        for (index, button) in self.buttons.iter().enumerate() {
            let rect = button.rect;
            let x = origin.x + FRAME + (rect.left + rect.right - font.width(&button.label)) / 2;
            let baseline = origin.y + FRAME + rect.top + 13;
            let color = if self.pressed == Some((index, true)) {
                0xffffff
            } else {
                0x000000
            };
            font.draw(
                buffer,
                width,
                height,
                scale,
                x,
                baseline,
                &button.label,
                color,
            );
        }
    }

    /// A point in the dialog (frame included) to its content's.
    fn content_point(point: Point) -> Point {
        Point::new(point.x - FRAME, point.y - FRAME)
    }

    fn button_outcome(&self, index: usize) -> Outcome {
        match &self.kind {
            Kind::About => about::button(index),
            Kind::EditPattern(dialog) => dialog.button(index),
            Kind::BrushShape(_) => Outcome::Open,
            Kind::Mirrors(dialog) => dialog.button(index),
            Kind::ShowPage(dialog) => dialog.button(index),
        }
    }

    /// A press at point, relative to the dialog. Presses outside it are
    /// ignored, as a modal dialog's are.
    pub fn mouse_down(&mut self, point: Point) -> Outcome {
        let point = Self::content_point(point);
        if let Some(index) = self.buttons.iter().position(|b| b.contains(point)) {
            self.pressed = Some((index, true));
            return Outcome::Open;
        }
        match &mut self.kind {
            Kind::About => {}
            Kind::EditPattern(dialog) => dialog.mouse_down(point),
            Kind::BrushShape(dialog) => return dialog.mouse_down(point),
            Kind::Mirrors(dialog) => dialog.mouse_down(point),
            Kind::ShowPage(dialog) => dialog.mouse_down(point),
        }
        Outcome::Open
    }

    pub fn mouse_moved(&mut self, point: Point, shift: bool) {
        let point = Self::content_point(point);
        if let Some((index, _)) = self.pressed {
            self.pressed = Some((index, self.buttons[index].contains(point)));
            return;
        }
        match &mut self.kind {
            Kind::EditPattern(dialog) => dialog.mouse_moved(point),
            Kind::ShowPage(dialog) => dialog.mouse_moved(point, shift),
            _ => {}
        }
    }

    pub fn mouse_up(&mut self) -> Outcome {
        match &mut self.kind {
            Kind::EditPattern(dialog) => dialog.mouse_up(),
            Kind::ShowPage(dialog) => dialog.mouse_up(),
            _ => {}
        }
        match self.pressed.take() {
            Some((index, true)) => self.button_outcome(index),
            _ => Outcome::Open,
        }
    }

    /// Return chooses the default button; Escape cancels.
    pub fn key(&mut self, escape: bool) -> Outcome {
        if escape {
            return Outcome::Cancelled;
        }
        match self.buttons.iter().position(|button| button.default) {
            Some(index) => self.button_outcome(index),
            None => Outcome::Open,
        }
    }
}

/// QuickDraw's FrameRect, as dialogs use it.
fn frame(canvas: &mut Canvas, rect: Rect) {
    frame_qd_rect(canvas, rect.left, rect.top, rect.right, rect.bottom);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpaint_engine::patterns::PATTERNS;

    /// A point in the content, as the dialog sees it.
    fn at(x: i32, y: i32) -> Point {
        Point::new(x + FRAME, y + FRAME)
    }

    #[test]
    fn buttons_act_on_release_inside_them() {
        let mut dialog = Dialog::edit_pattern(PATTERNS[3]);
        // OK, pressed then dragged off: nothing.
        dialog.mouse_down(at(40, 100));
        dialog.mouse_moved(at(40, 200), false);
        assert_eq!(dialog.mouse_up(), Outcome::Open);
        // Cancel.
        dialog.mouse_down(at(120, 100));
        assert_eq!(dialog.mouse_up(), Outcome::Cancelled);
        // Return is OK; Escape cancels.
        assert_eq!(dialog.key(false), Outcome::Pattern(PATTERNS[3]));
        assert_eq!(dialog.key(true), Outcome::Cancelled);
    }

    #[test]
    fn dialogs_render_their_frame_and_buttons() {
        let dialog = Dialog::mirrors(0);
        let canvas = dialog.render();
        assert_eq!((canvas.width(), canvas.height()), (176 + 8, 176 + 8));
        // Black, white, black around the content.
        assert!(canvas.get(0, 50) && !canvas.get(1, 50) && canvas.get(2, 50));
        // The OK button's frame.
        assert!(canvas.get(FRAME + 48, FRAME + 140));
    }
}
