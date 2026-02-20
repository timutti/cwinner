use crate::config::{Config, Intensity};
use crate::event::{Event, EventKind};
use crate::state::State;

#[derive(Debug, Clone, PartialEq)]
pub enum CelebrationLevel {
    Off,
    Mini,
    Medium,
    Epic,
}

impl From<&Intensity> for CelebrationLevel {
    fn from(i: &Intensity) -> Self {
        match i {
            Intensity::Off => Self::Off,
            Intensity::Mini => Self::Mini,
            Intensity::Medium => Self::Medium,
            Intensity::Epic => Self::Epic,
        }
    }
}

pub fn decide(event: &Event, state: &State, cfg: &Config) -> CelebrationLevel {
    // Průlom: bash fail → pass
    if event.event == EventKind::PostToolUse {
        if let Some(tool) = &event.tool {
            if tool == "Bash" {
                let exit_code = event.metadata.get("exit_code")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1);
                let prev_failed = state.last_bash_exit.map(|c| c != 0).unwrap_or(false);
                if exit_code == 0 && prev_failed {
                    return CelebrationLevel::from(&cfg.intensity.breakthrough);
                }
                if exit_code == 0 {
                    return CelebrationLevel::from(&cfg.intensity.routine);
                }
                return CelebrationLevel::Off;
            }
            if tool == "Write" || tool == "Edit" || tool == "Read" {
                return CelebrationLevel::from(&cfg.intensity.routine);
            }
        }
    }

    match event.event {
        EventKind::TaskCompleted => CelebrationLevel::from(&cfg.intensity.milestone),
        EventKind::GitCommit => CelebrationLevel::from(&cfg.intensity.milestone),
        EventKind::GitPush => CelebrationLevel::from(&cfg.intensity.breakthrough),
        EventKind::SessionEnd => CelebrationLevel::from(&cfg.intensity.milestone),
        EventKind::PostToolUseFailure => CelebrationLevel::Off,
        _ => CelebrationLevel::from(&cfg.intensity.routine),
    }
}

pub fn xp_for_level(level: &CelebrationLevel) -> u32 {
    match level {
        CelebrationLevel::Off => 0,
        CelebrationLevel::Mini => 5,
        CelebrationLevel::Medium => 25,
        CelebrationLevel::Epic => 100,
    }
}

/// XP with 2× streak bonus when commit_streak_days >= 5
pub fn xp_for_event(level: &CelebrationLevel, state: &State) -> u32 {
    let base = xp_for_level(level);
    if base > 0 && state.commit_streak_days >= 5 {
        base * 2
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::event::{Event, EventKind};
    use crate::state::State;
    use std::collections::HashMap;

    fn make_event(kind: EventKind, tool: Option<&str>) -> Event {
        Event {
            event: kind,
            tool: tool.map(String::from),
            session_id: "test".into(),
            tty_path: "/dev/null".into(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_bash_success_after_failure_is_breakthrough() {
        let cfg = Config::default();
        let mut state = State::default();
        state.last_bash_exit = Some(1);

        let mut meta = HashMap::new();
        meta.insert("exit_code".into(), serde_json::json!(0));
        let event = Event {
            event: EventKind::PostToolUse,
            tool: Some("Bash".into()),
            session_id: "test".into(),
            tty_path: "/dev/null".into(),
            metadata: meta,
        };

        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Epic);
    }

    #[test]
    fn test_task_completed_is_milestone() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_event(EventKind::TaskCompleted, None);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Medium);
    }

    #[test]
    fn test_routine_write_is_off_by_default() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_event(EventKind::PostToolUse, Some("Write"));
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Off);
    }

    #[test]
    fn test_git_push_is_big() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_event(EventKind::GitPush, None);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Epic);
    }

    #[test]
    fn test_streak_bonus_doubles_xp() {
        let mut state = State::default();
        state.commit_streak_days = 5;
        assert_eq!(xp_for_event(&CelebrationLevel::Medium, &state), 50); // 25 * 2
    }

    #[test]
    fn test_no_streak_bonus_below_5_days() {
        let state = State::default(); // streak = 0
        assert_eq!(xp_for_event(&CelebrationLevel::Medium, &state), 25);
    }
}
