//! Menu commands, shared by the native menus, the drawn menu bar and the
//! keyboard shortcuts.

use crate::i18n::{tr, tr_args};
use crate::typesetter::{Alignment, FONT_SIZES};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    // File.
    About,
    New,
    Open,
    Save,
    SaveAs,
    Revert,
    ExportPng,
    Print,
    PrintCatalog,
    Quit,
    // Edit.
    Undo,
    Cut,
    Copy,
    Paste,
    Clear,
    SelectAll,
    Invert,
    Fill,
    TraceEdges,
    FlipHorizontal,
    FlipVertical,
    Rotate,
    // Goodies.
    Grid,
    FatBits,
    ShowPage,
    EditPattern,
    BrushShape,
    BrushMirrors,
    // Font, FontSize and Style.
    /// One of the installed families, by its place in the Font menu.
    Font(usize),
    /// One of `FONT_SIZES`, by its place.
    FontSize(usize),
    Face(Face),
    Align(Alignment),
}

/// The Style menu's faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Plain,
    Bold,
    Italic,
    Underline,
    Outline,
    Shadow,
}

/// A menu: its title and its commands, in groups between separators.
pub struct MenuSpec {
    pub title: String,
    pub groups: Vec<Vec<Command>>,
}

/// HaikuPaint's menus: File (with About, as HaikuPaint's; on macOS it goes
/// in the application menu, with Quit), MacPaint's
/// Edit menu plus Select All, MacPaint's Goodies (without its Introduction
/// and Short Cuts screens), and Font, FontSize and Style. The Font menu
/// lists `font_count` families.
pub fn menus(font_count: usize) -> Vec<MenuSpec> {
    use Command::*;
    vec![
        MenuSpec {
            title: tr("menu-file"),
            groups: vec![
                vec![About],
                vec![New, Open],
                vec![Save, SaveAs, Revert, ExportPng],
                vec![Print, PrintCatalog],
                vec![Quit],
            ],
        },
        MenuSpec {
            title: tr("menu-edit"),
            groups: vec![
                vec![Undo],
                vec![Cut, Copy, Paste, Clear, SelectAll],
                vec![
                    Invert,
                    Fill,
                    TraceEdges,
                    FlipHorizontal,
                    FlipVertical,
                    Rotate,
                ],
            ],
        },
        MenuSpec {
            title: tr("menu-goodies"),
            groups: vec![
                vec![Grid, FatBits, ShowPage],
                vec![EditPattern, BrushShape, BrushMirrors],
            ],
        },
        MenuSpec {
            title: tr("menu-font"),
            groups: vec![(0..font_count).map(Font).collect()],
        },
        MenuSpec {
            title: tr("menu-font-size"),
            groups: vec![(0..FONT_SIZES.len()).map(FontSize).collect()],
        },
        MenuSpec {
            title: tr("menu-style"),
            groups: vec![
                [
                    self::Face::Plain,
                    self::Face::Bold,
                    self::Face::Italic,
                    self::Face::Underline,
                    self::Face::Outline,
                    self::Face::Shadow,
                ]
                .into_iter()
                .map(Command::Face)
                .collect(),
                [Alignment::Left, Alignment::Middle, Alignment::Right]
                    .into_iter()
                    .map(Align)
                    .collect(),
            ],
        },
    ]
}

/// The text style, as the menus show it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMenuState {
    /// The family's place in the Font menu.
    pub font: Option<usize>,
    /// The size's place in `FONT_SIZES`.
    pub size: Option<usize>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub outline: bool,
    pub shadow: bool,
    pub alignment: Alignment,
}

/// What the menus need to know to enable and check their items, as
/// HaikuPaint's MenusBeginning does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    pub can_undo: bool,
    pub has_selection: bool,
    pub can_revert: bool,
    pub grid: bool,
    pub fat_bits: bool,
    /// Any brush mirror is on.
    pub mirrors: bool,
    /// A dialog is open: nothing else can be chosen.
    pub modal: bool,
    pub text: TextMenuState,
}

impl Command {
    /// The commands that don't depend on the installed fonts.
    pub fn all() -> impl Iterator<Item = Command> {
        menus(0)
            .into_iter()
            .flat_map(|menu| menu.groups.into_iter().flatten())
    }

    /// The item's text; Font items are the families' names.
    pub fn label(self, families: &[String]) -> String {
        match self {
            Command::Font(index) => families.get(index).cloned().unwrap_or_default(),
            Command::FontSize(index) => tr_args(
                "font-size-points",
                &[("size", &FONT_SIZES[index].to_string())],
            ),
            command => tr(command.message_id()),
        }
    }

