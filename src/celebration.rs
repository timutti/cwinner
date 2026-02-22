use crate::config::{Config, Intensity};
use crate::event::{Event, EventKind};
use crate::state::State;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

/// Check if a bash command matches any custom trigger pattern (substring match).
/// Returns the intensity of the first matching trigger, or None.
pub fn check_custom_triggers(command: &str, cfg: &Config) -> Option<CelebrationLevel> {
    for trigger in &cfg.triggers.custom {
        if command.contains(&trigger.pattern) {
            return Some(CelebrationLevel::from(&trigger.intensity));
        }
    }
    None
}

/// Check if a Bash command string contains a `git commit` subcommand.
pub fn has_git_commit(command: &str) -> bool {
    command
        .split("&&")
        .flat_map(|s| s.split(';'))
        .flat_map(|s| s.split("||"))
        .any(|seg| {
            let t = seg.trim();
            t.starts_with("git commit ") || t == "git commit"
        })
}

/// Detect git commit/push from a Bash command string.
/// If both are present (e.g. `git commit && git push`), returns GitPush (higher priority).
pub fn detect_git_command(command: &str) -> Option<EventKind> {
    let mut found = None;
    for segment in command
        .split("&&")
        .flat_map(|s| s.split(';'))
        .flat_map(|s| s.split("||"))
    {
        let trimmed = segment.trim();
        if trimmed.starts_with("git push ") || trimmed == "git push" {
            return Some(EventKind::GitPush); // highest priority, return immediately
        }
        if trimmed.starts_with("git commit ") || trimmed == "git commit" {
            found = Some(EventKind::GitCommit);
        }
    }
    found
}

