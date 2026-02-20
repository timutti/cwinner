use crate::celebration::CelebrationLevel;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum SoundKind {
    Mini,
    Milestone,
    Epic,
    Fanfare,
    Streak,
}

pub fn sound_file_for_level(kind: &SoundKind) -> &'static str {
    match kind {
        SoundKind::Mini => "mini",
        SoundKind::Milestone => "milestone",
        SoundKind::Epic => "epic",
        SoundKind::Fanfare => "fanfare",
        SoundKind::Streak => "streak",
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
    let Some(sound_dir) = dirs::config_dir()
        .map(|d| d.join("cwinner").join("sounds").join(sound_pack))
    else {
        return;
    };

    let base = sound_file_for_level(kind);
    let Some(path) = find_sound_file(&sound_dir, base) else { return };

    let path_str = match path.to_str() {
        Some(s) => s.to_string(),
        None => return,
    };

    let (cmd, args): (&str, Vec<String>) = match player {
        Player::Afplay => ("afplay", vec![path_str]),
        Player::Paplay => ("paplay", vec![path_str]),
        Player::Aplay => ("aplay", vec!["-q".into(), path_str]),
        Player::Mpg123 => ("mpg123", vec!["-q".into(), path_str]),
        Player::Mpg321 => ("mpg321", vec!["-q".into(), path_str]),
    };

    let _ = Command::new(cmd).args(&args).spawn();
}

fn find_sound_file(dir: &PathBuf, base: &str) -> Option<PathBuf> {
    for ext in &["ogg", "wav", "mp3"] {
        let p = dir.join(format!("{}.{}", base, ext));
        if p.exists() {
            return Some(p);
        }
    }
    None
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
        assert_eq!(sound_file_for_level(&SoundKind::Mini), "mini");
        assert_eq!(sound_file_for_level(&SoundKind::Milestone), "milestone");
        assert_eq!(sound_file_for_level(&SoundKind::Epic), "epic");
        assert_eq!(sound_file_for_level(&SoundKind::Fanfare), "fanfare");
        assert_eq!(sound_file_for_level(&SoundKind::Streak), "streak");
    }
}
