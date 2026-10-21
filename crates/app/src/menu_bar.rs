//! The menu bar drawn in the window on Linux and Windows, as PhotoCraft
//! does (macOS has the system's): white with a black rule, the open title
//! inverted, and menus in framed boxes with a drop shadow, the chosen item
//! inverted and unavailable ones gray. The boxes are 1-bit like the rest
//! of the window; the text is smooth (see crate::text). A menu too long
//! for the window, like Font with many families, goes into columns.

use rustpaint_engine::paint_font::{GLYPH_CHECK, paint_font_glyph};
use rustpaint_engine::{Canvas, Point, Rect};

use crate::art::{draw_glyph, draw_palette_box, invert_qd_rect};
use crate::commands::{Command, MenuSpec, MenuState, menus};
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

/// Where an open menu goes, and how its rows are arranged.
struct Placement {
    /// The box, frame included and shadow not.
    rect: Rect,
    column_width: i32,
    rows_per_column: usize,
}

pub struct MenuBar {
    font: TextFont,
    menus: Vec<MenuSpec>,
    families: Vec<String>,
    /// Each menu's title, where it is in the bar.
    titles: Vec<Rect>,
    /// The window's height, which long menus wrap to fit.
    window_height: i32,
    open: Option<usize>,
    highlight: Option<usize>,
    /// The menu stays open after a click on its title, until the next
    /// click.
    sticky: bool,
}

