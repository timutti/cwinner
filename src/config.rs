use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Intensity {
    Off,
    Mini,
    Medium,
    Epic,
}

impl Intensity {
    fn off() -> Self { Self::Off }
    fn medium() -> Self { Self::Medium }
    fn epic() -> Self { Self::Epic }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntensityConfig {
    #[serde(default = "Intensity::off")]
    pub routine: Intensity,
    #[serde(default = "Intensity::medium")]
    pub milestone: Intensity,
    #[serde(default = "Intensity::epic")]
    pub breakthrough: Intensity,
}

impl Default for IntensityConfig {
    fn default() -> Self {
        Self {
            routine: Intensity::Off,
            milestone: Intensity::Medium,
            breakthrough: Intensity::Epic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_sound_pack")]
    pub sound_pack: String,
    #[serde(default = "default_volume")]
    pub volume: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self { enabled: true, sound_pack: "default".into(), volume: 0.8 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConfig {
    #[serde(default = "default_true")]
    pub confetti: bool,
    #[serde(default = "default_true")]
    pub splash_screen: bool,
    #[serde(default = "default_true")]
    pub progress_bar: bool,
    #[serde(default = "default_confetti_ms")]
    pub confetti_duration_ms: u64,
    #[serde(default = "default_splash_ms")]
    pub splash_duration_ms: u64,
}

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            confetti: true,
            splash_screen: true,
            progress_bar: true,
            confetti_duration_ms: 1500,
            splash_duration_ms: 2000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTrigger {
    pub name: String,
    pub pattern: String,
    pub intensity: Intensity,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriggersConfig {
    #[serde(default)]
    pub custom: Vec<CustomTrigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub intensity: IntensityConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub visual: VisualConfig,
    #[serde(default)]
    pub triggers: TriggersConfig,
}

fn default_true() -> bool { true }
fn default_sound_pack() -> String { "default".into() }
fn default_volume() -> f32 { 0.8 }
fn default_confetti_ms() -> u64 { 1500 }
fn default_splash_ms() -> u64 { 2000 }

impl Config {
    pub fn load() -> Self {
        config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn config_path() -> Option<PathBuf> {
        config_path()
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cwinner").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.intensity.milestone, Intensity::Medium);
        assert_eq!(cfg.intensity.routine, Intensity::Off);
        assert!(cfg.audio.enabled);
        assert!(cfg.visual.confetti);
    }

    #[test]
    fn test_parse_toml() {
        let toml_str = r#"
[intensity]
routine = "off"
milestone = "medium"
breakthrough = "epic"

[audio]
enabled = true
sound_pack = "default"
volume = 0.8

[visual]
confetti = true
splash_screen = true
progress_bar = true
confetti_duration_ms = 1500
splash_duration_ms = 2000
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.audio.volume, 0.8);
        assert_eq!(cfg.visual.confetti_duration_ms, 1500);
    }

    #[test]
    fn test_default_config_has_no_custom_triggers() {
        let cfg = Config::default();
        assert!(cfg.triggers.custom.is_empty());
    }

    #[test]
    fn test_parse_toml_with_custom_triggers() {
        let toml_str = r#"
[[triggers.custom]]
name = "deploy"
pattern = "git push.*production"
intensity = "epic"

[[triggers.custom]]
name = "test"
pattern = "cargo test"
intensity = "medium"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.triggers.custom.len(), 2);
        assert_eq!(cfg.triggers.custom[0].name, "deploy");
        assert_eq!(cfg.triggers.custom[0].pattern, "git push.*production");
        assert_eq!(cfg.triggers.custom[0].intensity, Intensity::Epic);
        assert_eq!(cfg.triggers.custom[1].name, "test");
        assert_eq!(cfg.triggers.custom[1].pattern, "cargo test");
        assert_eq!(cfg.triggers.custom[1].intensity, Intensity::Medium);
    }

    #[test]
    fn test_parse_toml_without_triggers_section() {
        let toml_str = r#"
[intensity]
routine = "mini"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.triggers.custom.is_empty());
    }
}
