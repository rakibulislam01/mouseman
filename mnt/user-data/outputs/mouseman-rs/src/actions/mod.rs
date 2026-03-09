// Dispatches button events to configured actions.
//
// SECURITY:
//   - Event data (button number + pressed state) is used immediately, never stored
//   - No logging of which buttons are pressed (only debug-level action names)
//   - All actions go through macos:: which uses only allowlisted key codes

use crate::config::{ActionKind, Config};

#[cfg(target_os = "macos")]
use crate::macos;

pub struct Executor {
    config: Config,
}

impl Executor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Handle a button event.
    /// Security: only called on press (not release), event data not stored.
    pub fn handle(&self, button: u32, pressed: bool) {
        // Only act on press, ignore release
        if !pressed {
            return;
        }

        let key = format!("button{button}");
        let Some(action) = self.config.buttons.get(&key) else {
            return; // not configured — silently ignore
        };

        #[cfg(target_os = "macos")]
        match &action.action {
            ActionKind::MissionControl => macos::mission_control(),
            ActionKind::AppSwitch      => macos::app_switch(),
            ActionKind::WindowSwitch   => macos::window_switch(),
            ActionKind::ExposeApp      => macos::expose_app(),
            ActionKind::Shortcut       => {
                if let Err(e) = macos::shortcut(&action.keys) {
                    // Log error but never panic — keep daemon running
                    log::error!("Shortcut error for {key}: {e}");
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        log::warn!("Action triggered on non-macOS platform (no-op): {:?}", action.action);
    }
}
