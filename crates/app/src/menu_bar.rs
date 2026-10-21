//! The menu bar drawn in the window on Linux and Windows, as PhotoCraft
//! does (macOS has the system's): white with a black rule, the open title
//! inverted, and menus in framed boxes with a drop shadow, the chosen item
//! inverted and unavailable ones gray. The boxes are 1-bit like the rest
//! of the window; the text is smooth (see crate::text).

use rustpaint_engine::{Canvas, Point, Rect};

use crate::art::{draw_palette_box, invert_qd_rect};
use crate::commands::{Command, MENUS, MenuState};
use crate::text::{INTER, TextFont};

pub const HEIGHT: i32 = 20;
const FONT_SIZE: f32 = 13.0;
const TITLE_START: i32 = 10;
const TITLE_PADDING: i32 = 8;
const ROW_HEIGHT: i32 = 16;
/// Room at the left of a menu, where the Mac put check marks.
const TEXT_LEFT: i32 = 14;
/// Space between an item's label and its shortcut.
const SHORTCUT_GAP: i32 = 24;
const TEXT_RIGHT: i32 = 10;

const BLACK: u32 = 0x000000;
const WHITE: u32 = 0xffffff;
/// Unavailable items.
const GRAY: u32 = 0x8c8c8c;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Item(Command),
    Separator,
}

pub struct MenuBar {
    font: TextFont,
    /// Each menu's title, where it is in the bar.
    titles: Vec<Rect>,
    open: Option<usize>,
    highlight: Option<usize>,
    /// The menu stays open after a click on its title, until the next
    /// click.
    sticky: bool,
}

/// The rows of a menu, separators between its groups.
fn rows(menu: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    for (index, group) in MENUS[menu].groups.iter().enumerate() {
        if index > 0 {
            rows.push(Row::Separator);
        }
        rows.extend(group.iter().map(|&command| Row::Item(command)));
    }
    rows
}

/// The shortcut as Linux writes it.
fn shortcut_label(command: Command) -> Option<String> {
    command.shortcut().map(|(key, shift)| {
        let key = key.to_ascii_uppercase();
        if shift {
            format!("Shift+Ctrl+{key}")
        } else {
            format!("Ctrl+{key}")
        }
    })
}

fn contains(rect: &Rect, point: Point) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

