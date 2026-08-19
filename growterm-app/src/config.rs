use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::platform::key_convert::char_to_keycode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyModeAction {
    Down,
    Up,
    Visual,
    HalfPageDown,
    HalfPageUp,
    Yank,
    OpenUrl,
    Exit,
}

fn deserialize_keys<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(s) => Ok(vec![s]),
        OneOrMany::Many(v) => Ok(v),
    }
}

fn default_down() -> Vec<String> { vec!["j".into()] }
fn default_up() -> Vec<String> { vec!["k".into()] }
fn default_visual() -> Vec<String> { vec!["v".into()] }
fn default_half_page_down() -> Vec<String> { vec!["h".into(), "d".into()] }
fn default_half_page_up() -> Vec<String> { vec!["l".into(), "u".into()] }
fn default_yank() -> Vec<String> { vec!["y".into()] }
fn default_open_url() -> Vec<String> { vec!["o".into()] }
fn default_exit() -> Vec<String> { vec!["q".into(), "Escape".into(), "`".into()] }

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CopyModeKeys {
    #[serde(default = "default_down", deserialize_with = "deserialize_keys")]
    pub down: Vec<String>,
    #[serde(default = "default_up", deserialize_with = "deserialize_keys")]
    pub up: Vec<String>,
    #[serde(default = "default_visual", deserialize_with = "deserialize_keys")]
    pub visual: Vec<String>,
    #[serde(default = "default_half_page_down", deserialize_with = "deserialize_keys")]
    pub half_page_down: Vec<String>,
    #[serde(default = "default_half_page_up", deserialize_with = "deserialize_keys")]
    pub half_page_up: Vec<String>,
    #[serde(default = "default_yank", deserialize_with = "deserialize_keys")]
    pub yank: Vec<String>,
    #[serde(default = "default_open_url", deserialize_with = "deserialize_keys")]
    pub open_url: Vec<String>,
    #[serde(default = "default_exit", deserialize_with = "deserialize_keys")]
    pub exit: Vec<String>,
}

impl Default for CopyModeKeys {
    fn default() -> Self {
        Self {
            down: default_down(),
            up: default_up(),
            visual: default_visual(),
            half_page_down: default_half_page_down(),
            half_page_up: default_half_page_up(),
            yank: default_yank(),
            open_url: default_open_url(),
            exit: default_exit(),
        }
    }
}

impl CopyModeKeys {
    pub fn build_action_map(&self) -> HashMap<u16, CopyModeAction> {
        let mut map = HashMap::new();
        let bindings: &[(&[String], CopyModeAction)] = &[
            (&self.down, CopyModeAction::Down),
            (&self.up, CopyModeAction::Up),
            (&self.visual, CopyModeAction::Visual),
            (&self.half_page_down, CopyModeAction::HalfPageDown),
            (&self.half_page_up, CopyModeAction::HalfPageUp),
            (&self.yank, CopyModeAction::Yank),
            (&self.open_url, CopyModeAction::OpenUrl),
            (&self.exit, CopyModeAction::Exit),
        ];
        for (keys, action) in bindings {
            for key_str in *keys {
                if let Some(kc) = char_to_keycode(key_str) {
                    map.insert(kc, *action);
                }
            }
        }
        map
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default)]
    pub pomodoro: bool,
    #[serde(default = "default_pomodoro_work_seconds")]
    pub pomodoro_work_seconds: u64,
    #[serde(default = "default_pomodoro_break_seconds")]
    pub pomodoro_break_seconds: u64,
    #[serde(default)]
    pub response_timer: bool,
    #[serde(default = "default_true")]
    pub coaching: bool,
    #[serde(default)]
    pub transparent_tab_bar: bool,
    #[serde(default = "default_header_opacity")]
    pub header_opacity: f32,
    #[serde(default = "default_unfocused_tint")]
    pub unfocused_tint: f32,
    #[serde(default = "default_unfocused_tint_color")]
    pub unfocused_tint_color: String,
    #[serde(default)]
    pub coaching_command: Option<String>,
    #[serde(default)]
    pub copy_mode_keys: CopyModeKeys,
    #[serde(default)]
    pub window_width: Option<f64>,
    #[serde(default)]
    pub window_height: Option<f64>,
    #[serde(default)]
    pub window_x: Option<f64>,
    #[serde(default)]
    pub window_y: Option<f64>,
}

fn default_font_family() -> String {
    "FiraCodeNerdFontMono-Retina".to_string()
}

