//! RustPaint: Bill Atkinson's MacPaint, re-created. The window draws
//! everything into one 1-bit-looking software framebuffer, as MacPaint did,
//! scaled up by a whole number on HiDPI screens to keep the pixels crisp.

mod art;
mod clipboard;
mod commands;
mod documents;
mod images;
mod menu_bar;
#[cfg(target_os = "macos")]
mod menus;
mod page_view;
mod palettes;
mod text;
mod tool;

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rustpaint_engine::paint_font::{
    DEFAULT_BRUSH, GLYPH_ALL_ONES, GLYPH_ERASER_BOX, GLYPH_FILL_MASK, GLYPH_FIRST_BRUSH,
    GLYPH_LINE_CURSOR_1, GLYPH_LINE_CURSOR_2, GLYPH_LINE_CURSOR_4, GLYPH_LINE_CURSOR_8,
    GLYPH_PENCIL_MASK, GLYPH_SPRAY,
};
use rustpaint_engine::patterns::{DEFAULT_PATTERN_INDEX, PATTERN_COUNT, PATTERNS};
use rustpaint_engine::{Canvas, Pattern, Point, Rect};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, CustomCursor, Window, WindowId};

use crate::art::CURSOR_SIZE;
use crate::clipboard::Clipboard;
use crate::commands::{Command, MenuState};
use crate::documents::{MACPAINT_EXTENSIONS, export_png, load_document, save_macpaint};
use crate::page_view::{Modifiers, PageView, SelectionCommand, ant_bit};
use crate::palettes::{line_widths, patterns, tools};
use crate::tool::Tool;

/// Space around and between the palettes and the page.
const MARGIN: i32 = 6;
/// Linux and Windows get a menu bar drawn in the window, as PhotoCraft
/// does: winit's Linux windows can't have native menus, and drawing them
/// keeps the two alike. macOS has the system's menu bar.
const DRAWN_MENUS: bool = !cfg!(target_os = "macos");
/// The window's top, above everything else: the drawn menu bar.
const TOP: i32 = if DRAWN_MENUS { menu_bar::HEIGHT } else { 0 };
/// The window's background, and the area beyond the page's edges.
const PANEL_COLOR: u32 = 0xd8d8d8;
const OUTSIDE_PAGE_COLOR: u32 = 0x989898;
const BLACK: u32 = 0x000000;
const WHITE: u32 = 0xffffff;

const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(500);
/// Pixels a mouse wheel notch scrolls.
const LINE_SCROLL: f64 = 16.0;
/// MacPaint stepped its ants every tick (1/60 s); a little slower is
/// kinder.
const ANTS_INTERVAL: Duration = Duration::from_millis(40);

const UNTITLED: &str = "untitled";

/// Events from outside the window.
#[derive(Debug)]
enum UserEvent {
    /// A menu item was chosen.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Menu(String),
}

/// Where everything goes in the window, in logical pixels. As in
/// HaikuPaint: tools above the line palette on the left, patterns below
/// the page.
struct Layout {
    tools: Point,
    lines: Point,
    page: Rect,
    patterns: Point,
}

impl Layout {
    fn new(width: i32, height: i32) -> Self {
        let page_left = MARGIN + tools::WIDTH + 1 + MARGIN;
        let pattern_height = patterns::HEIGHT + 1;
        let page_bottom = (height - MARGIN - pattern_height - MARGIN).max(MARGIN + 1);
        let patterns = Point::new(page_left, page_bottom + MARGIN);
        Self {
            tools: Point::new(MARGIN, TOP + MARGIN),
            lines: Point::new(
                MARGIN,
                patterns.y + pattern_height - (line_widths::HEIGHT + 1),
            ),
            page: Rect::new(
                page_left,
                TOP + MARGIN,
                (width - MARGIN).max(page_left + 1),
                page_bottom.max(TOP + MARGIN + 1),
            ),
            patterns,
        }
    }

