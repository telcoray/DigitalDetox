//! Icon in the notification area (system tray), next to the clock.
//!
//! Left click opens the main window; right click shows a menu to open it, start a detox right
//! away, turn "Start with Windows" on or off, or quit. While a detox is running the menu is
//! disabled, so it can't be used to leave it.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::Action;

/// Durations offered in the "Start detox" submenu, in minutes
const PRESETS: [u64; 5] = [5, 15, 25, 45, 60];

pub struct Tray {
    // The icon disappears from the notification area when this is dropped
    _icon: TrayIcon,
    open: MenuItem,
    start: Submenu,
    autostart: CheckMenuItem,
    quit: MenuItem,
}

impl Tray {
    /// `autostart`: whether "Start with Windows" is currently on, for its check mark
    pub fn new(autostart: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let open = MenuItem::with_id("open", "Open Digital Detox", true, None);
        let start = Submenu::new("Start detox", true);
        for minutes in PRESETS {
            start.append(&MenuItem::with_id(format!("start:{minutes}"), format!("{minutes} minutes"), true, None))?;
        }
        let autostart = CheckMenuItem::with_id("autostart", "Start with Windows", true, autostart, None);
        let quit = MenuItem::with_id("quit", "Quit", true, None);
        let menu = Menu::with_items(&[&open, &start, &PredefinedMenuItem::separator(), &autostart, &quit])?;

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
                "autostart" => Action::ToggleAutostart,
                "quit" => Action::Quit,
                id => match id.strip_prefix("start:").and_then(|m| m.parse().ok()) {
                    Some(minutes) => Action::Start(minutes),
                    None => return,
                },
            };
            crate::dispatch(action);
        }));

        Ok(Self { _icon: icon, open, start, autostart, quit })
    }

    /// Disables the menu while a detox is running, enables it again afterwards
    pub fn set_detox_running(&self, running: bool) {
        self.open.set_enabled(!running);
        self.start.set_enabled(!running);
        self.autostart.set_enabled(!running);
        self.quit.set_enabled(!running);
    }

    /// The menu flips the check mark by itself on click: this puts it back in line with the
    /// actual setting, also when changing it failed
    pub fn set_autostart(&self, enabled: bool) {
        self.autostart.set_checked(enabled);
    }
}
