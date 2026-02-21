use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Streak milestones that trigger special celebrations
pub const STREAK_MILESTONES: &[u32] = &[5, 10, 25, 100];

#[derive(Debug, Clone, PartialEq)]
pub struct CommitResult {
    pub first_today: bool,
    /// If the streak just hit a milestone (5, 10, 25, 100), contains the milestone value
    pub streak_milestone: Option<u32>,
}

pub const LEVELS: &[(u32, &str)] = &[
    (0,     "Vibe Initiate"),
    (100,   "Prompt Whisperer"),
    (500,   "Vibe Architect"),
    (1500,  "Flow State Master"),
    (5000,  "Claude Sensei"),
    (10000, "Code Whisperer"),
    (20000, "Vibe Lord"),
    (35000, "Zen Master"),
    (50000, "Transcendent"),
    (75000, "Singularity"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub xp: u32,
    pub level: u32,
    pub level_name: String,
    pub commits_total: u32,
    pub commit_streak_days: u32,
    pub last_commit_date: Option<NaiveDate>,
    pub sessions_total: u32,
    pub achievements_unlocked: Vec<String>,
    pub tools_used: HashSet<String>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub last_bash_exit: Option<i32>,
}

impl Default for State {
    fn default() -> Self {
        let (_, name) = LEVELS[0];
        Self {
            xp: 0,
            level: 1,
            level_name: name.to_string(),
            commits_total: 0,
            commit_streak_days: 0,
            last_commit_date: None,
            sessions_total: 0,
            achievements_unlocked: vec![],
            tools_used: HashSet::new(),
            last_event_at: None,
            last_bash_exit: None,
        }
    }
}

impl State {
    pub fn add_xp(&mut self, amount: u32) {
        self.xp += amount;
        self.update_level();
    }

    fn update_level(&mut self) {
        for (i, &(threshold, name)) in LEVELS.iter().enumerate().rev() {
            if self.xp >= threshold {
                self.level = (i + 1) as u32;
                self.level_name = name.to_string();
                break;
            }
        }
    }

    /// Returns CommitResult with first_today flag and optional streak milestone
    pub fn record_commit(&mut self) -> CommitResult {
        self.commits_total += 1;
        let today = Utc::now().date_naive();
        let first_today = self.last_commit_date.map(|d| d != today).unwrap_or(true);
        let old_streak = self.commit_streak_days;
        if first_today {
            let yesterday = today.pred_opt().unwrap();
            if self.last_commit_date == Some(yesterday) {
                self.commit_streak_days += 1;
            } else if self.last_commit_date != Some(today) {
                self.commit_streak_days = 1;
            }
            self.last_commit_date = Some(today);
        }
        let streak_milestone = if self.commit_streak_days != old_streak {
            STREAK_MILESTONES
                .iter()
                .find(|&&m| self.commit_streak_days == m)
                .copied()
        } else {
            None
        };
        CommitResult { first_today, streak_milestone }
    }

    /// Returns true if this is the first use of this tool
    pub fn record_tool_use(&mut self, tool: &str) -> bool {
        self.tools_used.insert(tool.to_string())
    }

    pub fn unlock_achievement(&mut self, id: &str) -> bool {
        if !self.achievements_unlocked.contains(&id.to_string()) {
            self.achievements_unlocked.push(id.to_string());
            true
        } else {
            false
        }
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&s)?)
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn state_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("cwinner").join("state.json"))
    }

    pub fn load() -> Self {
        Self::state_path()
            .and_then(|p| Self::load_from(&p).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::state_path() {
            let _ = self.save_to(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_state_defaults() {
        let s = State::default();
        assert_eq!(s.xp, 0);
        assert_eq!(s.level, 1);
        assert_eq!(s.level_name, "Vibe Initiate");
    }

    #[test]
    fn test_add_xp_no_level_up() {
        let mut s = State::default();
        s.add_xp(50);
        assert_eq!(s.xp, 50);
        assert_eq!(s.level, 1);
    }

    #[test]
    fn test_add_xp_level_up() {
        let mut s = State::default();
        s.add_xp(100);
        assert_eq!(s.level, 2);
        assert_eq!(s.level_name, "Prompt Whisperer");
    }

    #[test]
    fn test_persist_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut s = State::default();
        s.add_xp(250);
        s.save_to(&path).unwrap();
        let loaded = State::load_from(&path).unwrap();
        assert_eq!(loaded.xp, 250);
        assert_eq!(loaded.level, 2);
    }

    #[test]
    fn test_commit_streak() {
        let mut s = State::default();
        let result = s.record_commit();
        assert_eq!(s.commits_total, 1);
        assert_eq!(s.commit_streak_days, 1);
        assert!(result.first_today);
        assert_eq!(result.streak_milestone, None);
    }

    #[test]
    fn test_streak_milestone_at_5() {
        let mut s = State::default();
        // Simulate streak at 4, then commit extends to 5
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 4;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 5);
        assert_eq!(result.streak_milestone, Some(5));
    }

    #[test]
    fn test_streak_milestone_at_10() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 9;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 10);
        assert_eq!(result.streak_milestone, Some(10));
    }

    #[test]
    fn test_streak_milestone_at_25() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 24;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 25);
        assert_eq!(result.streak_milestone, Some(25));
    }

    #[test]
    fn test_streak_milestone_at_100() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 99;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 100);
        assert_eq!(result.streak_milestone, Some(100));
    }

    #[test]
    fn test_no_streak_milestone_at_6() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 5;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 6);
        assert_eq!(result.streak_milestone, None);
    }

    #[test]
    fn test_tool_first_use() {
        let mut s = State::default();
        assert!(s.record_tool_use("Task"));
        assert!(!s.record_tool_use("Task"));
    }
}
