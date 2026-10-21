//! Menu commands, shared by the native menus and the keyboard shortcuts.

/// MacPaint's Edit menu, plus Select All, as in HaikuPaint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
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

/// The Edit menu, in groups between separators.
pub const EDIT_MENU: &[&[Command]] = &[
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
];

impl Command {
    pub fn label(self) -> &'static str {
        match self {
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

    /// The key used with Command (Alt on Linux and Windows).
    pub fn shortcut(self) -> Option<char> {
        match self {
            Command::Undo => Some('z'),
            Command::Cut => Some('x'),
            Command::Copy => Some('c'),
            Command::Paste => Some('v'),
            Command::SelectAll => Some('a'),
            Command::TraceEdges => Some('e'),
            _ => None,
        }
    }

    /// Whether it only makes sense with a selection.
    pub fn needs_selection(self) -> bool {
        matches!(
            self,
            Command::Cut
                | Command::Copy
                | Command::Invert
                | Command::Fill
                | Command::TraceEdges
                | Command::FlipHorizontal
                | Command::FlipVertical
                | Command::Rotate
        )
    }

    /// The command a Command-key shortcut stands for.
    pub fn from_shortcut(key: char) -> Option<Command> {
        let key = key.to_ascii_lowercase();
        EDIT_MENU
            .iter()
            .flat_map(|group| group.iter())
            .copied()
            .find(|c| c.shortcut() == Some(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_unique_and_found() {
        let commands: Vec<Command> = EDIT_MENU.iter().flat_map(|g| g.iter()).copied().collect();
        assert_eq!(commands.len(), 12);
        for command in &commands {
            if let Some(key) = command.shortcut() {
                assert_eq!(
                    Command::from_shortcut(key.to_ascii_uppercase()),
                    Some(*command)
                );
            }
        }
        assert_eq!(Command::from_shortcut('q'), None);
    }
}