pub fn decide(event: &Event, _state: &State, cfg: &Config) -> CelebrationLevel {
    if event.event == EventKind::PostToolUse {
        if let Some(tool) = &event.tool {
            if tool == "Bash" {
                // Check custom triggers first — if a command matches, use trigger's intensity
                if let Some(command) = event.metadata.get("command").and_then(|v| v.as_str()) {
                    if let Some(level) = check_custom_triggers(command, cfg) {
                        return level;
                    }

                    // Detect git commit/push in successful Bash commands
                    if let Some(git_kind) = detect_git_command(command) {
                        return match git_kind {
                            EventKind::GitCommit => {
                                CelebrationLevel::from(&cfg.intensity.milestone)
                            }
                            EventKind::GitPush => {
                                CelebrationLevel::from(&cfg.intensity.breakthrough)
                            }
                            _ => unreachable!(),
                        };
                    }
                }

                return CelebrationLevel::from(&cfg.intensity.routine);
            }
            if tool == "Write" || tool == "Edit" || tool == "Read" {
                return CelebrationLevel::from(&cfg.intensity.routine);
            }
        }
    }

    match event.event {
        EventKind::TaskCompleted => CelebrationLevel::from(&cfg.intensity.task_completed),
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
    fn test_task_completed_is_medium_by_default() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_event(EventKind::TaskCompleted, None);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Medium);
    }

    #[test]
    fn test_routine_write_is_mini_by_default() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_event(EventKind::PostToolUse, Some("Write"));
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Mini);
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

    // --- Custom trigger tests ---

    fn make_bash_event_with_command(command: &str, exit_code: i64) -> Event {
        let mut meta = HashMap::new();
        meta.insert("exit_code".into(), serde_json::json!(exit_code));
        meta.insert("command".into(), serde_json::json!(command));
        Event {
            event: EventKind::PostToolUse,
            tool: Some("Bash".into()),
            session_id: "test".into(),
            tty_path: "/dev/null".into(),
            metadata: meta,
        }
    }

    fn config_with_triggers() -> Config {
        use crate::config::{CustomTrigger, Intensity, TriggersConfig};
        let mut cfg = Config::default();
        cfg.triggers = TriggersConfig {
            custom: vec![
                CustomTrigger {
                    name: "deploy".into(),
                    pattern: "git push".into(),
                    intensity: Intensity::Epic,
                },
                CustomTrigger {
                    name: "test".into(),
                    pattern: "cargo test".into(),
                    intensity: Intensity::Medium,
                },
            ],
        };
        cfg
    }

    #[test]
    fn test_custom_trigger_matches_deploy() {
        let cfg = config_with_triggers();
        let state = State::default();
        let event = make_bash_event_with_command("git push origin production", 0);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Epic);
    }

    #[test]
    fn test_custom_trigger_matches_test() {
        let cfg = config_with_triggers();
        let state = State::default();
        let event = make_bash_event_with_command("cargo test --release", 0);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Medium);
    }

    #[test]
    fn test_custom_trigger_no_match_falls_through() {
        let cfg = config_with_triggers();
        let state = State::default();
        let event = make_bash_event_with_command("ls -la", 0);
        let result = decide(&event, &state, &cfg);
        // No trigger matches, exit_code=0, no prev failure → routine (Mini by default)
        assert_eq!(result, CelebrationLevel::Mini);
    }

    #[test]
    fn test_custom_trigger_overrides_git_detection() {
        // Custom trigger takes priority over git command detection
        let cfg = config_with_triggers();
        let state = State::default();
        let event = make_bash_event_with_command("cargo test", 0);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Medium);
    }

    #[test]
    fn test_custom_trigger_first_match_wins() {
        use crate::config::{CustomTrigger, Intensity, TriggersConfig};
        let mut cfg = Config::default();
        cfg.triggers = TriggersConfig {
            custom: vec![
                CustomTrigger {
                    name: "first".into(),
                    pattern: "git".into(),
                    intensity: Intensity::Mini,
                },
                CustomTrigger {
                    name: "second".into(),
                    pattern: "git push".into(),
                    intensity: Intensity::Epic,
                },
            ],
        };
        let state = State::default();
        let event = make_bash_event_with_command("git push origin main", 0);
        let result = decide(&event, &state, &cfg);
        // First trigger matches "git" first
        assert_eq!(result, CelebrationLevel::Mini);
    }

    #[test]
    fn test_custom_trigger_no_triggers_configured() {
        let cfg = Config::default(); // empty triggers
        let state = State::default();
        let event = make_bash_event_with_command("git push origin main", 0);
        let result = decide(&event, &state, &cfg);
        // No triggers, but "git push" detected → breakthrough (Epic)
        assert_eq!(result, CelebrationLevel::Epic);
    }

    #[test]
    fn test_check_custom_triggers_function_directly() {
        let cfg = config_with_triggers();
        assert_eq!(
            check_custom_triggers("git push origin main", &cfg),
            Some(CelebrationLevel::Epic)
        );
        assert_eq!(
            check_custom_triggers("cargo test", &cfg),
            Some(CelebrationLevel::Medium)
        );
        assert_eq!(check_custom_triggers("echo hello", &cfg), None);
    }

    #[test]
    fn test_custom_trigger_non_bash_tool_not_affected() {
        let cfg = config_with_triggers();
        let state = State::default();
        // Write tool should not trigger custom trigger matching — returns routine level
        let event = make_event(EventKind::PostToolUse, Some("Write"));
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Mini);
    }

    // --- detect_git_command tests ---

    #[test]
    fn test_detect_git_push() {
        assert_eq!(
            detect_git_command("git push origin main"),
            Some(EventKind::GitPush)
        );
        assert_eq!(detect_git_command("git push"), Some(EventKind::GitPush));
    }

    #[test]
    fn test_detect_git_commit() {
        assert_eq!(
            detect_git_command("git commit -m \"msg\""),
            Some(EventKind::GitCommit)
        );
        assert_eq!(detect_git_command("git commit"), Some(EventKind::GitCommit));
    }

    #[test]
    fn test_detect_git_commit_and_push_chain() {
        // "git add . && git commit -m x && git push" — push wins (last git command)
        let result = detect_git_command("git add . && git commit -m 'fix' && git push origin main");
        assert_eq!(result, Some(EventKind::GitPush));
    }

    #[test]
    fn test_detect_no_git_command() {
        assert_eq!(detect_git_command("cargo test --release"), None);
        assert_eq!(detect_git_command("git status"), None);
        assert_eq!(detect_git_command("git diff"), None);
    }

    #[test]
    fn test_bash_git_commit_is_milestone() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_bash_event_with_command("git commit -m 'fix bug'", 0);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Medium); // milestone default
    }

    #[test]
    fn test_bash_git_push_is_epic() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_bash_event_with_command("git push origin master", 0);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Epic); // breakthrough default
    }

    #[test]
    fn test_bash_routine_command_is_mini() {
        let cfg = Config::default();
        let state = State::default();
        let event = make_bash_event_with_command("ls -la", 0);
        let result = decide(&event, &state, &cfg);
        assert_eq!(result, CelebrationLevel::Mini);
    }
}