fn default_font_size() -> f32 {
    32.0
}

fn default_pomodoro_work_seconds() -> u64 {
    25 * 60
}

fn default_pomodoro_break_seconds() -> u64 {
    3 * 60
}

fn default_header_opacity() -> f32 {
    0.8
}

/// How far an unfocused window is dimmed, 0.0 (off) to 1.0 (the tint colour).
fn default_unfocused_tint() -> f32 {
    0.1
}

/// What an unfocused window is washed with. A colour reads as "not this one"
/// at a glance, where a plain dim only reads as "harder to see".
fn default_unfocused_tint_color() -> String {
    "#cc0000".to_string()
}

fn default_true() -> bool {
    true
}

fn parse_hex_rgb(text: &str) -> Option<[f32; 3]> {
    let hex = text.trim().strip_prefix('#').unwrap_or(text.trim());
    let width = match hex.len() {
        3 => 1,
        6 => 2,
        _ => return None,
    };
    let mut rgb = [0.0f32; 3];
    for (i, slot) in rgb.iter_mut().enumerate() {
        let part = &hex[i * width..(i + 1) * width];
        let value = u8::from_str_radix(part, 16).ok()?;
        let max = if width == 1 { 15.0 } else { 255.0 };
        *slot = value as f32 / max;
    }
    Some(rgb)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            font_size: default_font_size(),
            pomodoro: false,
            pomodoro_work_seconds: default_pomodoro_work_seconds(),
            pomodoro_break_seconds: default_pomodoro_break_seconds(),
            response_timer: false,
            coaching: true,
            transparent_tab_bar: false,
            header_opacity: default_header_opacity(),
            unfocused_tint: default_unfocused_tint(),
            unfocused_tint_color: default_unfocused_tint_color(),
            coaching_command: None,
            copy_mode_keys: CopyModeKeys::default(),
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
        }
    }
}

impl Config {
    pub fn window_size(&self) -> (f64, f64) {
        (self.window_width.unwrap_or(800.0), self.window_height.unwrap_or(600.0))
    }

    pub fn window_position(&self) -> Option<(f64, f64)> {
        match (self.window_x, self.window_y) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("growterm")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            return Self::load_from_file(&path);
        }

        // Migration: read legacy individual files
        let dir = config_dir();
        let has_legacy = dir.join("pomodoro_enabled").exists()
            || dir.join("response_timer_enabled").exists()
            || dir.join("coaching_enabled").exists()
            || dir.join("transparent_tab_bar").exists();

        if has_legacy {
            let config = Self::migrate_from_legacy(&dir);
            config.save();
            return config;
        }