    /// The Fluent message of the item's text (see crate::i18n).
    fn message_id(self) -> &'static str {
        match self {
            Command::About => "about",
            Command::New => "new",
            Command::Open => "open",
            Command::Save => "save",
            Command::SaveAs => "save-as",
            Command::Revert => "revert",
            Command::ExportPng => "export-png",
            Command::Print => "print",
            Command::PrintCatalog => "print-catalog",
            Command::Quit => "quit",
            Command::Undo => "undo",
            Command::Cut => "cut",
            Command::Copy => "copy",
            Command::Paste => "paste",
            Command::Clear => "clear",
            Command::SelectAll => "select-all",
            Command::Invert => "invert",
            Command::Fill => "fill",
            Command::TraceEdges => "trace-edges",
            Command::FlipHorizontal => "flip-horizontal",
            Command::FlipVertical => "flip-vertical",
            Command::Rotate => "rotate",
            Command::Grid => "grid",
            Command::FatBits => "fat-bits",
            Command::ShowPage => "show-page",
            Command::EditPattern => "edit-pattern",
            Command::BrushShape => "brush-shape",
            Command::BrushMirrors => "brush-mirrors",
            Command::Face(Face::Plain) => "plain",
            Command::Face(Face::Bold) => "bold",
            Command::Face(Face::Italic) => "italic",
            Command::Face(Face::Underline) => "underline",
            Command::Face(Face::Outline) => "outline",
            Command::Face(Face::Shadow) => "shadow",
            Command::Align(Alignment::Left) => "align-left",
            Command::Align(Alignment::Middle) => "align-middle",
            Command::Align(Alignment::Right) => "align-right",
            Command::Font(_) | Command::FontSize(_) => "",
        }
    }

    /// The key used with Command (Ctrl on Linux and Windows), and whether
    /// Shift goes with it. The Style menu has MacPaint's, except Outline
    /// and Shadow, whose O and S are Open and Save.
    pub fn shortcut(self) -> Option<(char, bool)> {
        Some(match self {
            Command::New => ('n', false),
            Command::Open => ('o', false),
            Command::Save => ('s', false),
            Command::SaveAs => ('s', true),
            Command::Quit => ('q', false),
            Command::Undo => ('z', false),
            Command::Cut => ('x', false),
            Command::Copy => ('c', false),
            Command::Paste => ('v', false),
            Command::SelectAll => ('a', false),
            Command::TraceEdges => ('e', false),
            Command::Face(Face::Plain) => ('p', false),
            Command::Face(Face::Bold) => ('b', false),
            Command::Face(Face::Italic) => ('i', false),
            Command::Face(Face::Underline) => ('u', false),
            Command::Align(Alignment::Left) => ('l', false),
            Command::Align(Alignment::Middle) => ('m', false),
            Command::Align(Alignment::Right) => ('r', false),
            _ => return None,
        })
    }

    pub fn is_enabled(self, state: MenuState) -> bool {
        if state.modal {
            return false;
        }
        match self {
            Command::Undo => state.can_undo,
            Command::Revert => state.can_revert,
            Command::Cut
            | Command::Copy
            | Command::Invert
            | Command::Fill
            | Command::TraceEdges
            | Command::FlipHorizontal
            | Command::FlipVertical
            | Command::Rotate => state.has_selection,
            _ => true,
        }
    }

    /// Whether the item has a check mark, for the ones that can.
    pub fn is_checked(self, state: MenuState) -> Option<bool> {
        let text = state.text;
        Some(match self {
            Command::Grid => state.grid,
            Command::FatBits => state.fat_bits,
            Command::BrushMirrors => state.mirrors,
            Command::Font(index) => text.font == Some(index),
            Command::FontSize(index) => text.size == Some(index),
            Command::Face(Face::Plain) => {
                !(text.bold || text.italic || text.underline || text.outline || text.shadow)
            }
            Command::Face(Face::Bold) => text.bold,
            Command::Face(Face::Italic) => text.italic,
            Command::Face(Face::Underline) => text.underline,
            Command::Face(Face::Outline) => text.outline,
            Command::Face(Face::Shadow) => text.shadow,
            Command::Align(alignment) => text.alignment == alignment,
            _ => return None,
        })
    }

    /// The command a shortcut stands for. Trace Edges takes Shift too
    /// (for its shadow).
    pub fn from_shortcut(key: char, shift: bool) -> Option<Command> {
        let key = key.to_ascii_lowercase();
        Command::all().find(|command| match command.shortcut() {
            Some((k, _)) if k == key && *command == Command::TraceEdges => true,
            Some((k, s)) => k == key && s == shift,
            None => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_unique_and_found() {
        let commands: Vec<Command> = Command::all().collect();
        assert_eq!(commands.len(), 28 + FONT_SIZES.len() + 6 + 3);
        for command in &commands {
            if let Some((key, shift)) = command.shortcut() {
                assert_eq!(
                    Command::from_shortcut(key.to_ascii_uppercase(), shift),
                    Some(*command)
                );
            }
        }
        assert_eq!(Command::from_shortcut('E', true), Some(Command::TraceEdges));
        assert_eq!(Command::from_shortcut('j', false), None);
    }

    #[test]
    fn items_enable_with_the_state() {
        let none = MenuState::default();
        assert!(!Command::Undo.is_enabled(none) && !Command::Revert.is_enabled(none));
        assert!(!Command::Invert.is_enabled(none) && Command::Paste.is_enabled(none));
        let all = MenuState {
            can_undo: true,
            has_selection: true,
            can_revert: true,
            ..MenuState::default()
        };
        assert!(Command::all().all(|command| command.is_enabled(all)));
        let modal = MenuState { modal: true, ..all };
        assert!(Command::all().all(|command| !command.is_enabled(modal)));
        assert_eq!(
            Command::Grid.is_checked(MenuState { grid: true, ..all }),
            Some(true)
        );
        assert_eq!(Command::Undo.is_checked(all), None);
    }

    #[test]
    fn the_text_menus_follow_the_style() {
        let families = vec!["Geneva".to_string(), "Monaco".to_string()];
        assert_eq!(menus(families.len())[3].groups[0].len(), 2);
        assert_eq!(Command::Font(1).label(&families), "Monaco");
        assert_eq!(Command::FontSize(2).label(&families), "12 point");

        let mut state = MenuState::default();
        state.text.font = Some(1);
        state.text.bold = true;
        assert_eq!(Command::Font(1).is_checked(state), Some(true));
        assert_eq!(Command::Font(0).is_checked(state), Some(false));
        assert_eq!(Command::Face(Face::Plain).is_checked(state), Some(false));
        assert_eq!(Command::Face(Face::Bold).is_checked(state), Some(true));
        assert_eq!(
            Command::Align(Alignment::Left).is_checked(state),
            Some(true)
        );
    }
}
