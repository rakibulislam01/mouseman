// SECURITY:
//   - Config is read once at startup, never written back to disk
//   - No sensitive data stored — only button→action mappings
//   - File path is user-supplied; we do no directory traversal
//   - All input is validated before use

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A single button mapping
#[derive(Debug, Deserialize, Clone)]
pub struct ButtonAction {
    pub action: ActionKind,
    /// Only used for ActionKind::Shortcut
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    MissionControl,
    AppSwitch,
    WindowSwitch,
    ExposeApp,
    Shortcut,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub buttons: HashMap<String, ButtonAction>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Cannot read config file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("Invalid YAML in config: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl Config {
    /// Load and validate config from a YAML file.
    /// Security: reads file contents into memory only, discarded after parsing.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;

        let cfg: Config = serde_yaml::from_str(&contents)?;
        cfg.validate()?;

        // Security: `contents` (raw YAML bytes) is dropped here — not held in memory
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        for (btn, action) in &self.buttons {
            // Validate button name format: must be "buttonN" where N is a number
            if !btn.starts_with("button") {
                return Err(ConfigError::Validation(format!(
                    "Invalid button name '{btn}' — must be 'button4', 'button5', etc."
                )));
            }
            let num_part = &btn["button".len()..];
            if num_part.parse::<u32>().is_err() {
                return Err(ConfigError::Validation(format!(
                    "Invalid button name '{btn}' — number part '{num_part}' is not a valid integer"
                )));
            }

            // Shortcut action requires keys
            if action.action == ActionKind::Shortcut && action.keys.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "Button '{btn}' uses action 'shortcut' but has no 'keys' defined"
                )));
            }

            // Validate key names for shortcuts (prevents injection of arbitrary values)
            if action.action == ActionKind::Shortcut {
                for key in &action.keys {
                    if !is_valid_key(key) {
                        return Err(ConfigError::Validation(format!(
                            "Button '{btn}': unknown key '{key}' in shortcut"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Allowlist of valid key names — prevents arbitrary string injection
fn is_valid_key(key: &str) -> bool {
    const VALID_KEYS: &[&str] = &[
        // Modifiers
        "cmd", "command", "shift", "alt", "option", "ctrl", "control",
        // Letters
        "a","b","c","d","e","f","g","h","i","j","k","l","m",
        "n","o","p","q","r","s","t","u","v","w","x","y","z",
        // Numbers
        "0","1","2","3","4","5","6","7","8","9",
        // Function keys
        "f1","f2","f3","f4","f5","f6","f7","f8","f9","f10","f11","f12",
        // Special
        "space","return","enter","tab","delete","escape","esc",
        "left","right","up","down","home","end","pageup","pagedown",
        // Symbols
        "-","=","[","]","\\",";","'",",",".","/"," `,`","grave",
    ];
    VALID_KEYS.contains(&key.to_lowercase().as_str())
}
