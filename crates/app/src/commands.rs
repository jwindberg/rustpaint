//! Menu commands, shared by the native menus and the keyboard shortcuts.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    // File.
    New,
    Open,
    Save,
    SaveAs,
    Revert,
    ExportPng,
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
}

/// A menu: its title and its commands, in groups between separators.
pub struct MenuSpec {
    pub title: &'static str,
    pub groups: &'static [&'static [Command]],
}

/// HaikuPaint's File menu (printing comes with milestone 7), and MacPaint's
/// Edit menu plus Select All.
pub const MENUS: &[MenuSpec] = &[
    MenuSpec {
        title: "File",
        groups: &[
            &[Command::New, Command::Open],
            &[
                Command::Save,
                Command::SaveAs,
                Command::Revert,
                Command::ExportPng,
            ],
            &[Command::Quit],
        ],
    },
    MenuSpec {
        title: "Edit",
        groups: &[
            &[Command::Undo],
            &[
                Command::Cut,
                Command::Copy,
                Command::Paste,
                Command::Clear,
                Command::SelectAll,
            ],
            &[
                Command::Invert,
                Command::Fill,
                Command::TraceEdges,
                Command::FlipHorizontal,
                Command::FlipVertical,
                Command::Rotate,
            ],
        ],
    },
];

/// What the menus need to know to enable their items, as HaikuPaint's
/// MenusBeginning does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    pub can_undo: bool,
    pub has_selection: bool,
    pub can_revert: bool,
}

impl Command {
    pub fn all() -> impl Iterator<Item = Command> {
        MENUS
            .iter()
            .flat_map(|menu| menu.groups.iter().flat_map(|group| group.iter().copied()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Command::New => "New",
            Command::Open => "Open…",
            Command::Save => "Save",
            Command::SaveAs => "Save As…",
            Command::Revert => "Revert",
            Command::ExportPng => "Export as PNG…",
            Command::Quit => "Quit",
            Command::Undo => "Undo",
            Command::Cut => "Cut",
            Command::Copy => "Copy",
            Command::Paste => "Paste",
            Command::Clear => "Clear",
            Command::SelectAll => "Select All",
            Command::Invert => "Invert",
            Command::Fill => "Fill",
            Command::TraceEdges => "Trace Edges",
            Command::FlipHorizontal => "Flip Horizontal",
            Command::FlipVertical => "Flip Vertical",
            Command::Rotate => "Rotate",
        }
    }

    /// The key used with Command (Ctrl on Linux and Windows), and whether
    /// Shift goes with it.
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
            _ => return None,
        })
    }

    pub fn is_enabled(self, state: MenuState) -> bool {
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
        assert_eq!(commands.len(), 19);
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
        };
        assert!(Command::all().all(|command| command.is_enabled(all)));
    }
}
