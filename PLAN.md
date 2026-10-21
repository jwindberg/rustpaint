# RustPaint: plan and handoff

RustPaint re-creates Bill Atkinson's 1984 MacPaint in Rust. It runs on
macOS, Linux and Windows. It's a port of **HaikuPaint**
(`~/Projects/haiku/HaikuPaint`, https://github.com/jwindberg/HaikuPaint,
released as v2.0.0), a complete MacPaint re-creation for Haiku written in
C++. HaikuPaint is the specification: when in doubt, do what it does.

## Where things are
| What | Where |
|---|---|
| HaikuPaint source (the spec) | `~/Projects/haiku/HaikuPaint` |
| Portable C++ engine to port | `HaikuPaint/src/engine/` (~1,750 lines: Canvas, Geometry, Selection, Symmetry, MacPaintFile, Dither, Patterns) |
| Engine tests to port | `HaikuPaint/tests/test_canvas.cpp` (~800 lines, plain asserts) |
| UI behavior to reproduce | `HaikuPaint/src/ui/` (CanvasView.cpp is the heart: every tool, previews, selections, FatBits, text) |
| Roadmap with every feature | `HaikuPaint/docs/ROADMAP.md` |
| Art extractor (Python) | `HaikuPaint/tools/extract_macpaint_art.py` |
| MacPaint 1.x floppy image | `HaikuPaint/reference/macpaint2/Paint_2.dsk` |
| Apple's MacPaint/QuickDraw source (CHM) | `HaikuPaint/reference/macpaint/`, `reference/quickdraw/` |
| Extracted art, as C++ | `HaikuPaint/src/engine/Patterns.cpp`, `src/ui/PaintFont.cpp`, `src/ui/MacIcons.cpp` |
| Translations | `HaikuPaint/locales/*.catkeys` (en, de, fr, es, sv; tab-separated) |

## Rules carried over
- **Licensing:** RustPaint's code is MIT, copyright "2026 jwindberg". The
  MacPaint artwork (patterns, PaintFont glyphs, icons) is © Apple. Generate
  it with the extractor; never hand-copy it, and credit Apple in the README.
  The CHM source is **reference only**: never copy its code or comments,
  and never commit anything from a `reference/` folder (git-ignore it). The
  user chose to publish the artwork publicly in HaikuPaint. The Claude Code
  permission check has blocked Claude from committing Apple artwork before,
  so the user may need to run those commits themselves.
- **Fidelity:** follow MacPaint's own procedures (named in HaikuPaint's
  comments: CreateShape, BrushPaint, CalcMask, DragImage, GetSelPat,
  TraceEdges, SymBrush, ShowPage, PrintCatalog, and so on), geometry
  (palette sizes, offsets) and behaviors. Comments in HaikuPaint cite them.
- **Keys:** on macOS use MacPaint's real Option and Command. On Linux and
  Windows use Ctrl for Option and Alt for Command, as HaikuPaint does.
- **Architecture:** keep a pure engine crate (no GUI dependencies, fully
  unit tested) and a thin app crate that turns events into engine calls.

## Architecture
A Cargo workspace in `~/Projects/rustpaint`:
```
crates/engine/   rustpaint-engine: port of src/engine + tests (no deps);
                 also MacPaint's art (patterns, PaintFont, icons), which the
                 extractor writes to src/generated/ (pure data)
crates/app/      rustpaint: window, menus, palettes, tools, dialogs
assets/          bundled TTF, packaging icons
locales/         Fluent .ftl translations (converted from the catkeys)
tools/           the extractor, changed to emit Rust
```
Draw everything into one software pixel buffer, as MacPaint did. HaikuPaint
already renders its palettes and dialogs pixel by pixel into 1-bit canvases
(see DrawPaletteBox, ToolPalette::_Render, PatternPalette::_Render), so that
code ports directly. Scale the buffer by whole numbers for HiDPI to keep
pixels crisp.

Suggested crates (check the current versions):
| Need | Crate |
|---|---|
| Window, input, custom cursors | `winit` |
| Pixel buffer to window | `softbuffer` (or `pixels`) |
| Native menus | `muda` (Linux needs GTK) |
| File dialogs | `rfd` |
| Clipboard (images) | `arboard` |
| PNG and image import/export | `image` |
| Text tool rendering | `fontdue` with a bundled TTF (1-bit: threshold the coverage) |
| Translations | `fluent` + `fluent-langneg` + `sys-locale` |
| Printing | none good cross-platform: export a PDF (`printpdf`) and open it |

## Milestones (each one shippable, engine work tested)
1. **Engine port:** translate `src/engine/*` to Rust, port every test in
   `test_canvas.cpp`, and get `cargo test` green on macOS. Change the
   extractor to write Rust (`patterns.rs`, `paint_font.rs`, `mac_icons.rs`).
2. **Window and palettes:** a winit window with the 576 × 720 page, the tool,
   line width and pattern palettes rendered as in HaikuPaint, plus the
   pencil, line, eraser and paint bucket, and MacPaint's cursors.
3. **All drawing tools:** shapes, polygons, freeform, brush with the 32
   shapes, spray, Shift constraints, live previews (snapshot and redraw),
   undo, OR mode, and Option to draw in the pattern.
4. **Selections and Edit:** marquee, lasso, marching ants, moves and
   copies, stretch, the clipboard, and Invert, Fill, Trace Edges, Flip and
   Rotate.
5. **Files:** MacPaint documents (open and save, MacBinary), PNG export,
   dithered import, New, Open, Save, Revert, unsaved-changes prompts, and
   native menus.
6. **Goodies and text:** Grid, FatBits, Show Page, Edit Pattern, Brush Shape,
   Brush Mirrors, the grabber, and the text tool with Font, FontSize and
   Style.
7. **Release:** translations (the HaikuPaint catkeys converted to Fluent),
   PDF printing and catalog, an About box with the BILL ICON, GitHub Actions
   CI building and testing on macOS, Linux and Windows, packages (.app,
   .msi or .exe, AppImage), and a GitHub release.

## Testing
- `cargo test` for the engine, on every change.
- The GUI can be run and checked on this Mac. Linux and Windows get built
  and tested in CI, but someone has to try the GUI there by hand. A
  Raspberry Pi (`deckard.local`, Debian aarch64, key-based SSH as
  `jwindberg`) is available for trying the Linux build.
