use crate::audio::{celebration_to_sound, play_sound};
use crate::achievements::check_achievements;
use crate::celebration::{decide, xp_for_event, CelebrationLevel};
use crate::config::Config;
use crate::event::{DaemonCommand, DaemonResponse, Event, EventKind};
use crate::renderer::render;
use crate::state::State;
use std::collections::HashMap;
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
    let session_commits: Arc<Mutex<HashMap<String, u32>>> =
        Arc::new(Mutex::new(HashMap::new()));

    eprintln!("cwinnerd listening on {}", path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        let cfg = Arc::clone(&cfg);
        let session_commits = Arc::clone(&session_commits);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state, cfg, session_commits).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<State>>,
    cfg: Arc<Config>,
    session_commits: Arc<Mutex<HashMap<String, u32>>>,
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

        // Track session commits for SessionEnd epic logic
        let session_commit_count = if event.event == EventKind::GitCommit {
            let mut sc = session_commits.lock().unwrap_or_else(|e| e.into_inner());
            let count = sc.entry(event.session_id.clone()).or_insert(0);
            *count += 1;
            *count
        } else if event.event == EventKind::SessionEnd {
            let mut sc = session_commits.lock().unwrap_or_else(|e| e.into_inner());
            sc.remove(&event.session_id).unwrap_or(0)
        } else {
            0
        };

        // Process event under a single mutex lock, then clone state for rendering
        let (level, achievement_name, is_streak_milestone, state_snapshot) = {
            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
            let (mut level, achievement_name, is_streak_milestone) =
                process_event_with_state(&event, &mut s, &cfg);

            // SessionEnd with >=1 commit in this session → upgrade to Epic
            if event.event == EventKind::SessionEnd && session_commit_count >= 1 {
                level = CelebrationLevel::Epic;
            }

            s.save();
            let snapshot = s.clone();
            (level, achievement_name, is_streak_milestone, snapshot)
        };

        eprintln!("[cwinnerd] event={:?} tool={:?} level={:?} achievement={:?} streak_milestone={:?}",
            event.event, event.tool, level, achievement_name, is_streak_milestone);

        if level != CelebrationLevel::Off {
            let cfg2 = Arc::clone(&cfg);
            tokio::task::spawn_blocking(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let Some(guard) = crate::renderer::acquire_render_slot() else {
                    eprintln!("[cwinnerd] SKIPPED (cooldown)");
                    return;
                };
                eprintln!("[cwinnerd] RENDERING level={:?}", level);
                if cfg2.audio.enabled {
                    if let Some(sound) = celebration_to_sound(&level, achievement_name.is_some(), is_streak_milestone) {
                        play_sound(&sound, &cfg2.audio.sound_pack);
                    }
                }
                render(&tty_path, &level, &state_snapshot, achievement_name.as_deref());
                crate::renderer::finish_render(guard);
            });
        }
    }

    Ok(())
}

/// Process an event against the given state, returning the celebration level,
/// optionally the name of a newly unlocked achievement, and whether a streak
/// milestone was hit.
///
/// The caller is responsible for saving state and rendering visuals.
pub fn process_event_with_state(
    event: &Event,
    state: &mut State,
    cfg: &Config,
) -> (CelebrationLevel, Option<String>, bool) {
    let mut level = decide(event, state, cfg);
    let xp = xp_for_event(&level, state);
    if xp > 0 {
        state.add_xp(xp);
    }
    let mut is_streak_milestone = false;
    if event.event == EventKind::GitCommit {
        let commit_result = state.record_commit();
        if commit_result.streak_milestone.is_some() {
            is_streak_milestone = true;
            level = CelebrationLevel::Epic;
        }
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
    (level, achievement_name, is_streak_milestone)
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

    #[test]
    fn test_streak_milestone_upgrades_to_epic() {
        let mut state = crate::state::State::default();
        // Set up: streak at 4, yesterday was last commit
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        state.last_commit_date = Some(yesterday);
        state.commit_streak_days = 4;
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::GitCommit);

        let (level, _, is_streak) = process_event_with_state(&event, &mut state, &cfg);

        assert_eq!(level, CelebrationLevel::Epic);
        assert!(is_streak);
        assert_eq!(state.commit_streak_days, 5);
    }

    #[test]
    fn test_no_streak_milestone_at_non_milestone() {
        let mut state = crate::state::State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        state.last_commit_date = Some(yesterday);
        state.commit_streak_days = 5; // going to 6, not a milestone
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::GitCommit);

        let (_, _, is_streak) = process_event_with_state(&event, &mut state, &cfg);

        assert!(!is_streak);
    }

    #[test]
    fn test_process_event_returns_is_streak_false_for_non_commit() {
        let mut state = crate::state::State::default();
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::TaskCompleted);

        let (_, _, is_streak) = process_event_with_state(&event, &mut state, &cfg);

        assert!(!is_streak);
    }
}