    /// The smallest window everything fits in.
    fn minimum_size() -> LogicalSize<i32> {
        let width = MARGIN + tools::WIDTH + 1 + MARGIN + patterns::WIDTH + 1 + MARGIN;
        let height = TOP + MARGIN + tools::HEIGHT + 1 + MARGIN + line_widths::HEIGHT + 1 + MARGIN;
        LogicalSize::new(width, height)
    }
}

/// What the cursor looks like.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CursorKind {
    Arrow,
    Tool(Tool),
    /// The line and shape tools' crosshair, one per line size.
    Line(usize),
}

/// What a press is dragging, until the button comes up.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Capture {
    None,
    Page,
    Menu,
}

/// Something that reacts to a double-click.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClickTarget {
    Tool(Tool),
}

struct Graphics {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    cursors: HashMap<CursorKind, CustomCursor>,
    cursor: Option<CursorKind>,
}

struct App {
    graphics: Option<Graphics>,
    view: PageView,
    patterns: [Pattern; PATTERN_COUNT],
    pattern_index: usize,

    // The palettes, rendered when they change.
    tool_palette: Canvas,
    line_palette: Canvas,
    pattern_palette: Canvas,

    /// Logical pixels per physical pixel's worth: a whole number.
    scale: u32,
    layout: Layout,
    /// The page position at the top left of the view, kept fractional so
    /// trackpads scroll smoothly.
    scroll: (f64, f64),
    mouse: Point,
    modifiers: ModifiersState,
    capture: Capture,
    last_click: Option<(Instant, ClickTarget)>,
    next_ants: Instant,

    clipboard: Clipboard,
    /// The drawn menu bar, where there are no native menus.
    menu_bar: Option<menu_bar::MenuBar>,
    #[cfg(target_os = "macos")]
    menus: Option<menus::MenuBar>,

    /// The file Save writes to; only set for MacPaint documents, so an
    /// opened PNG is never overwritten in MacPaint format.
    path: Option<PathBuf>,
    /// The document's name, in the title bar.
    name: String,
    quit_requested: bool,
}

impl App {
    fn new() -> Self {
        let view = PageView::new();
        let pattern_index = DEFAULT_PATTERN_INDEX;
        Self {
            tool_palette: tools::render(view.tool()),
            line_palette: line_widths::render(view.line_width(), view.border()),
            pattern_palette: patterns::render(&PATTERNS, pattern_index),
            view,
            patterns: PATTERNS,
            pattern_index,
            graphics: None,
            scale: 1,
            layout: Layout::new(0, 0),
            scroll: (0.0, 0.0),
            mouse: Point::default(),
            modifiers: ModifiersState::empty(),
            capture: Capture::None,
            last_click: None,
            next_ants: Instant::now(),
            clipboard: Clipboard::new(),
            menu_bar: DRAWN_MENUS.then(menu_bar::MenuBar::new),
            #[cfg(target_os = "macos")]
            menus: None,
            path: None,
            name: UNTITLED.into(),
            quit_requested: false,
        }
    }

    /// MacPaint's modifier keys from the keyboard's.
    fn modifiers(&self) -> Modifiers {
        let state = self.modifiers;
        if cfg!(target_os = "macos") {
            Modifiers {
                shift: state.shift_key(),
                option: state.alt_key(),
                command: state.super_key(),
            }
        } else {
            Modifiers {
                shift: state.shift_key(),
                option: state.control_key(),
                command: state.alt_key(),
            }
        }
    }

    fn scroll_origin(&self) -> Point {
        Point::new(self.scroll.0 as i32, self.scroll.1 as i32)
    }

    /// Keeps the page in view.
    fn clamp_scroll(&mut self) {
        let page = self.view.canvas();
        let max_x = (page.width() - self.layout.page.width()).max(0) as f64;
        let max_y = (page.height() - self.layout.page.height()).max(0) as f64;
        self.scroll = (
            self.scroll.0.clamp(0.0, max_x),
            self.scroll.1.clamp(0.0, max_y),
        );

        // Brush Mirrors reflect around the middle of what's visible.
        let origin = self.scroll_origin();
        let (page, view) = (self.view.canvas(), &self.layout.page);
        let visible = Rect::new(
            origin.x,
            origin.y,
            (origin.x + view.width()).min(page.width()),
            (origin.y + view.height()).min(page.height()),
        );
        self.view.set_visible(visible);
    }

