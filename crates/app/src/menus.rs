//! The native menu bar, on macOS. Linux and Windows draw theirs in the
//! window (see crate::menu_bar).

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use crate::commands::{Command, MENUS, MenuState};

pub struct MenuBar {
    menu: Menu,
    items: Vec<(Command, MenuItem)>,
}

impl MenuBar {
    pub fn new() -> Self {
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
            let quit = new_item(Command::Quit);
            let _ = app_menu.append_items(&[
                &PredefinedMenuItem::about(None, Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &quit,
            ]);
            let _ = menu.append(&app_menu);
            items.push((Command::Quit, quit));
        }

        for spec in MENUS {
            let submenu = Submenu::new(spec.title, true);
            let mut first = true;
            for group in spec.groups {
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
                    let item = new_item(command);
                    let _ = submenu.append(&item);
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
            item.set_enabled(command.is_enabled(state));
        }
    }
}

fn new_item(command: Command) -> MenuItem {
    let modifier = Modifiers::META;
    let accelerator = command.shortcut().and_then(|(key, shift)| {
        let modifiers = if shift {
            modifier | Modifiers::SHIFT
        } else {
            modifier
        };
        key_code(key).map(|code| Accelerator::new(modifiers, code))
    });
    MenuItem::with_id(
        MenuId::new(format!("{command:?}")),
        command.label(),
        true,
        accelerator,
    )
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
