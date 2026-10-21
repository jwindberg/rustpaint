//! The text tool: MacPaint's PlaceCaret, TextChar and UpdateText, as
//! HaikuPaint's CanvasView does them. The text stays live, restyled by the
//! Font, FontSize and Style menus, until something else happens; then it's
//! just pixels on the page.

use std::rc::Rc;

use rustpaint_engine::{Point, Rect};

use super::PageView;
use crate::typesetter::{TextStyle, Typesetter};

/// A key for the text being typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextKey {
    Text(String),
    NewLine,
    Backspace,
    /// Accepts the text (MacPaint's Enter key).
    Done,
}

/// The caret blinks about twice a second, every this many ant ticks.
const CARET_TICKS: u32 = 12;

#[derive(Default)]
pub(super) struct TextState {
    typesetter: Option<Rc<Typesetter>>,
    style: Option<TextStyle>,
    /// A caret is placed...
    active: bool,
    /// ...and something was typed.
    started: bool,
    lines: Vec<String>,
    /// The first baseline's start.
    origin: Point,
    bounds: Rect,
    caret: Point,
    caret_height: i32,
    ticks: u32,
}

impl PageView {
    /// The fonts the text tool draws with, and its style.
    pub fn set_typesetter(&mut self, typesetter: Rc<Typesetter>, style: TextStyle) {
        self.text.typesetter = Some(typesetter);
        self.text.style = Some(style);
    }

    /// The Font, FontSize and Style menus. Text being typed stays live, as
    /// in MacPaint, so a change restyles it.
    pub fn set_text_style(&mut self, style: TextStyle) {
        self.text.style = Some(style);
        if !self.text.active {
            return;
        }
        if self.text.started {
            self.render_text();
        } else {
            self.place_caret(self.text.origin);
        }
    }

    pub fn text_active(&self) -> bool {
        self.text.active
    }

    /// Steps the caret's blink. Returns whether there's a caret.
    pub(super) fn blink_caret(&mut self) -> bool {
        if !self.text.active {
            return false;
        }
        self.text.ticks += 1;
        if self.text.ticks.is_multiple_of(CARET_TICKS) {
            let visible = self.sel_overlay_is_empty();
            self.show_caret(visible);
        }
        true
    }

    /// PlaceCaret: a new block of text starts here.
    pub(super) fn place_caret(&mut self, point: Point) {
        let (Some(typesetter), Some(style)) = (&self.text.typesetter, &self.text.style) else {
            return;
        };
        let layout = typesetter.render(&[String::new()], style, point, self.grid);
        let text = &mut self.text;
        text.active = true;
        text.started = false;
        text.lines = vec![String::new()];
        text.origin = point;
        text.bounds = Rect::default();
        text.caret = layout.caret;
        text.caret_height = layout.caret_height;
        text.ticks = 0;
        self.show_caret(true);
    }

    /// TextChar: types a key into the text.
    pub fn type_text(&mut self, key: TextKey) {
        if !self.text.active {
            return;
        }
        if key == TextKey::Done {
            self.accept_text();
            return;
        }
        if let TextKey::Text(text) = &key
            && text.chars().all(char::is_control)
        {
            return;
        }
        if !self.text.started {
            self.begin_operation();
            self.text.started = true;
        }

        let lines = &mut self.text.lines;
        match key {
            TextKey::NewLine => lines.push(String::new()),
            TextKey::Backspace => {
                let line = lines.last_mut().expect("a line");
                if line.pop().is_none() && lines.len() > 1 {
                    lines.pop();
                }
            }
            TextKey::Text(text) => lines.last_mut().expect("a line").push_str(&text),
            TextKey::Done => {}
        }
        self.render_text();
    }

