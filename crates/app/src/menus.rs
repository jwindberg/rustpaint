//! The native menu bar, on macOS. (Linux and Windows use the keyboard
//! shortcuts for now; their menus are settled with milestone 5.)

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use crate::commands::{Command, EDIT_MENU};

pub struct MenuBar {
    // Kept alive for as long as the menus are shown.
    _menu: Menu,
    items: Vec<(Command, MenuItem)>,
}

impl MenuBar {
    /// Builds the menus and installs them in the app. Call once the app is
    /// running.
    pub fn install() -> Self {
        let menu = Menu::new();

        let app_menu = Submenu::new("RustPaint", true);
        let about = AboutMetadata {
            name: Some("RustPaint".into()),
            ..Default::default()
        };
        let _ = app_menu.append_items(&[
            &PredefinedMenuItem::about(None, Some(about)),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ]);

        let edit_menu = Submenu::new("Edit", true);
        let mut items = Vec::new();
        for (index, group) in EDIT_MENU.iter().enumerate() {
            if index > 0 {
                let _ = edit_menu.append(&PredefinedMenuItem::separator());
            }
            for &command in *group {
                let accelerator = command
                    .shortcut()
                    .and_then(key_code)
                    .map(|code| Accelerator::new(Modifiers::META, code));
                let item = MenuItem::with_id(menu_id(command), command.label(), true, accelerator);
                let _ = edit_menu.append(&item);
                items.push((command, item));
            }
        }

        let _ = menu.append_items(&[&app_menu, &edit_menu]);
        menu.init_for_nsapp();
        Self { _menu: menu, items }
    }

    /// The command a menu event is for.
    pub fn command(&self, id: &MenuId) -> Option<Command> {
        self.items
            .iter()
            .find(|(_, item)| item.id() == id)
            .map(|(command, _)| *command)
    }

    /// Enables what makes sense right now, as HaikuPaint's MenusBeginning
    /// does.
    pub fn update(&self, can_undo: bool, has_selection: bool) {
        for (command, item) in &self.items {
            let enabled = match command {
                Command::Undo => can_undo,
                command => !command.needs_selection() || has_selection,
            };
            item.set_enabled(enabled);
        }
    }
}

fn menu_id(command: Command) -> MenuId {
    MenuId::new(format!("{command:?}"))
}

fn key_code(key: char) -> Option<Code> {
    Some(match key {
        'a' => Code::KeyA,
        'c' => Code::KeyC,
        'e' => Code::KeyE,
        'v' => Code::KeyV,
        'x' => Code::KeyX,
        'z' => Code::KeyZ,
        _ => return None,
    })
}