    /// The page point under a window point.
    fn page_point(&self, point: Point) -> Point {
        let origin = self.scroll_origin();
        Point::new(
            point.x - self.layout.page.left + origin.x,
            point.y - self.layout.page.top + origin.y,
        )
    }

    fn in_page_view(&self, point: Point) -> bool {
        let page = &self.layout.page;
        point.x >= page.left && point.x < page.right && point.y >= page.top && point.y < page.bottom
    }

    fn relayout(&mut self) {
        let Some(graphics) = &self.graphics else {
            return;
        };
        let size = graphics.window.inner_size();
        self.scale = (graphics.window.scale_factor().round() as u32).max(1);
        self.layout = Layout::new(
            (size.width / self.scale) as i32,
            (size.height / self.scale) as i32,
        );
        self.clamp_scroll();
    }

    fn set_tool(&mut self, tool: Tool) {
        self.view.set_tool(tool);
        self.tool_palette = tools::render(tool);
    }

    fn set_pattern(&mut self, index: usize) {
        self.pattern_index = index;
        self.view.set_pattern(self.patterns[index]);
        self.pattern_palette = patterns::render(&self.patterns, index);
    }

    fn set_line_width(&mut self, width: i32, border: bool) {
        self.view.set_line_width(width, border);
        self.line_palette = line_widths::render(width, border);
    }

    /// Whether this click on target completes a double-click.
    fn is_double_click(&mut self, target: ClickTarget) -> bool {
        let now = Instant::now();
        let double = matches!(self.last_click,
            Some((time, last)) if last == target && now - time < DOUBLE_CLICK_TIME);
        self.last_click = if double { None } else { Some((now, target)) };
        double
    }

    fn mouse_down(&mut self) {
        let point = self.mouse;
        let state = self.menu_state();
        if let Some(bar) = &mut self.menu_bar
            && bar.mouse_down(point, state)
        {
            self.capture = Capture::Menu;
            return;
        }
        let at = |origin: Point| (point.x - origin.x, point.y - origin.y);

        let (x, y) = at(self.layout.tools);
        if let Some(tool) = tools::hit(x, y) {
            let double = self.is_double_click(ClickTarget::Tool(tool));
            // Double-clicking the eraser clears the page, as in MacPaint.
            if double && tool == Tool::Eraser {
                self.view.clear();
                return;
            }
            self.set_tool(tool);
            // Double-clicking the marquee selects the whole window.
            if double && tool == Tool::Select {
                self.view.select_all();
            }
            return;
        }
        self.last_click = None;

        let (x, y) = at(self.layout.lines);
        if let Some((width, border)) = line_widths::hit(x, y) {
            self.set_line_width(width, border);
            return;
        }
        let (x, y) = at(self.layout.patterns);
        if let Some(index) = patterns::hit(x, y) {
            self.set_pattern(index);
            return;
        }
        if self.in_page_view(point) {
            self.capture = Capture::Page;
            self.view
                .mouse_down(self.page_point(point), self.modifiers());
        }
    }

    fn key_down(&mut self, key: &Key) {
        // Menu shortcuts: Command on macOS, where the menus mostly take them
        // before the window sees them, and Ctrl elsewhere.
        let state = self.modifiers;
        let shortcut = if cfg!(target_os = "macos") {
            state.super_key()
        } else {
            state.control_key()
        };
        if shortcut {
            if let Key::Character(text) = key
                && let Some(command) = text
                    .chars()
                    .next()
                    .and_then(|key| Command::from_shortcut(key, state.shift_key()))
            {
                self.run(command);
            }
            return;
        }
        let delete = matches!(key, Key::Named(NamedKey::Backspace | NamedKey::Delete));
        self.view.key_down(delete);
    }

