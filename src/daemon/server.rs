use crate::audio::{celebration_to_sound, play_sound};
use crate::achievements::check_achievements;
use crate::celebration::{decide, xp_for_event, CelebrationLevel};
use crate::config::Config;
use crate::event::{Event, EventKind};
use crate::renderer::render;
use crate::state::State;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};

/// Duration milestones in minutes and their celebration levels
pub const DURATION_MILESTONES: &[(u64, CelebrationLevel)] = &[
    (60, CelebrationLevel::Medium),   // 1 hour
    (180, CelebrationLevel::Medium),  // 3 hours
    (480, CelebrationLevel::Epic),    // 8 hours
];

/// Runtime-only session tracking (not persisted to disk)
#[derive(Debug)]
pub struct SessionInfo {
    pub started_at: Instant,
    pub commits: u32,
    pub duration_milestones_fired: Vec<u64>, // minutes already celebrated
}

impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            commits: 0,
            duration_milestones_fired: Vec::new(),
        }
    }
}

impl SessionInfo {
    /// Check if any duration milestones have been crossed and return the highest
    /// new milestone's celebration level (if any).
    pub fn check_duration_milestones(&mut self) -> Option<CelebrationLevel> {
        let elapsed_minutes = self.started_at.elapsed().as_secs() / 60;
        let mut best_level: Option<CelebrationLevel> = None;

        for &(minutes, ref level) in DURATION_MILESTONES {
            if elapsed_minutes >= minutes
                && !self.duration_milestones_fired.contains(&minutes)
            {
                self.duration_milestones_fired.push(minutes);
                // Keep the highest-priority level (Epic > Medium > Mini > Off)
                best_level = Some(best_level.map_or(level.clone(), |b| b.max(level.clone())));
            }
        }

        best_level
    }

    #[cfg(test)]
    pub fn with_started_at(started_at: Instant) -> Self {
        Self {
            started_at,
            commits: 0,
            duration_milestones_fired: Vec::new(),
        }
    }
}