/// The shortcut as Linux and Windows write it.
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
    /// The menus, with the installed font families in the Font menu.
    pub fn new(families: Vec<String>) -> Self {
        let font = TextFont::new(INTER, FONT_SIZE);
        let menus = menus(families.len());
        let mut left = TITLE_START;
        let titles = menus
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
            menus,
            families,
            titles,
            window_height: i32::MAX,
            open: None,
            highlight: None,
            sticky: false,
        }
    }

    pub fn set_window_height(&mut self, height: i32) {
        self.window_height = height;
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The rows of a menu, separators between its groups.
    fn rows(&self, menu: usize) -> Vec<Row> {
        let mut rows = Vec::new();
        for (index, group) in self.menus[menu].groups.iter().enumerate() {
            if index > 0 {
                rows.push(Row::Separator);
            }
            rows.extend(group.iter().map(|&command| Row::Item(command)));
        }
        rows
    }

    fn title_at(&self, point: Point) -> Option<usize> {
        if point.y < 0 || point.y >= HEIGHT {
            return None;
        }
        self.titles
            .iter()
            .position(|title| point.x >= title.left && point.x < title.right)
    }

    fn placement(&self, menu: usize) -> Placement {
        let rows = self.rows(menu);
        let column_width = rows
            .iter()
            .map(|row| match row {
                Row::Item(command) => {
                    let shortcut = shortcut_label(*command)
                        .map_or(0, |label| SHORTCUT_GAP + self.font.width(&label));
                    let label = command.label(&self.families);
                    TEXT_LEFT + self.font.width(&label) + shortcut + TEXT_RIGHT
                }
                Row::Separator => 0,
            })
            .max()
            .unwrap_or(0)
            .max(120);
        let room = (self.window_height - HEIGHT - 4) / ROW_HEIGHT;
        let rows_per_column = (room.max(1) as usize).min(rows.len().max(1));
        let columns = rows.len().div_ceil(rows_per_column).max(1) as i32;
        let left = self.titles[menu].left;
        let top = HEIGHT - 1;
        Placement {
            rect: Rect::new(
                left,
                top,
                left + columns * column_width,
                top + rows_per_column as i32 * ROW_HEIGHT + 2,
            ),
            column_width,
            rows_per_column,
        }
    }

    /// Where row `index` of a menu is, inside its box (box coordinates).
    fn row_rect(placement: &Placement, index: usize) -> Rect {
        let column = (index / placement.rows_per_column) as i32;
        let row = (index % placement.rows_per_column) as i32;
        let left = 1 + column * placement.column_width;
        let top = 1 + row * ROW_HEIGHT;
        let right = (left + placement.column_width).min(placement.rect.width() - 1);
        Rect::new(left, top, right, top + ROW_HEIGHT)
    }

    /// The row of the open menu at point.
    fn row_at(&self, point: Point) -> Option<usize> {
        let menu = self.open?;
        let placement = self.placement(menu);
        let rect = placement.rect;
        if !contains(&rect.inset_by(1, 1), point) {
            return None;
        }
        let column = ((point.x - rect.left - 1) / placement.column_width) as usize;
        let row = ((point.y - rect.top - 1) / ROW_HEIGHT) as usize;
        let index = column * placement.rows_per_column + row;
        (index < self.rows(menu).len()).then_some(index)
    }

    /// The command at point, if it's an item that's available.
    fn item_at(&self, point: Point, state: MenuState) -> Option<(usize, Command)> {
        let row = self.row_at(point)?;
        match self.rows(self.open?)[row] {
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

    /// The open menu and where it goes, shadow included, without its text
    /// but with its check marks (PaintFont's).
    pub fn render_menu(&self, state: MenuState) -> Option<(Point, Canvas)> {
        let menu = self.open?;
        let placement = self.placement(menu);
        let rect = placement.rect;
        let mut canvas = Canvas::new(rect.width() + 1, rect.height() + 1);
        draw_palette_box(&mut canvas);

        for (index, row) in self.rows(menu).iter().enumerate() {
            let cell = Self::row_rect(&placement, index);
            match row {
                Row::Separator => {
                    // A dotted gray line.
                    let y = cell.top + ROW_HEIGHT / 2;
                    for x in (cell.left..cell.right).step_by(2) {
                        canvas.set(x, y, true);
                    }
                }
                Row::Item(command) => {
                    if command.is_checked(state) == Some(true)
                        && let Some(check) = paint_font_glyph(GLYPH_CHECK)
                    {
                        draw_glyph(&mut canvas, check, cell.left + 1, cell.top, false);
                    }
                    if self.highlight == Some(index) {
                        invert_qd_rect(&mut canvas, cell.left, cell.top, cell.right, cell.bottom);
                    }
                }
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
            draw(
                title.left + TITLE_PADDING,
                14,
                self.menus[index].title,
                color,
            );
        }

        let Some(menu) = self.open else { return };
        let placement = self.placement(menu);
        let origin = Point::new(placement.rect.left, placement.rect.top);
        for (index, row) in self.rows(menu).iter().enumerate() {
            let Row::Item(command) = row else { continue };
            let cell = Self::row_rect(&placement, index);
            let color = if self.highlight == Some(index) {
                WHITE
            } else if command.is_enabled(state) {
                BLACK
            } else {
                GRAY
            };
            let baseline = origin.y + cell.top + 12;
            let label = command.label(&self.families);
            draw(
                origin.x + cell.left + TEXT_LEFT - 1,
                baseline,
                &label,
                color,
            );
            if let Some(shortcut) = shortcut_label(*command) {
                let x = origin.x + cell.right - TEXT_RIGHT - self.font.width(&shortcut);
                draw(x, baseline, &shortcut, color);
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
        grid: false,
        fat_bits: false,
        mirrors: false,
        modal: false,
        text: crate::commands::TextMenuState {
            font: None,
            size: None,
            bold: false,
            italic: false,
            underline: false,
            outline: false,
            shadow: false,
            alignment: crate::typesetter::Alignment::Left,
        },
    };

    fn bar() -> MenuBar {
        MenuBar::new(vec!["Geneva".into(), "Monaco".into()])
    }

    /// The middle of row `row` of the open menu.
    fn row_point(bar: &MenuBar, row: usize) -> Point {
        let placement = bar.placement(bar.open.unwrap());
        let cell = MenuBar::row_rect(&placement, row);
        Point::new(
            placement.rect.left + cell.left + 30,
            placement.rect.top + cell.top + 8,
        )
    }

    #[test]
    fn dragging_down_a_menu_chooses_on_release() {
        let mut bar = bar();
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
        let mut bar = bar();
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
        let mut bar = bar();
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
    fn the_font_menu_lists_the_families_and_long_menus_wrap() {
        let mut bar = MenuBar::new((0..40).map(|i| format!("Font {i}")).collect());
        bar.set_window_height(HEIGHT + 4 + 10 * ROW_HEIGHT);
        let font = Point::new(bar.titles[3].left + 5, 10);
        bar.mouse_down(font, ALL);
        let placement = bar.placement(3);
        assert_eq!(placement.rows_per_column, 10);
        assert_eq!(placement.rect.width(), 4 * placement.column_width);

        // The first row of the third column is the 21st family.
        let point = row_point(&bar, 20);
        bar.mouse_moved(point, ALL);
        assert_eq!(bar.mouse_up(point, ALL), Some(Command::Font(20)));
    }

    #[test]
    fn menus_render_with_their_titles_and_items() {
        let mut bar = bar();
        let canvas = bar.render_bar(300);
        assert!((0..300).all(|x| canvas.get(x, HEIGHT - 1)));
        assert!(bar.render_menu(ALL).is_none());

        bar.mouse_down(Point::new(bar.titles[0].left + 5, 10), ALL);
        let open = bar.render_bar(300);
        // The open title is inverted.
        assert!(open.get(bar.titles[0].left + 1, 2));
        let (origin, menu) = bar.render_menu(ALL).unwrap();
        assert_eq!(origin, Point::new(bar.titles[0].left, HEIGHT - 1));
        assert!(menu.width() > 120 && menu.height() == 9 * ROW_HEIGHT + 3);
    }
}