    /// UpdateText: restores the page under the old text, then ORs in the
    /// new.
    fn render_text(&mut self) {
        let (Some(typesetter), Some(style)) = (&self.text.typesetter, &self.text.style) else {
            return;
        };
        let layout = typesetter.render(&self.text.lines, style, self.text.origin, self.grid);
        self.canvas.copy_rect(&self.undo, self.text.bounds);
        for y in 0..layout.mask.height() {
            for x in 0..layout.mask.width() {
                if layout.mask.get(x, y) {
                    self.canvas
                        .set(layout.bounds.left + x, layout.bounds.top + y, true);
                }
            }
        }
        self.text.bounds = layout.bounds;
        self.text.caret = layout.caret;
        self.text.caret_height = layout.caret_height;
        self.text.ticks = 0;
        self.show_caret(true);
    }

    /// Accepting leaves the text on the page, as it already is.
    pub(super) fn accept_text(&mut self) {
        if !self.text.active {
            return;
        }
        self.text.active = false;
        self.text.started = false;
        self.text.lines.clear();
        self.text.bounds = Rect::default();
        self.set_overlay(Vec::new(), false);
    }

    fn show_caret(&mut self, visible: bool) {
        let caret = if visible {
            let (x, top) = (self.text.caret.x, self.text.caret.y);
            (0..self.text.caret_height)
                .map(|y| Point::new(x, top + y))
                .collect()
        } else {
            Vec::new()
        };
        self.set_overlay(caret, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_view::Modifiers;
    use crate::tool::Tool;

    fn text_view() -> PageView {
        let typesetter = Rc::new(Typesetter::from_data(crate::text::INTER));
        let style = typesetter.default_style();
        let mut view = PageView::new();
        view.set_typesetter(typesetter, style);
        view.set_tool(Tool::Text);
        view
    }

    fn black_count(view: &PageView) -> usize {
        view.canvas().pixels().iter().map(|&p| p as usize).sum()
    }

    fn type_str(view: &mut PageView, text: &str) {
        view.type_text(TextKey::Text(text.into()));
    }

    #[test]
    fn typing_puts_text_on_the_page_and_undo_takes_it_off() {
        let mut view = text_view();
        view.mouse_down(Point::new(100, 100), Modifiers::default());
        assert!(view.text_active() && view.overlay().0.len() >= 12);
        assert_eq!(black_count(&view), 0);

        type_str(&mut view, "Hi");
        let hi = black_count(&view);
        assert!(hi > 10);
        type_str(&mut view, "!");
        assert!(black_count(&view) > hi);
        view.type_text(TextKey::Backspace);
        assert_eq!(black_count(&view), hi);

        // A new line goes below the first.
        view.type_text(TextKey::NewLine);
        type_str(&mut view, "Yo");
        let two_lines = view.canvas().clone();
        assert!((110..130).any(|y| (90..140).any(|x| two_lines.get(x, y))));

        // Done leaves it on the page; undo takes the whole block away.
        view.type_text(TextKey::Done);
        assert!(!view.text_active() && view.overlay().0.is_empty());
        view.undo();
        assert_eq!(black_count(&view), 0);
    }

    #[test]
    fn restyling_redraws_the_live_text() {
        let mut view = text_view();
        view.mouse_down(Point::new(100, 100), Modifiers::default());
        type_str(&mut view, "Bold");
        let plain = black_count(&view);
        let mut style = view.text.style.clone().unwrap();
        style.bold = true;
        view.set_text_style(style);
        assert!(black_count(&view) > plain);

        // Once accepted, a new style doesn't touch it.
        view.set_tool(Tool::Pencil);
        let accepted = black_count(&view);
        let mut style = view.text.style.clone().unwrap();
        style.size = 24;
        view.set_text_style(style);
        assert_eq!(black_count(&view), accepted);
    }

    #[test]
    fn the_caret_blinks() {
        let mut view = text_view();
        view.mouse_down(Point::new(50, 50), Modifiers::default());
        for _ in 0..CARET_TICKS {
            assert!(view.blink_caret());
        }
        assert!(view.overlay().0.is_empty());
        for _ in 0..CARET_TICKS {
            view.blink_caret();
        }
        assert!(!view.overlay().0.is_empty());
    }
}
