use crate::audio::{celebration_to_sound, play_sound};
use crate::achievements::check_achievements;
use crate::celebration::{decide, xp_for_event, CelebrationLevel};
use crate::config::Config;
use crate::event::{DaemonCommand, DaemonResponse, Event, EventKind};
use crate::renderer::render;
use crate::state::State;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub fn socket_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("cwinner")
        .join("cwinner.sock")
}

pub async fn run() -> anyhow::Result<()> {
    let path = socket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    let state = Arc::new(Mutex::new(State::load()));
    let cfg = Arc::new(Config::load());

    eprintln!("cwinnerd listening on {}", path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        let cfg = Arc::clone(&cfg);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state, cfg).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<State>>,
    cfg: Arc<Config>,
) -> anyhow::Result<()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.contains(&b'\n') {
            break;
        }
    }

    let line = String::from_utf8_lossy(&buf);
    let line = line.trim();

    // Příkazy (status/stats) — odpověz synchronně
    if let Ok(cmd) = serde_json::from_str::<DaemonCommand>(line) {
        let resp = handle_command(&cmd, &state);
        let json = serde_json::to_string(&resp)?;
        stream.write_all(json.as_bytes()).await?;
        return Ok(());
    }

    // Eventy — fire-and-forget
    if let Ok(event) = serde_json::from_str::<Event>(line) {
        let tty_path = event.tty_path.clone();
        // Process event under a single mutex lock, then clone state for rendering
        let (level, achievement_name, state_snapshot) = {
            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
            let (level, achievement_name) = process_event_with_state(&event, &mut s, &cfg);
            s.save();
            let snapshot = s.clone();
            (level, achievement_name, snapshot)
        };

        if level != CelebrationLevel::Off {
            let cfg2 = Arc::clone(&cfg);
            tokio::task::spawn_blocking(move || {
                if cfg2.audio.enabled {
                    if let Some(sound) = celebration_to_sound(&level) {
                        play_sound(&sound, &cfg2.audio.sound_pack);
                    }
                }
                render(&tty_path, &level, &state_snapshot, achievement_name.as_deref());
            });
        }
    }

    Ok(())
}

/// Process an event against the given state, returning the celebration level
/// and optionally the name of a newly unlocked achievement.
///
/// The caller is responsible for saving state and rendering visuals.
pub fn process_event_with_state(
    event: &Event,
    state: &mut State,
    cfg: &Config,
) -> (CelebrationLevel, Option<String>) {
    let level = decide(event, state, cfg);
    let xp = xp_for_event(&level, state);
    if xp > 0 {
        state.add_xp(xp);
    }
    if event.event == EventKind::GitCommit {
        state.record_commit();
    }
    if let Some(tool) = &event.tool {
        state.record_tool_use(tool);
    }
    // Check achievements BEFORE updating last_bash_exit (test_whisperer needs old value)
    let newly_unlocked = check_achievements(state, event);
    let achievement_name = newly_unlocked.first().map(|a| a.name.to_string());
    for a in &newly_unlocked {
        state.unlock_achievement(a.id);
    }
    // Update last_bash_exit AFTER achievements checked
    if event.event == EventKind::PostToolUse {
        if let Some(code) = event.metadata.get("exit_code").and_then(|v| v.as_i64()) {
            state.last_bash_exit = Some(code as i32);
        }
    }
    (level, achievement_name)
}

fn handle_command(cmd: &DaemonCommand, state: &Arc<Mutex<State>>) -> DaemonResponse {
    let s = state.lock().unwrap_or_else(|e| e.into_inner());
    match cmd {
        DaemonCommand::Status => DaemonResponse {
            ok: true,
            data: serde_json::json!({
                "running": true,
                "xp": s.xp,
                "level": s.level_name
            }),
        },
        DaemonCommand::Stats => DaemonResponse {
            ok: true,
            data: serde_json::to_value(&*s).unwrap_or_default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventKind};
    use std::collections::HashMap;

    fn make_event(kind: EventKind) -> Event {
        Event {
            event: kind,
            tool: None,
            session_id: "s1".into(),
            tty_path: "/dev/null".into(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_process_event_task_completed_adds_xp() {
        let mut state = crate::state::State::default();
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::TaskCompleted);
        process_event_with_state(&event, &mut state, &cfg);
        assert!(state.xp > 0);
    }

    #[test]
    fn test_process_event_git_commit_increments_commits() {
        let mut state = crate::state::State::default();
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::GitCommit);
        process_event_with_state(&event, &mut state, &cfg);
        assert_eq!(state.commits_total, 1);
    }

    #[test]
    fn test_first_commit_achievement_fires() {
        let mut state = crate::state::State::default();
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::GitCommit);

        process_event_with_state(&event, &mut state, &cfg);

        assert!(state.achievements_unlocked.iter().any(|id| id == "first_commit"));
    }

    #[test]
    fn test_level_up_achievement_fires() {
        let mut state = crate::state::State::default();
        state.xp = 95; // just below level 2 (100 XP)
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::TaskCompleted); // adds 25 XP → level 2

        process_event_with_state(&event, &mut state, &cfg);

        assert!(state.achievements_unlocked.iter().any(|id| id == "level_2"));
    }

    #[test]
    fn test_streak_bonus_applied_in_process_event() {
        let mut state = crate::state::State::default();
        state.commit_streak_days = 5;
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::TaskCompleted);

        process_event_with_state(&event, &mut state, &cfg);

        // 25 XP * 2 streak bonus = 50 XP
        assert_eq!(state.xp, 50);
    }
}