    fn run(&mut self, command: Command) {
        let selection_command = |command| match command {
            Command::Invert => Some(SelectionCommand::Invert),
            Command::Fill => Some(SelectionCommand::Fill),
            Command::TraceEdges => Some(SelectionCommand::TraceEdges),
            Command::FlipHorizontal => Some(SelectionCommand::FlipHorizontal),
            Command::FlipVertical => Some(SelectionCommand::FlipVertical),
            Command::Rotate => Some(SelectionCommand::Rotate),
            _ => None,
        };
        match command {
            Command::New => {
                if self.confirm_discard() {
                    self.new_document();
                }
            }
            Command::Open => self.open(),
            Command::Save => {
                self.save();
            }
            Command::SaveAs => {
                self.save_as();
            }
            Command::Revert => self.revert(),
            Command::ExportPng => self.export(),
            Command::Quit => {
                if self.confirm_discard() {
                    self.quit_requested = true;
                }
            }
            Command::Undo => self.view.undo(),
            Command::Cut | Command::Copy => {
                if let Some(selection) = self.view.selection() {
                    self.clipboard.copy(selection);
                    if command == Command::Cut {
                        self.view.clear_selection();
                    }
                }
            }
            Command::Paste => {
                let page = self.view.canvas();
                if let Some(pasted) = self.clipboard.paste(page.width(), page.height()) {
                    self.view.paste(pasted);
                }
            }
            Command::Clear => self.view.clear(),
            Command::SelectAll => self.view.select_all(),
            _ => {
                if let Some(mut operation) = selection_command(command) {
                    // As in MacPaint, Shift makes Trace Edges add a shadow.
                    if operation == SelectionCommand::TraceEdges && self.modifiers().shift {
                        operation = SelectionCommand::TraceEdgesShadow;
                    }
                    self.view.apply_to_selection(operation);
                }
            }
        }
        self.request_redraw();
    }

    fn menu_state(&self) -> MenuState {
        MenuState {
            can_undo: self.view.can_undo(),
            has_selection: self.view.has_selection(),
            can_revert: self.path.is_some() && self.view.is_dirty(),
        }
    }

    fn window(&self) -> Option<&Window> {
        self.graphics.as_ref().map(|graphics| &*graphics.window)
    }

