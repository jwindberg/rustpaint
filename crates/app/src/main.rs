//! RustPaint: Bill Atkinson's MacPaint, re-created. The window draws
//! everything into one 1-bit-looking software framebuffer, as MacPaint did,
//! scaled up by a whole number on HiDPI screens to keep the pixels crisp.

mod art;
mod page_view;
mod palettes;
mod tool;

use std::collections::HashMap;
use std::num::NonZeroU32;
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
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState};
use winit::window::{CursorIcon, CustomCursor, Window, WindowId};

use crate::art::CURSOR_SIZE;
use crate::page_view::{Modifiers, PageView};
use crate::palettes::{line_widths, patterns, tools};
use crate::tool::Tool;

/// Space around and between the palettes and the page.
const MARGIN: i32 = 6;
/// The window's background, and the area beyond the page's edges.
const PANEL_COLOR: u32 = 0xd8d8d8;
const OUTSIDE_PAGE_COLOR: u32 = 0x989898;
const BLACK: u32 = 0x000000;
const WHITE: u32 = 0xffffff;

const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(500);
/// Pixels a mouse wheel notch scrolls.
const LINE_SCROLL: f64 = 16.0;

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
            tools: Point::new(MARGIN, MARGIN),
            lines: Point::new(
                MARGIN,
                patterns.y + pattern_height - (line_widths::HEIGHT + 1),
            ),
            page: Rect::new(
                page_left,
                MARGIN,
                (width - MARGIN).max(page_left + 1),
                page_bottom,
            ),
            patterns,
        }
    }

    /// The smallest window everything fits in.
    fn minimum_size() -> LogicalSize<i32> {
        let width = MARGIN + tools::WIDTH + 1 + MARGIN + patterns::WIDTH + 1 + MARGIN;
        let height = MARGIN + tools::HEIGHT + 1 + MARGIN + line_widths::HEIGHT + 1 + MARGIN;
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
        let at = |origin: Point| (point.x - origin.x, point.y - origin.y);

        let (x, y) = at(self.layout.tools);
        if let Some(tool) = tools::hit(x, y) {
            let double = self.is_double_click(ClickTarget::Tool(tool));
            // Double-clicking the eraser clears the page, as in MacPaint.
            if double && tool == Tool::Eraser {
                self.view.clear();
            } else {
                self.set_tool(tool);
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
        // Command-Z until the Edit menu arrives with milestone 5.
        if self.modifiers().command
            && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("z"))
        {
            self.view.undo();
            return;
        }
        self.view.key_down();
    }

    fn mouse_up(&mut self) {
        if self.capture == Capture::Page {
            self.view
                .mouse_up(self.page_point(self.mouse), self.modifiers());
        }
        self.capture = Capture::None;
    }

    fn cursor_kind(&self) -> CursorKind {
        if self.capture != Capture::Page && !self.in_page_view(self.mouse) {
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
        let mut blit = |canvas: &Canvas, origin: Point| {
            let (w, h) = (canvas.width(), canvas.height());
            for y in 0..h {
                for x in 0..w {
                    // The two corners the drop shadow doesn't reach show
                    // the background.
                    if (x == w - 1 && y == 0) || (x == 0 && y == h - 1) {
                        continue;
                    }
                    let (fx, fy) = (origin.x + x, origin.y + y);
                    if fx >= 0 && fy >= 0 && fx < width && fy < height {
                        frame[(fy * width + fx) as usize] =
                            if canvas.get(x, y) { BLACK } else { WHITE };
                    }
                }
            }
        };
        blit(&self.tool_palette, self.layout.tools);
        blit(&self.line_palette, self.layout.lines);
        blit(&self.pattern_palette, self.layout.patterns);

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
    }

    fn redraw(&mut self) {
        let scale = self.scale as usize;
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

impl ApplicationHandler for App {
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
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let mut changed = true;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
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
        if changed && let Some(graphics) = &self.graphics {
            graphics.window.request_redraw();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("starting the event loop");
    let mut app = App::new();
    event_loop
        .run_app(&mut app)
        .expect("running the event loop");
}
