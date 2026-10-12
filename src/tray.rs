//! Icon in the notification area (system tray), next to the clock.
//!
//! Left click opens the main window; right click shows a menu to open it, start a detox right
//! away, or quit. While a detox is running the menu is disabled, so it can't be used to leave it.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::Action;

/// Durations offered in the "Start detox" submenu, in minutes
const PRESETS: [u64; 5] = [5, 15, 25, 45, 60];

pub struct Tray {
    // The icon disappears from the notification area when this is dropped
    _icon: TrayIcon,
    open: MenuItem,
    start: Submenu,
    quit: MenuItem,
}

impl Tray {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let open = MenuItem::with_id("open", "Open Digital Detox", true, None);
        let start = Submenu::new("Start detox", true);
        for minutes in PRESETS {
            start.append(&MenuItem::with_id(format!("start:{minutes}"), format!("{minutes} minutes"), true, None))?;
        }
        let quit = MenuItem::with_id("quit", "Quit", true, None);
        let menu = Menu::with_items(&[&open, &start, &PredefinedMenuItem::separator(), &quit])?;

        let icon = TrayIconBuilder::new()
            // The app icon embedded in the .exe by build.rs (resource ID 1)
            .with_icon(Icon::from_resource(1, None)?)
            .with_tooltip("Digital Detox")
            .with_menu(Box::new(menu))
            // The menu is for the right button; the left one opens the window
            .with_menu_on_left_click(false)
            .build()?;

        // Both handlers run inside Windows' message handling: they only pass the action on
        TrayIconEvent::set_event_handler(Some(|event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                crate::dispatch(Action::Open);
            }
        }));
        MenuEvent::set_event_handler(Some(|event: MenuEvent| {
            let action = match event.id.as_ref() {
                "open" => Action::Open,
                "quit" => Action::Quit,
                id => match id.strip_prefix("start:").and_then(|m| m.parse().ok()) {
                    Some(minutes) => Action::Start(minutes),
                    None => return,
                },
            };
            crate::dispatch(action);
        }));

        Ok(Self { _icon: icon, open, start, quit })
    }

    /// Disables the menu while a detox is running, enables it again afterwards
    pub fn set_detox_running(&self, running: bool) {
        self.open.set_enabled(!running);
        self.start.set_enabled(!running);
        self.quit.set_enabled(!running);
    }
}
