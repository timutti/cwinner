use crate::celebration::CelebrationLevel;
use crate::config::AudioConfig;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub enum SoundKind {
    Mini,
    Milestone,
    Epic,
    Fanfare,
    Streak,
}

impl SoundKind {
    pub fn name(&self) -> &'static str {
        match self {
            SoundKind::Mini      => "mini",
            SoundKind::Milestone => "milestone",
            SoundKind::Epic      => "epic",
            SoundKind::Fanfare   => "fanfare",
            SoundKind::Streak    => "streak",
        }
    }
}

pub fn celebration_to_sound(level: &CelebrationLevel) -> Option<SoundKind> {
    match level {
        CelebrationLevel::Off => None,
        CelebrationLevel::Mini => Some(SoundKind::Mini),
        CelebrationLevel::Medium => Some(SoundKind::Milestone),
        CelebrationLevel::Epic => Some(SoundKind::Fanfare),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Player {
    Afplay,
    PwPlay,
    Paplay,
    Aplay,
    Mpg123,
    Mpg321,
}

pub fn detect_player() -> Option<Player> {
    let candidates: Vec<(Player, &str)> = if cfg!(target_os = "macos") {
        vec![(Player::Afplay, "afplay")]
    } else {
        vec![
            (Player::PwPlay, "pw-play"),
            (Player::Paplay, "paplay"),
            (Player::Aplay, "aplay"),
            (Player::Mpg123, "mpg123"),
            (Player::Mpg321, "mpg321"),
        ]
    };

    for (player, cmd) in candidates {
        if Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(player);
        }
    }
    None
}

pub fn play_sound(kind: &SoundKind, sound_pack: &str) {
    let Some(player) = detect_player() else { return };
    let sounds_dir = dirs::config_dir()
        .map(|d| d.join("cwinner").join("sounds"))
        .unwrap_or_else(|| PathBuf::from("/tmp/cwinner/sounds"));

    let cfg = AudioConfig {
        enabled: true,
        sound_pack: sound_pack.to_string(),
        volume: 0.8,
    };

    let Some(path) = find_sound_file(kind, &cfg, &sounds_dir) else { return };

    let path_str = match path.to_str() {
        Some(s) => s.to_string(),
        None => return,
    };

    let (cmd, args): (&str, Vec<String>) = match player {
        Player::Afplay => ("afplay", vec![path_str]),
        Player::PwPlay => ("pw-play", vec![path_str]),
        Player::Paplay => ("paplay", vec![path_str]),
        Player::Aplay => ("aplay", vec!["-q".into(), path_str]),
        Player::Mpg123 => ("mpg123", vec!["-q".into(), path_str]),
        Player::Mpg321 => ("mpg321", vec!["-q".into(), path_str]),
    };

    let _ = Command::new(cmd).args(&args).spawn();
}

pub fn find_sound_file(kind: &SoundKind, cfg: &AudioConfig, sounds_dir: &Path) -> Option<PathBuf> {
    let pack_dir = sounds_dir.join(&cfg.sound_pack);
    let name = kind.name();
    for ext in ["ogg", "wav", "mp3"] {
        let p = pack_dir.join(format!("{name}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    // Fallback: generate WAV to /tmp/cwinner/
    crate::sounds::ensure_sound_file(kind).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_player_returns_something_or_none() {
        let _player = detect_player();
    }

    #[test]
    fn test_sound_file_name() {
        assert_eq!(SoundKind::Mini.name(), "mini");
        assert_eq!(SoundKind::Milestone.name(), "milestone");
        assert_eq!(SoundKind::Epic.name(), "epic");
        assert_eq!(SoundKind::Fanfare.name(), "fanfare");
        assert_eq!(SoundKind::Streak.name(), "streak");
    }

    #[test]
    fn test_play_sound_generates_wav_when_no_pack() {
        // Provide a non-existent sound pack dir
        let tmp = tempfile::tempdir().unwrap();
        let cfg = AudioConfig {
            enabled: true,
            sound_pack: "nonexistent".to_string(),
            volume: 0.8,
        };
        // Should not panic/error even with no sound files
        let result = find_sound_file(&SoundKind::Mini, &cfg, tmp.path());
        assert!(result.is_some(), "should fall back to generated WAV");
    }
}
