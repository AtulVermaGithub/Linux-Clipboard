//! User Settings and Configuration Module
//! Handles persistence of user preferences in ~/.config/linux-clipboard/settings.json

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const USER_SETTINGS_FILE: &str = "settings.json";
pub const DEFAULT_MAX_HISTORY_SIZE: usize = 50;

/// User-configurable settings for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    /// Theme mode: "system", "dark", or "light"
    pub theme_mode: String,
    /// Background opacity for dark mode (0.0 to 1.0)
    pub dark_background_opacity: f32,
    /// Background opacity for light mode (0.0 to 1.0)
    pub light_background_opacity: f32,

    // --- Feature Flags ---
    /// Enable Dynamic Tray Icon (changes color based on system theme)
    pub enable_dynamic_tray_icon: bool,
    /// Enable Smart Actions (URL, Color, Email detection)
    pub enable_smart_actions: bool,
    /// Enable UI Polish
    pub enable_ui_polish: bool,

    // --- History Settings ---
    /// Maximum number of clipboard history items to keep (1 to 100000)
    pub max_history_size: usize,
    /// Auto-delete interval value (0 means disabled)
    pub auto_delete_interval: u64,
    /// Auto-delete interval unit ("minutes", "hours", "days", "weeks")
    pub auto_delete_unit: String,

    // --- UI Scale ---
    /// UI scale factor for the clipboard window (0.5 to 2.0, default 1.0)
    pub ui_scale: f32,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme_mode: "system".to_string(),
            dark_background_opacity: 0.70,
            light_background_opacity: 0.70,
            enable_dynamic_tray_icon: true,
            enable_smart_actions: true,
            enable_ui_polish: true,
            max_history_size: DEFAULT_MAX_HISTORY_SIZE,
            auto_delete_interval: 0,
            auto_delete_unit: "hours".to_string(),
            ui_scale: 1.0,
        }
    }
}

impl UserSettings {
    pub fn auto_delete_interval_in_minutes(&self) -> u64 {
        if self.auto_delete_interval == 0 {
            return 0;
        }

        let base = self.auto_delete_interval;

        match self.auto_delete_unit.as_str() {
            "minutes" => base,
            "hours" => base.saturating_mul(60),
            "days" => base.saturating_mul(60).saturating_mul(24),
            "weeks" => base.saturating_mul(60).saturating_mul(24).saturating_mul(7),
            _ => 0,
        }
    }

    /// Validates and clamps setting fields
    pub fn validate(&mut self) {
        self.dark_background_opacity = self.dark_background_opacity.clamp(0.0, 1.0);
        self.light_background_opacity = self.light_background_opacity.clamp(0.0, 1.0);

        if !["system", "dark", "light"].contains(&self.theme_mode.as_str()) {
            self.theme_mode = "system".to_string();
        }

        self.max_history_size = self.max_history_size.clamp(1, 100_000);
        self.ui_scale = self.ui_scale.clamp(0.5, 2.0);

        if !["minutes", "hours", "days", "weeks"].contains(&self.auto_delete_unit.as_str()) {
            self.auto_delete_unit = "hours".to_string();
        }
    }
}

/// Manages loading and saving of user settings
pub struct UserSettingsManager {
    config_dir: PathBuf,
}

impl UserSettingsManager {
    /// Creates a new UserSettingsManager using ~/.config/linux-clipboard
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lincb.ople.in");

        Self { config_dir }
    }

    /// Gets the path to the settings file
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join(USER_SETTINGS_FILE)
    }

    /// Loads user settings from the config file
    pub fn load(&self) -> UserSettings {
        let path = self.settings_path();

        if !path.exists() {
            return UserSettings::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<UserSettings>(&content) {
                Ok(mut settings) => {
                    settings.validate();
                    settings
                }
                Err(e) => {
                    eprintln!("[Config] Failed to parse settings file: {}. Using defaults.", e);
                    UserSettings::default()
                }
            },
            Err(e) => {
                eprintln!("[Config] Failed to read settings file: {}. Using defaults.", e);
                UserSettings::default()
            }
        }
    }

    /// Saves user settings to the config file
    #[allow(dead_code)]
    pub fn save(&self, settings: &UserSettings) -> Result<(), String> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let mut validated_settings = settings.clone();
        validated_settings.validate();

        let content = serde_json::to_string_pretty(&validated_settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        fs::write(self.settings_path(), content)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }
}

impl Default for UserSettingsManager {
    fn default() -> Self {
        Self::new()
    }
}