    fn file_dialog(&self) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new();
        match self.window() {
            Some(window) => dialog.set_parent(window),
            None => dialog,
        }
    }

    fn message_dialog(&self, title: &str, text: &str) -> rfd::MessageDialog {
        let dialog = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(text);
        match self.window() {
            Some(window) => dialog.set_parent(window),
            None => dialog,
        }
    }

    fn show_error(&self, text: &str) {
        self.message_dialog("Error", text)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }

    /// Asks about unsaved changes. Returns whether the caller may go
    /// ahead.
    fn confirm_discard(&mut self) -> bool {
        if !self.view.is_dirty() {
            return true;
        }
        let text = format!("Save changes to the document \"{}\"?", self.name);
        let dialog = self
            .message_dialog("Unsaved changes", &text)
            .set_level(rfd::MessageLevel::Warning);
        // Windows needs a manifest for custom labels; Yes and No stand in.
        let buttons = if cfg!(target_os = "windows") {
            rfd::MessageButtons::YesNoCancel
        } else {
            rfd::MessageButtons::YesNoCancelCustom(
                "Save".into(),
                "Don't Save".into(),
                "Cancel".into(),
            )
        };
        match dialog.set_buttons(buttons).show() {
            rfd::MessageDialogResult::Yes => self.save(),
            rfd::MessageDialogResult::No => true,
            rfd::MessageDialogResult::Custom(label) if label == "Save" => self.save(),
            rfd::MessageDialogResult::Custom(label) if label == "Don't Save" => true,
            _ => false,
        }
    }

    fn new_document(&mut self) {
        self.view.set_image(Canvas::default());
        self.set_patterns(PATTERNS);
        self.path = None;
        self.set_name(UNTITLED.into());
    }

    fn open(&mut self) {
        if !self.confirm_discard() {
            return;
        }
        let path = self
            .file_dialog()
            .add_filter("MacPaint documents", &MACPAINT_EXTENSIONS)
            .add_filter(
                "Images",
                &[
                    "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp", "pbm", "pgm", "ppm",
                ],
            )
            .add_filter("All files", &["*"])
            .pick_file();
        if let Some(path) = path {
            self.open_path(&path);
        }
    }

    fn open_path(&mut self, path: &Path) {
        match load_document(path) {
            Ok(document) => {
                self.view.set_image(document.canvas);
                self.set_patterns(document.patterns);
                self.path = document.is_macpaint.then(|| path.to_path_buf());
                self.set_name(file_name(path));
            }
            Err(error) => self.show_error(&error),
        }
    }

    /// Saves to the document's file, or asks where. Returns whether it was
    /// saved.
    fn save(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            return self.save_as();
        };
        match save_macpaint(&path, self.view.canvas(), &self.patterns) {
            Ok(()) => {
                self.view.set_dirty(false);
                true
            }
            Err(error) => {
                self.show_error(&error);
                false
            }
        }
    }

    fn save_as(&mut self) -> bool {
        // Suggest a MacPaint name, also for an image opened from another
        // format.
        let name = if self.path.is_some() {
            self.name.clone()
        } else {
            format!("{}.mac", stem(&self.name))
        };
        let Some(path) = self
            .file_dialog()
            .set_file_name(name)
            .add_filter("MacPaint document", &MACPAINT_EXTENSIONS)
            .save_file()
        else {
            return false;
        };
        if let Err(error) = save_macpaint(&path, self.view.canvas(), &self.patterns) {
            self.show_error(&error);
            return false;
        }
        self.view.set_dirty(false);
        self.set_name(file_name(&path));
        self.path = Some(path);
        true
    }

    fn revert(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        // MacPaint's Revert alert.
        let answer = self
            .message_dialog("Revert", "Revert to the last version saved?")
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show();
        if answer == rfd::MessageDialogResult::Ok {
            self.open_path(&path);
        }
    }

    fn export(&mut self) {
        let path = self
            .file_dialog()
            .set_file_name(format!("{}.png", stem(&self.name)))
            .add_filter("PNG image", &["png"])
            .save_file();
        if let Some(path) = path
            && let Err(error) = export_png(&path, self.view.canvas())
        {
            self.show_error(&error);
        }
    }

    /// The document's patterns, saved with it.
    fn set_patterns(&mut self, patterns: [Pattern; PATTERN_COUNT]) {
        self.patterns = patterns;
        self.view.set_pattern(patterns[self.pattern_index]);
        self.pattern_palette = patterns::render(&self.patterns, self.pattern_index);
    }

    fn set_name(&mut self, name: String) {
        if let Some(window) = self.window() {
            window.set_title(&name);
        }
        self.name = name;
    }

    fn request_redraw(&self) {
        if let Some(graphics) = &self.graphics {
            graphics.window.request_redraw();
        }
    }

    fn mouse_up(&mut self) {
        match self.capture {
            Capture::Page => {
                self.view
                    .mouse_up(self.page_point(self.mouse), self.modifiers());
            }
            Capture::Menu => {
                let state = self.menu_state();
                let command = self
                    .menu_bar
                    .as_mut()
                    .and_then(|bar| bar.mouse_up(self.mouse, state));
                if let Some(command) = command {
                    self.run(command);
                }
            }
            Capture::None => {}
        }
        self.capture = Capture::None;
    }

    fn cursor_kind(&self) -> CursorKind {
        if self.menu_bar.as_ref().is_some_and(|bar| bar.is_open()) {
            return CursorKind::Arrow;
        }
        if self.capture != Capture::Page && !self.in_page_view(self.mouse) {
            return CursorKind::Arrow;
        }
        if self.view.shows_arrow(self.page_point(self.mouse)) {
            return CursorKind::Arrow;
        }
        match self.view.tool() {
            Tool::Line
            | Tool::Rect
            | Tool::FilledRect
            | Tool::RoundRect
            | Tool::FilledRoundRect
            | Tool::Oval
            | Tool::FilledOval
            | Tool::Freeform
            | Tool::FilledFreeform
            | Tool::Polygon
            | Tool::FilledPolygon => {
                // A crosshair as thick as the line.
                let width = self.view.line_width();
                CursorKind::Line(match width {
                    8.. => 3,
                    4.. => 2,
                    2.. => 1,
                    _ => 0,
                })
            }
            tool => CursorKind::Tool(tool),
        }
    }

    fn update_cursor(&mut self) {
        let kind = self.cursor_kind();
        let Some(graphics) = &mut self.graphics else {
            return;
        };
        if graphics.cursor == Some(kind) {
            return;
        }
        graphics.cursor = Some(kind);
        match graphics.cursors.get(&kind) {
            Some(cursor) => graphics.window.set_cursor(cursor.clone()),
            None => graphics.window.set_cursor(CursorIcon::Default),
        }
    }

    /// Composes the window at logical resolution.
    fn render(&self, frame: &mut [u32], width: i32, height: i32) {
        frame.fill(PANEL_COLOR);
        for (canvas, origin) in [
            (&self.tool_palette, self.layout.tools),
            (&self.line_palette, self.layout.lines),
            (&self.pattern_palette, self.layout.patterns),
        ] {
            blit(frame, width, height, canvas, origin, true);
        }

        let view = &self.layout.page;
        let page = self.view.canvas();
        let origin = self.scroll_origin();
        for y in view.top.max(0)..view.bottom.min(height) {
            for x in view.left.max(0)..view.right.min(width) {
                let (px, py) = (x - view.left + origin.x, y - view.top + origin.y);
                frame[(y * width + x) as usize] = if !page.contains(px, py) {
                    OUTSIDE_PAGE_COLOR
                } else if page.get(px, py) {
                    BLACK
                } else {
                    WHITE
                };
            }
        }

        // The overlay goes on top, on screen only.
        let (overlay, ants, phase) = self.view.overlay();
        for point in overlay {
            let (x, y) = (
                point.x - origin.x + view.left,
                point.y - origin.y + view.top,
            );
            if page.contains(point.x, point.y)
                && x >= view.left.max(0)
                && x < view.right.min(width)
                && y >= view.top.max(0)
                && y < view.bottom.min(height)
            {
                let black = !ants || ant_bit(point.x, point.y, phase);
                frame[(y * width + x) as usize] = if black { BLACK } else { WHITE };
            }
        }

        // The menus go over everything.
        if let Some(bar) = &self.menu_bar {
            blit(
                frame,
                width,
                height,
                &bar.render_bar(width),
                Point::default(),
                false,
            );
            if let Some((origin, menu)) = bar.render_menu() {
                blit(frame, width, height, &menu, origin, true);
            }
        }
    }

    fn redraw(&mut self) {
        let scale = self.scale as usize;
        let menu_state = self.menu_state();
        let mut logical = Vec::new();
        let Some(graphics) = &mut self.graphics else {
            return;
        };
        let size = graphics.window.inner_size();
        let (Some(physical_width), Some(physical_height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        graphics
            .surface
            .resize(physical_width, physical_height)
            .expect("resizing the surface");

        let (physical_width, physical_height) = (size.width as usize, size.height as usize);
        let (width, height) = (physical_width / scale, physical_height / scale);
        logical.resize(width * height, PANEL_COLOR);
        self.render(&mut logical, width as i32, height as i32);

        // Scale up by whole pixels; any leftover edge shows the panel.
        let Some(graphics) = &mut self.graphics else {
            return;
        };
        let mut buffer = graphics
            .surface
            .buffer_mut()
            .expect("getting the frame buffer");
        buffer.fill(PANEL_COLOR);
        for y in 0..height {
            let source = &logical[y * width..(y + 1) * width];
            let first = y * scale * physical_width;
            let row = &mut buffer[first..first + physical_width];
            for (x, &pixel) in source.iter().enumerate() {
                row[x * scale..(x + 1) * scale].fill(pixel);
            }
            for copy in 1..scale {
                buffer.copy_within(first..first + physical_width, first + copy * physical_width);
            }
        }

        // The menus' text goes on at full resolution.
        if let Some(bar) = &self.menu_bar {
            bar.draw_text(
                &mut buffer,
                physical_width,
                physical_height,
                self.scale,
                menu_state,
            );
        }
        buffer.present().expect("presenting the frame");
    }

    fn create_cursors(&mut self, event_loop: &ActiveEventLoop) {
        let Some(graphics) = &mut self.graphics else {
            return;
        };
        // macOS sizes cursors in points and scales them itself; elsewhere
        // they're in physical pixels.
        let scale = if cfg!(target_os = "macos") {
            1
        } else {
            self.scale as usize
        };

        // Glyphs, masks and hotspots from MacPaint's SetToolCursor. Cursors
        // without a mask use 0, a character PaintFont doesn't have.
        let mut specs: Vec<(CursorKind, i32, i32, i32, i32)> = vec![
            (
                CursorKind::Tool(Tool::Pencil),
                'L' as i32,
                GLYPH_PENCIL_MASK,
                3,
                16,
            ),
            (
                CursorKind::Tool(Tool::PaintBucket),
                'I' as i32,
                GLYPH_FILL_MASK,
                13,
                16,
            ),
            (
                CursorKind::Tool(Tool::Eraser),
                GLYPH_ERASER_BOX,
                GLYPH_ALL_ONES,
                8,
                8,
            ),
            (CursorKind::Tool(Tool::Spray), GLYPH_SPRAY, 0, 8, 8),
            (
                CursorKind::Tool(Tool::Brush),
                GLYPH_FIRST_BRUSH + DEFAULT_BRUSH,
                0,
                8,
                8,
            ),
            (CursorKind::Tool(Tool::Lasso), 'E' as i32, 0, 2, 15),
            (CursorKind::Tool(Tool::Select), 'r' as i32, 's' as i32, 8, 8),
            (
                CursorKind::Tool(Tool::Grabber),
                'G' as i32,
                'm' as i32,
                8,
                8,
            ),
            (CursorKind::Tool(Tool::Text), 'n' as i32, 0, 8, 13),
        ];
        let line_glyphs = [
            GLYPH_LINE_CURSOR_1,
            GLYPH_LINE_CURSOR_2,
            GLYPH_LINE_CURSOR_4,
            GLYPH_LINE_CURSOR_8,
        ];
        for (index, glyph) in line_glyphs.into_iter().enumerate() {
            specs.push((CursorKind::Line(index), glyph, 0, 8, 8));
        }

        graphics.cursors.clear();
        graphics.cursor = None;
        let side = (CURSOR_SIZE * scale) as u16;
        for (kind, data, mask, hot_x, hot_y) in specs {
            // The Mac allowed a hotspot one past the edge (the pencil's is
            // at v = 16); clamp it onto the cursor.
            let hot = |value: i32| (value.min(CURSOR_SIZE as i32 - 1) as usize * scale) as u16;
            let rgba = art::glyph_cursor_rgba(data, mask, scale);
            if let Ok(source) = CustomCursor::from_rgba(rgba, side, side, hot(hot_x), hot(hot_y)) {
                graphics
                    .cursors
                    .insert(kind, event_loop.create_custom_cursor(source));
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        let page_height = Canvas::DEFAULT_HEIGHT;
        let attributes = Window::default_attributes()
            .with_title("untitled")
            .with_inner_size(LogicalSize::new(
                Layout::minimum_size()
                    .width
                    .max(Layout::new(0, 0).page.left + Canvas::DEFAULT_WIDTH + MARGIN),
                (MARGIN + page_height * 7 / 8 + MARGIN + patterns::HEIGHT + 1 + MARGIN).min(900),
            ))
            .with_min_inner_size(Layout::minimum_size());
        let window = Rc::new(
            event_loop
                .create_window(attributes)
                .expect("creating the window"),
        );
        let context = Context::new(window.clone()).expect("creating the graphics context");
        let surface = Surface::new(&context, window.clone()).expect("creating the surface");
        self.graphics = Some(Graphics {
            window,
            surface,
            cursors: HashMap::new(),
            cursor: None,
        });
        self.relayout();
        self.create_cursors(event_loop);

        #[cfg(target_os = "macos")]
        {
            let menus = menus::MenuBar::new();
            menus.install();
            self.menus = Some(menus);
        }

        // A document named on the command line.
        if let Some(path) = std::env::args_os().nth(1) {
            self.open_path(Path::new(&path));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let mut changed = true;
        match event {
            WindowEvent::CloseRequested => {
                if self.confirm_discard() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => self.relayout(),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.relayout();
                self.create_cursors(event_loop);
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                changed = false;
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                changed = false;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.scale as f64;
                self.mouse = Point::new(
                    (position.x / scale).floor() as i32,
                    (position.y / scale).floor() as i32,
                );
                // A polygon's next edge follows the pointer with the button
                // up.
                if let Some(bar) = &mut self.menu_bar
                    && bar.is_open()
                {
                    let state = self.menu_state();
                    if let Some(bar) = &mut self.menu_bar {
                        bar.mouse_moved(self.mouse, state);
                    }
                    self.update_cursor();
                    self.request_redraw();
                    return;
                }
                changed = self.capture == Capture::Page || self.view.polygon_active();
                if changed {
                    self.view
                        .mouse_moved(self.page_point(self.mouse), self.modifiers());
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed if self.capture == Capture::None => self.mouse_down(),
                ElementState::Released => self.mouse_up(),
                _ => {}
            },
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !event.repeat {
                    self.key_down(&event.logical_key);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (x as f64 * LINE_SCROLL, y as f64 * LINE_SCROLL)
                    }
                    MouseScrollDelta::PixelDelta(position) => (
                        position.x / self.scale as f64,
                        position.y / self.scale as f64,
                    ),
                };
                self.scroll = (self.scroll.0 - dx, self.scroll.1 - dy);
                self.clamp_scroll();
                // Keep drawing under the pointer as the page moves.
                if self.capture == Capture::Page {
                    self.view
                        .mouse_moved(self.page_point(self.mouse), self.modifiers());
                }
            }
            _ => changed = false,
        }
        self.update_cursor();
        if changed {
            self.request_redraw();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            #[cfg(target_os = "macos")]
            UserEvent::Menu(id) => {
                let command = self
                    .menus
                    .as_ref()
                    .and_then(|menus| menus.command(&id.into()));
                if let Some(command) = command {
                    self.run(command);
                }
            }
            #[cfg(not(target_os = "macos"))]
            UserEvent::Menu(_) => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit_requested {
            event_loop.exit();
            return;
        }
        #[cfg(target_os = "macos")]
        if let Some(menus) = &self.menus {
            menus.update(self.menu_state());
        }

        // March the ants while there are any.
        if !self.view.has_ants() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        if now >= self.next_ants {
            self.view.march_ants();
            self.request_redraw();
            self.next_ants = now + ANTS_INTERVAL;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_ants));
    }
}

fn main() {
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        // RustPaint installs its own menus.
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    let event_loop = builder.build().expect("starting the event loop");
    #[cfg(target_os = "macos")]
    {
        let proxy = event_loop.create_proxy();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(event.id.0));
        }));
    }
    let mut app = App::new();
    event_loop
        .run_app(&mut app)
        .expect("running the event loop");
}

/// The file's name, for the title bar.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| UNTITLED.into())
}

/// A name without its extension.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

/// Draws a 1-bit canvas into the frame at origin. A shadowed canvas is a
/// palette box (see art::draw_palette_box): the two corners its drop
/// shadow doesn't reach show the background.
fn blit(
    frame: &mut [u32],
    width: i32,
    height: i32,
    canvas: &Canvas,
    origin: Point,
    shadowed: bool,
) {
    let (w, h) = (canvas.width(), canvas.height());
    for y in 0..h {
        for x in 0..w {
            if shadowed && ((x == w - 1 && y == 0) || (x == 0 && y == h - 1)) {
                continue;
            }
            let (fx, fy) = (origin.x + x, origin.y + y);
            if fx >= 0 && fy >= 0 && fx < width && fy < height {
                frame[(fy * width + fx) as usize] = if canvas.get(x, y) { BLACK } else { WHITE };
            }
        }
    }
}
