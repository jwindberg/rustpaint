//! The native menu bar, on macOS. Linux and Windows draw theirs in the
//! window (see crate::menu_bar).

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{CheckMenuItem, IsMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use crate::commands::{Command, MenuState, menus};

/// An item, plain or with a check mark.
enum Item {
    Plain(MenuItem),
    Check(CheckMenuItem),
}

impl Item {
    fn as_menu_item(&self) -> &dyn IsMenuItem {
        match self {
            Item::Plain(item) => item,
            Item::Check(item) => item,
        }
    }

    fn id(&self) -> &MenuId {
        match self {
            Item::Plain(item) => item.id(),
            Item::Check(item) => item.id(),
        }
    }
}

pub struct MenuBar {
    menu: Menu,
    items: Vec<(Command, Item)>,
}

impl MenuBar {
    /// The menus, with the installed font families in the Font menu.
    pub fn new(families: &[String]) -> Self {
        let menu = Menu::new();
        let mut items = Vec::new();

        {
            // The application menu, with Quit, which RustPaint handles so
            // it can ask about unsaved changes first.
            let app_menu = Submenu::new("RustPaint", true);
            let about = muda::AboutMetadata {
                name: Some("RustPaint".into()),
                ..Default::default()
            };
            let quit = new_item(Command::Quit, families);
            let _ = app_menu.append_items(&[
                &PredefinedMenuItem::about(None, Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                quit.as_menu_item(),
            ]);
            let _ = menu.append(&app_menu);
            items.push((Command::Quit, quit));
        }

        for spec in menus(families.len()) {
            let submenu = Submenu::new(spec.title, true);
            let mut first = true;
            for group in &spec.groups {
                // Quit lives in the application menu on macOS.
                let commands: Vec<Command> = group
                    .iter()
                    .copied()
                    .filter(|&c| c != Command::Quit)
                    .collect();
                if commands.is_empty() {
                    continue;
                }
                if !first {
                    let _ = submenu.append(&PredefinedMenuItem::separator());
                }
                first = false;
                for command in commands {
                    let item = new_item(command, families);
                    let _ = submenu.append(item.as_menu_item());
                    items.push((command, item));
                }
            }
            let _ = menu.append(&submenu);
        }

        Self { menu, items }
    }

    /// Shows the menus in the menu bar. Call once the app is running.
    pub fn install(&self) {
        self.menu.init_for_nsapp();
    }

    /// The command a menu event is for.
    pub fn command(&self, id: &MenuId) -> Option<Command> {
        self.items
            .iter()
            .find(|(_, item)| item.id() == id)
            .map(|(command, _)| *command)
    }

    pub fn update(&self, state: MenuState) {
        for (command, item) in &self.items {
            let enabled = command.is_enabled(state);
            match item {
                Item::Plain(item) => item.set_enabled(enabled),
                Item::Check(item) => {
                    item.set_enabled(enabled);
                    item.set_checked(command.is_checked(state) == Some(true));
                }
            }
        }
    }
}

fn new_item(command: Command, families: &[String]) -> Item {
    let modifier = Modifiers::META;
    let accelerator = command.shortcut().and_then(|(key, shift)| {
        let modifiers = if shift {
            modifier | Modifiers::SHIFT
        } else {
            modifier
        };
        key_code(key).map(|code| Accelerator::new(modifiers, code))
    });
    let id = MenuId::new(format!("{command:?}"));
    if command.is_checked(MenuState::default()).is_some() {
        Item::Check(CheckMenuItem::with_id(
            id,
            command.label(families),
            true,
            false,
            accelerator,
        ))
    } else {
        Item::Plain(MenuItem::with_id(
            id,
            command.label(families),
            true,
            accelerator,
        ))
    }
}

fn key_code(key: char) -> Option<Code> {
    Some(match key {
        'a' => Code::KeyA,
        'c' => Code::KeyC,
        'e' => Code::KeyE,
        'n' => Code::KeyN,
        'o' => Code::KeyO,
        'q' => Code::KeyQ,
        's' => Code::KeyS,
        'v' => Code::KeyV,
        'x' => Code::KeyX,
        'z' => Code::KeyZ,
        _ => return None,
    })
}
