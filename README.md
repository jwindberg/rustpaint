# RustPaint

Bill Atkinson's 1984 MacPaint, re-created in Rust for macOS, Linux and
Windows. It's a port of [HaikuPaint](https://github.com/jwindberg/HaikuPaint),
a complete MacPaint re-creation for Haiku, and does what it does: the 576 × 720
one-bit page, MacPaint's tools, patterns, brushes and cursors, FatBits, Show
Page, the Goodies, the text tool, and MacPaint documents in and out.

## Using it

Everything works as it did in MacPaint. A few things to know:

- **Modifier keys.** On macOS, Option and Command do what they did on the Mac:
  Option draws lines and borders in the current pattern and copies selections
  as you drag them; Command paints in OR mode and stretches selections. On
  Linux and Windows, Ctrl stands in for Option and Alt for Command, while menu
  shortcuts use Ctrl.
- **Double-clicks**, as in MacPaint: the eraser clears the page, the marquee
  selects the whole window, the pencil toggles FatBits, the brush opens Brush
  Shape, the grabber opens Show Page, and a pattern opens Edit Pattern.
- **Files.** RustPaint saves MacPaint documents (`.mac`), with the document's
  own patterns, and opens them with or without a MacBinary header. Other
  images open in dithered black and white, and File › Export as PNG writes a
  PNG.
- **Printing** writes a PDF of the page, at MacPaint's real 8 × 10 inches,
  and opens it in your PDF viewer to print. Print Catalog does the same for
  every MacPaint document in a folder, 25 to a page.
- **Text** uses your system's fonts, drawn one bit deep.
- **Languages:** English, German, French, Spanish and Swedish, from the
  system's settings. `RUSTPAINT_LANGUAGE=de` picks one.

## Building

```sh
cargo run --release -p rustpaint
cargo test
```

The workspace has two crates: `crates/engine`, the drawing engine, with no
dependencies and every test, and `crates/app`, the window. `PLAN.md` describes
how it's put together.

## Credits

- **MacPaint** was written by Bill Atkinson and designed with Susan Kare.
  MacPaint's artwork in RustPaint (its patterns, the PaintFont glyphs that make
  the tool icons, cursors and brushes, and its icons) is **© Apple**. It isn't
  copied by hand: `tools/extract_macpaint_art.py` extracts it from the
  original MacPaint disk.
- **Inter**, the menu font on Linux and Windows, is by the Inter Project
  Authors, under the SIL Open Font License (`assets/fonts/OFL-Inter.txt`).
- The translations are HaikuPaint's.

See `ATTRIBUTION.md` for the details.

## License

RustPaint's code is MIT licensed (`LICENSE`), copyright 2026 jwindberg. The
MacPaint artwork and Inter are not covered by it; see above.