pub type SessionMap = HashMap<String, SessionInfo>;

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
    let sessions: Arc<Mutex<SessionMap>> =
        Arc::new(Mutex::new(HashMap::new()));

    eprintln!("cwinnerd listening on {}", path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        let cfg = Arc::clone(&cfg);
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state, cfg, sessions).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<State>>,
    cfg: Arc<Config>,
    sessions: Arc<Mutex<SessionMap>>,
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

    // Eventy — fire-and-forget
    if let Ok(event) = serde_json::from_str::<Event>(line) {
        let tty_path = event.tty_path.clone();

        // Track session info (commits + duration) for SessionEnd epic logic
        let (session_commit_count, duration_milestone_level) = {
            let mut sm = sessions.lock().unwrap_or_else(|e| e.into_inner());

            if event.event == EventKind::SessionEnd {
                // Check duration milestones one last time, then remove session
                let mut info = sm.remove(&event.session_id)
                    .unwrap_or_default();
                let dur_level = info.check_duration_milestones();
                (info.commits, dur_level)
            } else {
                // Ensure session exists
                let info = sm.entry(event.session_id.clone())
                    .or_default();

                if event.event == EventKind::GitCommit {
                    info.commits += 1;
                }

                // Check duration milestones on every event
                let dur_level = info.check_duration_milestones();

                (info.commits, dur_level)
            }
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

            // Duration milestone can upgrade celebration level
            if let Some(dur_level) = duration_milestone_level {
                level = level.max(dur_level);
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
                        play_sound(&sound, &cfg2.audio);
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
    fn test_process_event_task_completed_no_xp_by_default() {
        let mut state = crate::state::State::default();
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::TaskCompleted);
        process_event_with_state(&event, &mut state, &cfg);
        assert_eq!(state.xp, 0); // task_completed defaults to "off"
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
        let event = make_event(EventKind::GitCommit); // adds 25 XP (milestone) → level 2

        process_event_with_state(&event, &mut state, &cfg);

        assert!(state.achievements_unlocked.iter().any(|id| id == "level_2"));
    }

    #[test]
    fn test_streak_bonus_applied_in_process_event() {
        let mut state = crate::state::State::default();
        state.commit_streak_days = 5;
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        state.last_commit_date = Some(yesterday);
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::GitCommit);

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

    // --- Session duration milestone tests ---

    #[test]
    fn test_session_info_new_has_no_milestones_fired() {
        let info = SessionInfo::default();
        assert_eq!(info.commits, 0);
        assert!(info.duration_milestones_fired.is_empty());
    }

    #[test]
    fn test_duration_milestone_not_reached_before_60_min() {
        let started = Instant::now(); // just started
        let mut info = SessionInfo::with_started_at(started);
        let result = info.check_duration_milestones();
        assert!(result.is_none());
        assert!(info.duration_milestones_fired.is_empty());
    }

    #[test]
    fn test_duration_milestone_1h_fires_medium() {
        // Simulate session started 61 minutes ago
        let started = Instant::now() - std::time::Duration::from_secs(61 * 60);
        let mut info = SessionInfo::with_started_at(started);
        let result = info.check_duration_milestones();
        assert_eq!(result, Some(CelebrationLevel::Medium));
        assert!(info.duration_milestones_fired.contains(&60));
    }

    #[test]
    fn test_duration_milestone_1h_does_not_refire() {
        let started = Instant::now() - std::time::Duration::from_secs(61 * 60);
        let mut info = SessionInfo::with_started_at(started);

        // First check fires
        let result1 = info.check_duration_milestones();
        assert_eq!(result1, Some(CelebrationLevel::Medium));

        // Second check does NOT refire
        let result2 = info.check_duration_milestones();
        assert!(result2.is_none());
    }

    #[test]
    fn test_duration_milestone_3h_fires_medium() {
        let started = Instant::now() - std::time::Duration::from_secs(181 * 60);
        let mut info = SessionInfo::with_started_at(started);
        // Pre-fire the 1h milestone so we only see the 3h one
        info.duration_milestones_fired.push(60);

        let result = info.check_duration_milestones();
        assert_eq!(result, Some(CelebrationLevel::Medium));
        assert!(info.duration_milestones_fired.contains(&180));
    }

    #[test]
    fn test_duration_milestone_8h_fires_epic() {
        let started = Instant::now() - std::time::Duration::from_secs(481 * 60);
        let mut info = SessionInfo::with_started_at(started);
        // Pre-fire earlier milestones
        info.duration_milestones_fired.push(60);
        info.duration_milestones_fired.push(180);

        let result = info.check_duration_milestones();
        assert_eq!(result, Some(CelebrationLevel::Epic));
        assert!(info.duration_milestones_fired.contains(&480));
    }

    #[test]
    fn test_duration_milestone_multiple_at_once_returns_highest() {
        // Session started 4 hours ago, no milestones fired yet
        let started = Instant::now() - std::time::Duration::from_secs(241 * 60);
        let mut info = SessionInfo::with_started_at(started);

        // Both 60min and 180min crossed; should return Medium (highest of the two)
        let result = info.check_duration_milestones();
        assert_eq!(result, Some(CelebrationLevel::Medium));
        assert!(info.duration_milestones_fired.contains(&60));
        assert!(info.duration_milestones_fired.contains(&180));
    }

    #[test]
    fn test_duration_milestone_all_three_at_once_returns_epic() {
        // Session started 9 hours ago, no milestones fired
        let started = Instant::now() - std::time::Duration::from_secs(541 * 60);
        let mut info = SessionInfo::with_started_at(started);

        let result = info.check_duration_milestones();
        assert_eq!(result, Some(CelebrationLevel::Epic));
        assert_eq!(info.duration_milestones_fired.len(), 3);
    }

    #[test]
    fn test_celebration_level_max_picks_higher() {
        assert_eq!(CelebrationLevel::Off.max(CelebrationLevel::Medium), CelebrationLevel::Medium);
        assert_eq!(CelebrationLevel::Medium.max(CelebrationLevel::Epic), CelebrationLevel::Epic);
        assert_eq!(CelebrationLevel::Epic.max(CelebrationLevel::Medium), CelebrationLevel::Epic);
        assert_eq!(CelebrationLevel::Mini.max(CelebrationLevel::Medium), CelebrationLevel::Medium);
        assert_eq!(CelebrationLevel::Off.max(CelebrationLevel::Off), CelebrationLevel::Off);
    }

    #[test]
    fn test_session_cleanup_on_session_end() {
        // Verify that SessionInfo is properly removed when SessionEnd arrives
        let mut sessions: SessionMap = HashMap::new();
        let info = SessionInfo::default();
        sessions.insert("s1".into(), info);
        assert!(sessions.contains_key("s1"));

        // Simulate what handle_connection does on SessionEnd
        let removed = sessions.remove("s1");
        assert!(removed.is_some());
        assert!(!sessions.contains_key("s1"));
    }

    #[test]
    fn test_session_commits_tracked_in_session_info() {
        let mut info = SessionInfo::default();
        assert_eq!(info.commits, 0);
        info.commits += 1;
        assert_eq!(info.commits, 1);
        info.commits += 1;
        assert_eq!(info.commits, 2);
    }
}
