//! Icon in the notification area (system tray), next to the clock.
//!
//! Left click opens the main window; right click shows a menu to open it, start a detox right
//! away, start or stop the Pomodoro mode, turn syncing with paired devices and "Start with
//! Windows" on or off, or quit. While a
//! detox (or a Pomodoro break) is running the menu is disabled, so it can't be used to leave it.

use std::cell::{Cell, RefCell};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::Action;

/// Durations offered in the "Start detox" submenu, in minutes
const PRESETS: [u64; 5] = [5, 15, 25, 45, 60];

pub struct Tray {
    // The icon disappears from the notification area when this is dropped
    icon: TrayIcon,
    open: MenuItem,
    start: Submenu,
    pomodoro: MenuItem,
    sync: CheckMenuItem,
    autostart: CheckMenuItem,
    quit: MenuItem,
    detox_running: Cell<bool>,
    pomodoro_running: Cell<bool>,
    // Whether this device has paired devices to sync with
    paired: Cell<bool>,
    tooltip: RefCell<String>,
}

impl Tray {
    /// `autostart`: whether "Start with Windows" is currently on, for its check mark
    pub fn new(autostart: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let open = MenuItem::with_id("open", "Open Digital Detox", true, None);
        let start = Submenu::new("Start detox", true);
        for minutes in PRESETS {
            start.append(&MenuItem::with_id(format!("start:{minutes}"), format!("{minutes} minutes"), true, None))?;
        }
        let pomodoro = MenuItem::with_id("pomodoro", "Start Pomodoro", true, None);
        let sync = CheckMenuItem::with_id("sync", "Sync with paired devices", false, false, None);
        let autostart = CheckMenuItem::with_id("autostart", "Start with Windows", true, autostart, None);
        let quit = MenuItem::with_id("quit", "Quit", true, None);
        let menu =
            Menu::with_items(&[&open, &start, &pomodoro, &PredefinedMenuItem::separator(), &sync, &autostart, &quit])?;

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
                "pomodoro" => Action::TogglePomodoro,
                "sync" => Action::ToggleSync,
                "autostart" => Action::ToggleAutostart,
                "quit" => Action::Quit,
                id => match id.strip_prefix("start:").and_then(|m| m.parse().ok()) {
                    Some(minutes) => Action::Start(minutes),
                    None => return,
                },
            };
            crate::dispatch(action);
        }));

        Ok(Self {
            icon,
            open,
            start,
            pomodoro,
            sync,
            autostart,
            quit,
            detox_running: Cell::new(false),
            pomodoro_running: Cell::new(false),
            paired: Cell::new(false),
            tooltip: RefCell::new("Digital Detox".into()),
        })
    }

    /// Disables the menu while a detox is running, enables it again afterwards
    pub fn set_detox_running(&self, running: bool) {
        self.detox_running.set(running);
        self.update_items();
    }

    /// While the Pomodoro mode is on, "Start Pomodoro" becomes "Stop Pomodoro" and no detox can
    /// be started
    pub fn set_pomodoro_running(&self, running: bool) {
        if self.pomodoro_running.replace(running) != running {
            self.pomodoro.set_text(if running { "Stop Pomodoro" } else { "Start Pomodoro" });
            self.update_items();
        }
    }

    fn update_items(&self) {
        let detox = self.detox_running.get();
        self.open.set_enabled(!detox);
        self.start.set_enabled(!detox && !self.pomodoro_running.get());
        self.pomodoro.set_enabled(!detox);
        self.sync.set_enabled(!detox && self.paired.get());
        self.autostart.set_enabled(!detox);
        self.quit.set_enabled(!detox);
    }

    /// The "Sync with paired devices" check mark; the item is greyed out until a device is paired
    pub fn set_sync(&self, paired: bool, enabled: bool) {
        self.sync.set_checked(paired && enabled);
        if self.paired.replace(paired) != paired {
            self.update_items();
        }
    }

    /// Text shown when hovering the icon. Only passed on to Windows when it changes.
    pub fn set_tooltip(&self, text: &str) {
        if *self.tooltip.borrow() != text {
            let _ = self.icon.set_tooltip(Some(text));
            *self.tooltip.borrow_mut() = text.to_owned();
        }
    }

    /// The menu flips the check mark by itself on click: this puts it back in line with the
    /// actual setting, also when changing it failed
    pub fn set_autostart(&self, enabled: bool) {
        self.autostart.set_checked(enabled);
    }
}