impl MenuBar {
    pub fn new() -> Self {
        let font = TextFont::new(INTER, FONT_SIZE);
        let mut left = TITLE_START;
        let titles = MENUS
            .iter()
            .map(|menu| {
                let width = font.width(menu.title) + 2 * TITLE_PADDING;
                let rect = Rect::new(left, 0, left + width, HEIGHT - 1);
                left += width;
                rect
            })
            .collect();
        Self {
            font,
            titles,
            open: None,
            highlight: None,
            sticky: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn title_at(&self, point: Point) -> Option<usize> {
        if point.y < 0 || point.y >= HEIGHT {
            return None;
        }
        self.titles
            .iter()
            .position(|title| point.x >= title.left && point.x < title.right)
    }

    /// The open menu's box, frame included and shadow not.
    fn menu_rect(&self, menu: usize) -> Rect {
        let width = rows(menu)
            .iter()
            .map(|row| match row {
                Row::Item(command) => {
                    let shortcut = shortcut_label(*command)
                        .map_or(0, |label| SHORTCUT_GAP + self.font.width(&label));
                    TEXT_LEFT + self.font.width(command.label()) + shortcut + TEXT_RIGHT
                }
                Row::Separator => 0,
            })
            .max()
            .unwrap_or(0)
            .max(120);
        let height = rows(menu).len() as i32 * ROW_HEIGHT + 2;
        let left = self.titles[menu].left;
        Rect::new(left, HEIGHT - 1, left + width, HEIGHT - 1 + height)
    }

    /// The row of the open menu at point.
    fn row_at(&self, point: Point) -> Option<usize> {
        let menu = self.open?;
        let rect = self.menu_rect(menu);
        if !contains(&rect.inset_by(1, 1), point) {
            return None;
        }
        Some(((point.y - rect.top - 1) / ROW_HEIGHT) as usize)
    }

    /// The command at point, if it's an item that's available.
    fn item_at(&self, point: Point, state: MenuState) -> Option<(usize, Command)> {
        let row = self.row_at(point)?;
        match rows(self.open?)[row] {
            Row::Item(command) if command.is_enabled(state) => Some((row, command)),
            _ => None,
        }
    }

    fn close(&mut self) {
        self.open = None;
        self.highlight = None;
        self.sticky = false;
    }

    /// A press in the window. Returns whether the menus took it.
    pub fn mouse_down(&mut self, point: Point, state: MenuState) -> bool {
        if let Some(title) = self.title_at(point) {
            if self.open == Some(title) && self.sticky {
                self.close();
            } else {
                self.open = Some(title);
                self.sticky = false;
                self.highlight = None;
            }
            return true;
        }
        if self.open.is_none() {
            return false;
        }
        // A click in a sticky menu chooses on release; a click elsewhere
        // just closes it.
        if self.row_at(point).is_some() {
            self.highlight = self.item_at(point, state).map(|(row, _)| row);
        } else {
            self.close();
        }
        true
    }

    /// The pointer moved, with the button down or a menu open.
    pub fn mouse_moved(&mut self, point: Point, state: MenuState) {
        if self.open.is_none() {
            return;
        }
        if let Some(title) = self.title_at(point) {
            if self.open != Some(title) {
                self.open = Some(title);
                self.highlight = None;
            }
            return;
        }
        self.highlight = self.item_at(point, state).map(|(row, _)| row);
    }

    /// The button came up. Returns the command chosen, if any.
    pub fn mouse_up(&mut self, point: Point, state: MenuState) -> Option<Command> {
        self.open?;
        if let Some((_, command)) = self.item_at(point, state) {
            self.close();
            return Some(command);
        }
        // Released on the title: keep the menu open to click in.
        if self.title_at(point).is_some() && !self.sticky {
            self.sticky = true;
            return None;
        }
        if self.row_at(point).is_none() {
            self.close();
        }
        None
    }

    /// The bar, as wide as the window, without its text.
    pub fn render_bar(&self, width: i32) -> Canvas {
        let mut canvas = Canvas::new(width, HEIGHT);
        canvas.draw_line(0, HEIGHT - 1, width - 1, HEIGHT - 1, true);
        if let Some(open) = self.open {
            let title = &self.titles[open];
            invert_qd_rect(
                &mut canvas,
                title.left,
                title.top,
                title.right,
                title.bottom,
            );
        }
        canvas
    }

    /// The open menu and where it goes, shadow included, without its text.
    pub fn render_menu(&self) -> Option<(Point, Canvas)> {
        let menu = self.open?;
        let rect = self.menu_rect(menu);
        let mut canvas = Canvas::new(rect.width() + 1, rect.height() + 1);
        draw_palette_box(&mut canvas);

        for (index, row) in rows(menu).iter().enumerate() {
            let top = 1 + index as i32 * ROW_HEIGHT;
            if *row == Row::Separator {
                // A dotted gray line.
                let y = top + ROW_HEIGHT / 2;
                for x in (1..rect.width() - 1).step_by(2) {
                    canvas.set(x, y, true);
                }
            } else if self.highlight == Some(index) {
                invert_qd_rect(&mut canvas, 1, top, rect.width() - 1, top + ROW_HEIGHT);
            }
        }
        Some((Point::new(rect.left, rect.top), canvas))
    }

    /// Draws the titles and the open menu's items over the boxes, into a
    /// buffer of physical pixels, `scale` per logical one.
    pub fn draw_text(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        scale: u32,
        state: MenuState,
    ) {
        let mut draw = |x, baseline, text: &str, color| {
            self.font
                .draw(buffer, width, height, scale, x, baseline, text, color);
        };
        for (index, title) in self.titles.iter().enumerate() {
            let color = if self.open == Some(index) {
                WHITE
            } else {
                BLACK
            };
            draw(title.left + TITLE_PADDING, 14, MENUS[index].title, color);
        }

        let Some(menu) = self.open else { return };
        let rect = self.menu_rect(menu);
        for (index, row) in rows(menu).iter().enumerate() {
            let Row::Item(command) = row else { continue };
            let color = if self.highlight == Some(index) {
                WHITE
            } else if command.is_enabled(state) {
                BLACK
            } else {
                GRAY
            };
            let baseline = rect.top + 1 + index as i32 * ROW_HEIGHT + 12;
            draw(rect.left + TEXT_LEFT, baseline, command.label(), color);
            if let Some(label) = shortcut_label(*command) {
                let x = rect.right - TEXT_RIGHT - self.font.width(&label);
                draw(x, baseline, &label, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: MenuState = MenuState {
        can_undo: true,
        has_selection: true,
        can_revert: true,
    };

    /// The middle of row `row` of the open menu.
    fn row_point(bar: &MenuBar, row: usize) -> Point {
        let rect = bar.menu_rect(bar.open.unwrap());
        Point::new(rect.left + 30, rect.top + 1 + row as i32 * ROW_HEIGHT + 8)
    }

    #[test]
    fn dragging_down_a_menu_chooses_on_release() {
        let mut bar = MenuBar::new();
        let file = Point::new(bar.titles[0].left + 5, 10);
        assert!(bar.mouse_down(file, ALL) && bar.is_open());
        // File: New, Open, separator, Save, ...
        let save = row_point(&bar, 3);
        bar.mouse_moved(save, ALL);
        assert_eq!(bar.highlight, Some(3));
        assert_eq!(bar.mouse_up(save, ALL), Some(Command::Save));
        assert!(!bar.is_open());
    }

    #[test]
    fn a_click_on_a_title_keeps_the_menu_open() {
        let mut bar = MenuBar::new();
        let edit = Point::new(bar.titles[1].left + 5, 10);
        bar.mouse_down(edit, ALL);
        assert_eq!(bar.mouse_up(edit, ALL), None);
        assert!(bar.is_open());

        // Then a click on an item chooses it.
        let cut = row_point(&bar, 2);
        assert!(bar.mouse_down(cut, ALL));
        assert_eq!(bar.mouse_up(cut, ALL), Some(Command::Cut));

        // A click elsewhere closes the menu without passing through.
        bar.mouse_down(edit, ALL);
        bar.mouse_up(edit, ALL);
        assert!(bar.mouse_down(Point::new(400, 400), ALL) && !bar.is_open());
        assert!(!bar.mouse_down(Point::new(400, 400), ALL));
    }

    #[test]
    fn unavailable_items_and_separators_cant_be_chosen() {
        let mut bar = MenuBar::new();
        let edit = Point::new(bar.titles[1].left + 5, 10);
        bar.mouse_down(edit, MenuState::default());
        let undo = row_point(&bar, 0);
        bar.mouse_moved(undo, MenuState::default());
        assert_eq!(bar.highlight, None);
        assert_eq!(bar.mouse_up(undo, MenuState::default()), None);

        bar.mouse_down(edit, ALL);
        let separator = row_point(&bar, 1);
        assert_eq!(bar.mouse_up(separator, ALL), None);
    }

    #[test]
    fn menus_render_with_their_titles_and_items() {
        let mut bar = MenuBar::new();
        let canvas = bar.render_bar(300);
        assert!((0..300).all(|x| canvas.get(x, HEIGHT - 1)));
        assert!(bar.render_menu().is_none());

        bar.mouse_down(Point::new(bar.titles[0].left + 5, 10), ALL);
        let open = bar.render_bar(300);
        // The open title is inverted.
        assert!(open.get(bar.titles[0].left + 1, 2));
        let (origin, menu) = bar.render_menu().unwrap();
        assert_eq!(origin, Point::new(bar.titles[0].left, HEIGHT - 1));
        assert!(menu.width() > 120 && menu.height() == 9 * ROW_HEIGHT + 3);
    }
}