        Self::default()
    }

    fn load_from_file(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::parse_str(&contents),
            Err(_) => Self::default(),
        }
    }

    fn parse_str(contents: &str) -> Self {
        let mut config: Config = toml::from_str(contents).unwrap_or_default();
        config.adopt_legacy_minutes(contents);
        config
    }

    /// The pomodoro durations were once written in minutes. Serde skips keys it
    /// no longer knows, so a config still in that form would quietly fall back
    /// to the defaults — read them here and carry them across. Saving writes
    /// only the seconds form, so the old keys drop out on their own.
    fn adopt_legacy_minutes(&mut self, contents: &str) {
        let Ok(value) = contents.parse::<toml::Value>() else {
            return;
        };
        let minutes = |key: &str| {
            value
                .get(key)
                .and_then(toml::Value::as_integer)
                .filter(|m| *m >= 0)
                .map(|m| m as u64 * 60)
        };
        if value.get("pomodoro_work_seconds").is_none() {
            if let Some(secs) = minutes("pomodoro_work_minutes") {
                self.pomodoro_work_seconds = secs;
            }
        }
        if value.get("pomodoro_break_seconds").is_none() {
            if let Some(secs) = minutes("pomodoro_break_minutes") {
                self.pomodoro_break_seconds = secs;
            }
        }
    }

    fn migrate_from_legacy(dir: &std::path::Path) -> Self {
        let read_bool = |name: &str, default: bool| -> bool {
            match std::fs::read_to_string(dir.join(name)) {
                Ok(s) => {
                    if default {
                        s.trim() != "0"
                    } else {
                        s.trim() == "1"
                    }
                }
                Err(_) => default,
            }
        };

        Self {
            font_family: default_font_family(),
            font_size: default_font_size(),
            pomodoro: read_bool("pomodoro_enabled", false),
            pomodoro_work_seconds: default_pomodoro_work_seconds(),
            pomodoro_break_seconds: default_pomodoro_break_seconds(),
            response_timer: read_bool("response_timer_enabled", false),
            coaching: read_bool("coaching_enabled", true),
            transparent_tab_bar: read_bool("transparent_tab_bar", false),
            header_opacity: default_header_opacity(),
            unfocused_tint: default_unfocused_tint(),
            unfocused_tint_color: default_unfocused_tint_color(),
            coaching_command: None,
            copy_mode_keys: CopyModeKeys::default(),
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
        }
    }

    /// The tint colour as linear-ish 0..1 components, falling back to the
    /// default when the config carries something unreadable.
    pub fn unfocused_tint_rgb(&self) -> [f32; 3] {
        parse_hex_rgb(&self.unfocused_tint_color)
            .or_else(|| parse_hex_rgb(&default_unfocused_tint_color()))
            .unwrap_or([0.0, 0.0, 0.0])
    }

    /// Flip one of the settings the desktop's menu offers. Unknown names are
    /// refused rather than guessed at, so a typo in the .desktop file shows up.
    pub fn toggle(&mut self, name: &str) -> bool {
        let field = match name {
            "pomodoro" => &mut self.pomodoro,
            "coaching" => &mut self.coaching,
            "response-timer" => &mut self.response_timer,
            "transparent-tab-bar" => &mut self.transparent_tab_bar,
            _ => return false,
        };
        *field = !*field;
        true
    }

    pub fn save(&self) {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(content) = toml::to_string(self) {
            let _ = std::fs::write(config_path(), content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml = r#"
font_family = "Menlo"
font_size = 24.0
pomodoro = true
response_timer = false
coaching = false
transparent_tab_bar = true
header_opacity = 0.5
coaching_command = "claude -p --system 'You are a coach' '{prompt}'"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.font_family, "Menlo");
        assert_eq!(config.font_size, 24.0);
        assert!(config.pomodoro);
        assert!(!config.response_timer);
        assert!(!config.coaching);
        assert!(config.transparent_tab_bar);
        assert_eq!(config.header_opacity, 0.5);
        assert_eq!(
            config.coaching_command,
            Some("claude -p --system 'You are a coach' '{prompt}'".to_string())
        );
    }

    #[test]
    fn coaching_command_default_is_none() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.coaching_command.is_none());
    }

    #[test]
    fn parse_empty_uses_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn parse_partial_config() {
        let toml = "font_size = 16.0\npomodoro = true\n";
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.font_size, 16.0);
        assert!(config.pomodoro);
        assert_eq!(config.font_family, "FiraCodeNerdFontMono-Retina");
        assert!(config.coaching); // default true
    }

    #[test]
    fn default_values() {
        let config = Config::default();
        assert_eq!(config.font_family, "FiraCodeNerdFontMono-Retina");
        assert_eq!(config.font_size, 32.0);
        assert!(!config.pomodoro);
        assert!(!config.response_timer);
        assert!(config.coaching);
        assert!(!config.transparent_tab_bar);
        assert_eq!(config.header_opacity, 0.8);
    }

    #[test]
    fn migrate_from_legacy_files() {
        let dir = std::env::temp_dir().join("growterm_test_migrate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("pomodoro_enabled"), "1").unwrap();
        std::fs::write(dir.join("response_timer_enabled"), "0").unwrap();
        std::fs::write(dir.join("coaching_enabled"), "0").unwrap();
        std::fs::write(dir.join("transparent_tab_bar"), "1").unwrap();

        let config = Config::migrate_from_legacy(&dir);
        assert!(config.pomodoro);
        assert!(!config.response_timer);
        assert!(!config.coaching);
        assert!(config.transparent_tab_bar);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_mode_keys_default() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.copy_mode_keys.down, vec!["j"]);
        assert_eq!(config.copy_mode_keys.exit, vec!["q", "Escape", "`"]);
    }

    #[test]
    fn copy_mode_keys_custom() {
        let toml = r#"
[copy_mode_keys]
down = "n"
half_page_down = ["d", "h"]
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.copy_mode_keys.down, vec!["n"]);
        assert_eq!(config.copy_mode_keys.half_page_down, vec!["d", "h"]);
        // defaults preserved for unspecified
        assert_eq!(config.copy_mode_keys.up, vec!["k"]);
    }

    #[test]
    fn copy_mode_keys_single_string_deserialized_as_vec() {
        let toml = r#"
[copy_mode_keys]
exit = "q"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.copy_mode_keys.exit, vec!["q"]);
    }

    #[test]
    fn build_action_map_default() {
        use crate::platform::key_convert::keycode as kc;
        let keys = CopyModeKeys::default();
        let map = keys.build_action_map();
        assert_eq!(map.get(&kc::ANSI_J), Some(&CopyModeAction::Down));
        assert_eq!(map.get(&kc::ANSI_K), Some(&CopyModeAction::Up));
        assert_eq!(map.get(&kc::ANSI_V), Some(&CopyModeAction::Visual));
        assert_eq!(map.get(&kc::ANSI_H), Some(&CopyModeAction::HalfPageDown));
        assert_eq!(map.get(&kc::ANSI_D), Some(&CopyModeAction::HalfPageDown));
        assert_eq!(map.get(&kc::ANSI_L), Some(&CopyModeAction::HalfPageUp));
        assert_eq!(map.get(&kc::ANSI_U), Some(&CopyModeAction::HalfPageUp));
        assert_eq!(map.get(&kc::ANSI_Y), Some(&CopyModeAction::Yank));
        assert_eq!(map.get(&kc::ANSI_O), Some(&CopyModeAction::OpenUrl));
        assert_eq!(map.get(&kc::ESCAPE), Some(&CopyModeAction::Exit));
        assert_eq!(map.get(&kc::ANSI_Q), Some(&CopyModeAction::Exit));
        assert_eq!(map.get(&kc::ANSI_GRAVE), Some(&CopyModeAction::Exit));
    }

    #[test]
    fn build_action_map_custom() {
        use crate::platform::key_convert::keycode as kc;
        let mut keys = CopyModeKeys::default();
        keys.down = vec!["n".into()];
        let map = keys.build_action_map();
        assert_eq!(map.get(&kc::ANSI_N), Some(&CopyModeAction::Down));
        assert_eq!(map.get(&kc::ANSI_J), None); // j no longer mapped
    }

    #[test]
    fn window_size_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.window_size(), (800.0, 600.0));
        assert_eq!(config.window_position(), None);
    }

    #[test]
    fn window_size_and_position_from_config() {
        let toml = r#"
window_width = 1200
window_height = 800
window_x = 100
window_y = 50
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.window_size(), (1200.0, 800.0));
        assert_eq!(config.window_position(), Some((100.0, 50.0)));
    }

    #[test]
    fn window_position_requires_both_x_and_y() {
        let toml = "window_x = 100\n";
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.window_position(), None);
    }

    #[test]
    fn pomodoro_time_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.pomodoro_work_seconds, 25 * 60);
        assert_eq!(config.pomodoro_break_seconds, 3 * 60);
    }

    #[test]
    fn pomodoro_time_custom() {
        let toml = "pomodoro_work_seconds = 3000\npomodoro_break_seconds = 600\n";
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.pomodoro_work_seconds, 3000);
        assert_eq!(config.pomodoro_break_seconds, 600);
    }

    #[test]
    fn pomodoro_break_accepts_under_a_minute() {
        let config: Config = toml::from_str("pomodoro_break_seconds = 10\n").unwrap();
        assert_eq!(config.pomodoro_break_seconds, 10);
    }

    #[test]
    fn unknown_fields_ignored() {
        let toml = "font_size = 20.0\nunknown_field = 42\n";
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.font_size, 20.0);
    }

    #[test]
    fn save_preserves_all_fields_roundtrip() {
        let dir = std::env::temp_dir().join("growterm_test_save_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let mut config = Config::default();
        config.pomodoro = true;
        config.pomodoro_work_seconds = 120;
        config.coaching_command = Some("claude -p".to_string());
        config.window_x = Some(100.0);
        config.window_y = Some(50.0);

        let serialized = toml::to_string(&config).unwrap();
        std::fs::write(&path, &serialized).unwrap();

        // Simulate UI toggle: load, change one field, save again
        let mut reloaded: Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        reloaded.transparent_tab_bar = true;
        let serialized2 = toml::to_string(&reloaded).unwrap();
        std::fs::write(&path, &serialized2).unwrap();

        // Verify all fields survived
        let final_config: Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(final_config.pomodoro_work_seconds, 120);
        assert_eq!(final_config.coaching_command, Some("claude -p".to_string()));
        assert!(final_config.transparent_tab_bar);
        assert_eq!(final_config.window_x, Some(100.0));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod unfocused_tint_tests {
    use super::*;

    #[test]
    fn unfocused_tint_defaults_to_a_visible_wash() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.unfocused_tint, 0.1);
    }

    #[test]
    fn unfocused_tint_is_configurable() {
        let config: Config = toml::from_str("unfocused_tint = 0.4\n").unwrap();
        assert_eq!(config.unfocused_tint, 0.4);
    }

    #[test]
    fn unfocused_tint_can_be_turned_off() {
        let config: Config = toml::from_str("unfocused_tint = 0.0\n").unwrap();
        assert_eq!(config.unfocused_tint, 0.0);
    }

    #[test]
    fn unfocused_tint_colour_defaults_to_red() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.unfocused_tint_color, "#cc0000");
    }

    #[test]
    fn unfocused_tint_colour_is_configurable() {
        let config: Config = toml::from_str("unfocused_tint_color = \"#ff8800\"\n").unwrap();
        assert_eq!(config.unfocused_tint_rgb(), [1.0, 0x88 as f32 / 255.0, 0.0]);
    }

    #[test]
    fn a_three_digit_colour_expands() {
        let config: Config = toml::from_str("unfocused_tint_color = \"#f00\"\n").unwrap();
        assert_eq!(config.unfocused_tint_rgb(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_colour_needs_no_hash() {
        let config: Config = toml::from_str("unfocused_tint_color = \"000000\"\n").unwrap();
        assert_eq!(config.unfocused_tint_rgb(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn an_unreadable_colour_falls_back_to_the_default() {
        let config: Config = toml::from_str("unfocused_tint_color = \"not a colour\"\n").unwrap();
        assert_eq!(config.unfocused_tint_rgb(), [0.8, 0.0, 0.0]);
    }
}

#[cfg(test)]
mod toggle_tests {
    use super::*;

    #[test]
    fn a_toggle_flips_its_setting() {
        let mut config = Config::default();
        assert!(!config.pomodoro);

        assert!(config.toggle("pomodoro"));

        assert!(config.pomodoro);
    }

    #[test]
    fn a_toggle_flips_back() {
        let mut config = Config::default();

        config.toggle("response-timer");
        config.toggle("response-timer");

        assert!(!config.response_timer);
    }

    #[test]
    fn every_name_the_desktop_menu_uses_is_known() {
        let mut config = Config::default();

        for name in ["pomodoro", "coaching", "response-timer", "transparent-tab-bar"] {
            assert!(config.toggle(name), "{name} should be a setting");
        }
    }

    #[test]
    fn an_unknown_name_changes_nothing() {
        let mut config = Config::default();
        let before = config.clone();

        assert!(!config.toggle("pomodoro-timer"));

        assert_eq!(config, before);
    }
}

#[cfg(test)]
mod minutes_migration_tests {
    use super::*;

    #[test]
    fn minutes_from_an_older_config_become_seconds() {
        let config = Config::parse_str("pomodoro_work_minutes = 40\npomodoro_break_minutes = 7\n");

        assert_eq!(config.pomodoro_work_seconds, 40 * 60);
        assert_eq!(config.pomodoro_break_seconds, 7 * 60);
    }

    #[test]
    fn seconds_win_when_a_config_carries_both() {
        let config = Config::parse_str(
            "pomodoro_work_minutes = 40\npomodoro_work_seconds = 90\n",
        );

        assert_eq!(config.pomodoro_work_seconds, 90);
    }

    #[test]
    fn each_duration_migrates_on_its_own() {
        let config = Config::parse_str(
            "pomodoro_work_seconds = 90\npomodoro_break_minutes = 7\n",
        );

        assert_eq!(config.pomodoro_work_seconds, 90);
        assert_eq!(config.pomodoro_break_seconds, 7 * 60);
    }

    #[test]
    fn a_config_without_either_keeps_the_defaults() {
        let config = Config::parse_str("font_size = 20.0\n");

        assert_eq!(config.pomodoro_work_seconds, 25 * 60);
        assert_eq!(config.pomodoro_break_seconds, 3 * 60);
    }

    /// Saving writes only the seconds form, so the old keys disappear the first
    /// time anything is toggled.
    #[test]
    fn saving_a_migrated_config_drops_the_minutes_keys() {
        let config = Config::parse_str("pomodoro_work_minutes = 40\n");

        let written = toml::to_string(&config).unwrap();

        assert!(written.contains("pomodoro_work_seconds = 2400"));
        assert!(!written.contains("minutes"));
    }
}
