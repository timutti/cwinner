# cwinner Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Sestavit cwinnerd Rust daemon + cwinner CLI, který oslavuje úspěchy v Claude Code pomocí zvuků, ASCII konfet, splash screenů a progress baru přes TTY rendering.

**Architecture:** Tokio async daemon naslouchá na Unix socketu, přijímá JSON eventy z hook skriptů (Claude Code + git), rozhoduje o intenzitě oslavy a renderuje efekty přímo do TTY kde event vznikl. Stav (XP, streaky) persistuje do JSON souboru.

**Tech Stack:** Rust, tokio, serde_json, toml, crossterm, clap. Žádné runtime závislosti — vše staticky linkováno.

---

## Task 1: Cargo.toml a projekt scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

**Step 1: Inicializuj Cargo projekt**

```bash
cargo init --name cwinner
```

Expected: `Cargo.toml` + `src/main.rs` vytvořeny.

**Step 2: Přepiš `Cargo.toml`**

```toml
[package]
name = "cwinner"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "cwinner"
path = "src/main.rs"

[[bin]]
name = "cwinnerd"
path = "src/daemon_main.rs"

[lib]
name = "cwinner_lib"
path = "src/lib.rs"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
rand = "0.8"
dirs = "5"

[dev-dependencies]
tempfile = "3"
```

**Step 3: Vytvoř `src/lib.rs`**

```rust
pub mod config;
pub mod event;
pub mod state;
pub mod celebration;
pub mod renderer;
pub mod audio;
pub mod install;
```

**Step 4: Vytvoř prázdné moduly**

```bash
mkdir -p src/hooks/templates sounds/default
touch src/config.rs src/event.rs src/state.rs src/celebration.rs src/renderer.rs src/audio.rs src/install.rs src/daemon_main.rs
```

**Step 5: Ověř že projekt zkompiluje**

```bash
cargo build 2>&1 | head -20
```

Expected: Kompiluje (možná unused warnings, to je OK).

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: initial project scaffold"
```

---

## Task 2: IPC Event typy

**Files:**
- Modify: `src/event.rs`

**Step 1: Napiš test pro deserializaci eventu**

```rust
// src/event.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_post_tool_use() {
        let json = r#"{
            "event": "PostToolUse",
            "tool": "Bash",
            "session_id": "abc123",
            "tty_path": "/dev/pts/3",
            "metadata": {"exit_code": 0}
        }"#;
        let e: Event = serde_json::from_str(json).unwrap();
        assert_eq!(e.event, EventKind::PostToolUse);
        assert_eq!(e.tty_path, "/dev/pts/3");
    }

    #[test]
    fn test_deserialize_task_completed() {
        let json = r#"{
            "event": "TaskCompleted",
            "tool": null,
            "session_id": "xyz",
            "tty_path": "/dev/ttys001",
            "metadata": {}
        }"#;
        let e: Event = serde_json::from_str(json).unwrap();
        assert_eq!(e.event, EventKind::TaskCompleted);
    }
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test event 2>&1 | tail -20
```

Expected: FAIL — typy neexistují.

**Step 3: Implementuj typy**

```rust
// src/event.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum EventKind {
    PostToolUse,
    PostToolUseFailure,
    TaskCompleted,
    SessionEnd,
    GitCommit,
    GitPush,
    UserDefined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event: EventKind,
    pub tool: Option<String>,
    pub session_id: String,
    pub tty_path: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Interní příkazy daemonovi (status, stats)
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum DaemonCommand {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "stats")]
    Stats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub ok: bool,
    pub data: serde_json::Value,
}
```

**Step 4: Spusť test — ověř pass**

```bash
cargo test event 2>&1 | tail -10
```

Expected: `test event::tests::test_deserialize_post_tool_use ... ok`

**Step 5: Commit**

```bash
git add src/event.rs
git commit -m "feat: IPC event types with serde"
```

---

## Task 3: Config modul

**Files:**
- Modify: `src/config.rs`

**Step 1: Napiš testy**

```rust
// src/config.rs
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
        let toml = r#"
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
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.audio.volume, 0.8);
        assert_eq!(cfg.visual.confetti_duration_ms, 1500);
    }
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test config 2>&1 | tail -10
```

**Step 3: Implementuj**

```rust
// src/config.rs
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

impl Intensity {
    fn off() -> Self { Self::Off }
    fn medium() -> Self { Self::Medium }
    fn epic() -> Self { Self::Epic }
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub intensity: IntensityConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub visual: VisualConfig,
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
```

**Step 4: Spusť test — ověř pass**

```bash
cargo test config 2>&1 | tail -10
```

**Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: config module with TOML parsing"
```

---

## Task 4: State engine

**Files:**
- Modify: `src/state.rs`

**Step 1: Napiš testy**

```rust
// src/state.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_state_path(dir: &std::path::Path) -> std::path::PathBuf {
        dir.join("state.json")
    }

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
        let path = test_state_path(dir.path());
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
        s.record_commit();
        assert_eq!(s.commits_total, 1);
        assert_eq!(s.commit_streak_days, 1);
    }

    #[test]
    fn test_tool_first_use() {
        let mut s = State::default();
        assert!(s.record_tool_use("Task"));
        assert!(!s.record_tool_use("Task")); // druhé použití není "první"
    }
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test state 2>&1 | tail -10
```

**Step 3: Implementuj**

```rust
// src/state.rs
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const LEVELS: &[(u32, &str)] = &[
    (0,    "Vibe Initiate"),
    (100,  "Prompt Whisperer"),
    (500,  "Vibe Architect"),
    (1500, "Flow State Master"),
    (5000, "Claude Sensei"),
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

    /// Vrátí true pokud je to první commit dnes
    pub fn record_commit(&mut self) -> bool {
        self.commits_total += 1;
        let today = Utc::now().date_naive();
        let first_today = self.last_commit_date.map(|d| d != today).unwrap_or(true);
        if first_today {
            let yesterday = today.pred_opt().unwrap();
            if self.last_commit_date == Some(yesterday) {
                self.commit_streak_days += 1;
            } else if self.last_commit_date != Some(today) {
                self.commit_streak_days = 1;
            }
            self.last_commit_date = Some(today);
        }
        first_today
    }

    /// Vrátí true pokud je to první použití tohoto nástroje
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
```

Přidej `anyhow = "1"` do `[dependencies]` v `Cargo.toml`.

**Step 4: Spusť test — ověř pass**

```bash
cargo test state 2>&1 | tail -15
```

**Step 5: Commit**

```bash
git add src/state.rs Cargo.toml
git commit -m "feat: state engine with XP, levels, streaks"
```

---

## Task 5: Celebration engine

**Files:**
- Modify: `src/celebration.rs`

**Step 1: Napiš testy**

```rust
// src/celebration.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Intensity};
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
        state.last_bash_exit = Some(1); // předchozí selhal

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
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test celebration 2>&1 | tail -10
```

**Step 3: Implementuj**

```rust
// src/celebration.rs
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
                // Rutina
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
```

**Step 4: Spusť test — ověř pass**

```bash
cargo test celebration 2>&1 | tail -15
```

**Step 5: Commit**

```bash
git add src/celebration.rs
git commit -m "feat: celebration engine with contextual decisions"
```

---

## Task 6: TTY Renderer — progress bar

**Files:**
- Modify: `src/renderer.rs`

**Step 1: Napiš test pro progress bar string**

```rust
// src/renderer.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xp_bar_empty() {
        let bar = xp_bar_string(0, 100, 20);
        assert_eq!(bar.len(), 20);
        assert!(bar.chars().all(|c| c == '░'));
    }

    #[test]
    fn test_xp_bar_half() {
        let bar = xp_bar_string(50, 100, 20);
        let filled = bar.chars().filter(|&c| c == '█').count();
        assert_eq!(filled, 10);
    }

    #[test]
    fn test_xp_bar_full() {
        let bar = xp_bar_string(100, 100, 20);
        assert!(bar.chars().all(|c| c == '█'));
    }

    #[test]
    fn test_level_thresholds() {
        assert_eq!(xp_for_next_level(1), 100);
        assert_eq!(xp_for_next_level(2), 500);
        assert_eq!(xp_for_next_level(3), 1500);
    }
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test renderer 2>&1 | tail -10
```

**Step 3: Implementuj pomocné funkce + renderer**

```rust
// src/renderer.rs
use crate::celebration::CelebrationLevel;
use crate::state::State;
use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use rand::Rng;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

const LEVEL_THRESHOLDS: &[u32] = &[0, 100, 500, 1500, 5000, u32::MAX];
const CONFETTI_CHARS: &[char] = &['✦', '★', '♦', '●', '*', '+', '#', '✿', '❋'];
const CONFETTI_COLORS: &[Color] = &[
    Color::Red, Color::Green, Color::Yellow, Color::Blue,
    Color::Magenta, Color::Cyan, Color::White,
];

pub fn xp_bar_string(current_xp: u32, next_xp: u32, width: usize) -> String {
    let ratio = if next_xp == 0 { 1.0 } else { current_xp as f64 / next_xp as f64 };
    let filled = ((ratio * width as f64).round() as usize).min(width);
    format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(width - filled)
    )
}

pub fn xp_for_next_level(level: u32) -> u32 {
    LEVEL_THRESHOLDS.get(level as usize).copied().unwrap_or(u32::MAX)
}

pub fn render(tty_path: &str, level: &CelebrationLevel, state: &State, achievement: Option<&str>) {
    match level {
        CelebrationLevel::Off => {}
        CelebrationLevel::Mini => { let _ = render_progress_bar(tty_path, state); }
        CelebrationLevel::Medium => {
            let _ = render_progress_bar(tty_path, state);
        }
        CelebrationLevel::Epic => {
            let _ = render_confetti(tty_path, state);
            let _ = render_splash(tty_path, state, achievement.unwrap_or("ACHIEVEMENT UNLOCKED!"));
        }
    }
}

fn open_tty(tty_path: &str) -> io::Result<impl Write> {
    OpenOptions::new().write(true).open(tty_path)
}

pub fn render_progress_bar(tty_path: &str, state: &State) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let next_xp = xp_for_next_level(state.level);
    let xp_in_level = state.xp.saturating_sub(LEVEL_THRESHOLDS.get((state.level - 1) as usize).copied().unwrap_or(0));
    let xp_needed = next_xp.saturating_sub(LEVEL_THRESHOLDS.get((state.level - 1) as usize).copied().unwrap_or(0));
    let bar = xp_bar_string(xp_in_level, xp_needed, 20);

    queue!(tty,
        cursor::SavePosition,
        cursor::MoveTo(0, terminal::size().map(|s| s.1.saturating_sub(1)).unwrap_or(24)),
        Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Cyan),
        Print(format!(" ⚡ {} │ {} │ {} XP ", state.level_name, bar, state.xp)),
        ResetColor,
        cursor::RestorePosition,
    )?;
    tty.flush()?;

    thread::sleep(Duration::from_millis(3000));

    queue!(tty,
        cursor::SavePosition,
        cursor::MoveTo(0, terminal::size().map(|s| s.1.saturating_sub(1)).unwrap_or(24)),
        Clear(ClearType::CurrentLine),
        cursor::RestorePosition,
    )?;
    tty.flush()
}

pub fn render_confetti(tty_path: &str, _state: &State) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let mut rng = rand::thread_rng();
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let duration_ms = 1500u64;
    let frames = 15;
    let frame_ms = duration_ms / frames;

    execute!(tty, cursor::SavePosition, cursor::Hide)?;

    for _ in 0..frames {
        for _ in 0..(cols / 4) {
            let col = rng.gen_range(0..cols);
            let row = rng.gen_range(0..rows.saturating_sub(2));
            let ch = CONFETTI_CHARS[rng.gen_range(0..CONFETTI_CHARS.len())];
            let color = CONFETTI_COLORS[rng.gen_range(0..CONFETTI_COLORS.len())];
            queue!(tty,
                cursor::MoveTo(col, row),
                SetForegroundColor(color),
                Print(ch),
            )?;
        }
        tty.flush()?;
        thread::sleep(Duration::from_millis(frame_ms));
    }

    execute!(tty, Clear(ClearType::All), cursor::Show, cursor::RestorePosition, ResetColor)
}

pub fn render_splash(tty_path: &str, state: &State, achievement: &str) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let mid_row = rows / 2;
    let mid_col = cols / 2;

    execute!(tty, cursor::SavePosition, cursor::Hide, Clear(ClearType::All))?;

    let border = "═".repeat(cols as usize - 2);
    let top = format!("╔{}╗", border);
    let bot = format!("╚{}╝", border);

    queue!(tty,
        cursor::MoveTo(0, mid_row.saturating_sub(3)),
        SetForegroundColor(Color::Yellow),
        Print(&top),
        cursor::MoveTo(0, mid_row.saturating_sub(2)),
        Print(format!("║{:^width$}║", "", width = (cols - 2) as usize)),
        cursor::MoveTo(0, mid_row.saturating_sub(1)),
        SetForegroundColor(Color::Green),
        Print(format!("║{:^width$}║", achievement, width = (cols - 2) as usize)),
        cursor::MoveTo(0, *mid_row as u16),
        SetForegroundColor(Color::Cyan),
        Print(format!("║{:^width$}║", format!("Lvl {} {} ✦ {} XP", state.level, state.level_name, state.xp), width = (cols - 2) as usize)),
        cursor::MoveTo(0, mid_row + 1),
        SetForegroundColor(Color::Yellow),
        Print(format!("║{:^width$}║", "", width = (cols - 2) as usize)),
        cursor::MoveTo(0, mid_row + 2),
        Print(&bot),
        ResetColor,
    )?;
    tty.flush()?;

    thread::sleep(Duration::from_millis(2000));

    execute!(tty, Clear(ClearType::All), cursor::Show, cursor::RestorePosition, ResetColor)
}
```

**Step 4: Spusť test — ověř pass**

```bash
cargo test renderer 2>&1 | tail -15
```

**Step 5: Commit**

```bash
git add src/renderer.rs
git commit -m "feat: TTY renderer with confetti, splash, progress bar"
```

---

## Task 7: Audio engine

**Files:**
- Modify: `src/audio.rs`

**Step 1: Napiš test pro player detection**

```rust
// src/audio.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_player_returns_something_or_none() {
        // Jen ověříme že funkce nepanikuje
        let _player = detect_player();
        // OK i když vrátí None (CI nemá audio)
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
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test audio 2>&1 | tail -10
```

**Step 3: Implementuj**

```rust
// src/audio.rs
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
    Afplay,   // macOS
    Paplay,   // Linux PulseAudio
    Aplay,    // Linux ALSA
    Mpg123,
    Mpg321,
}

pub fn detect_player() -> Option<Player> {
    let candidates = if cfg!(target_os = "macos") {
        vec![Player::Afplay]
    } else {
        vec![Player::Paplay, Player::Aplay, Player::Mpg123, Player::Mpg321]
    };

    for player in candidates {
        let cmd = match &player {
            Player::Afplay => "afplay",
            Player::Paplay => "paplay",
            Player::Aplay => "aplay",
            Player::Mpg123 => "mpg123",
            Player::Mpg321 => "mpg321",
        };
        if Command::new("which").arg(cmd).output().map(|o| o.status.success()).unwrap_or(false) {
            return Some(player);
        }
    }
    None
}

pub fn play_sound(kind: &SoundKind, sound_pack: &str) {
    let Some(player) = detect_player() else { return };
    let Some(sound_dir) = dirs::config_dir().map(|d| d.join("cwinner").join("sounds").join(sound_pack)) else { return };

    let base = sound_file_for_level(kind);
    let path = find_sound_file(&sound_dir, base);
    let Some(path) = path else { return };

    let (cmd, args): (&str, Vec<&str>) = match player {
        Player::Afplay => ("afplay", vec![path.to_str().unwrap_or("")]),
        Player::Paplay => ("paplay", vec![path.to_str().unwrap_or("")]),
        Player::Aplay => ("aplay", vec!["-q", path.to_str().unwrap_or("")]),
        Player::Mpg123 => ("mpg123", vec!["-q", path.to_str().unwrap_or("")]),
        Player::Mpg321 => ("mpg321", vec!["-q", path.to_str().unwrap_or("")]),
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
```

**Step 4: Spusť test — ověř pass**

```bash
cargo test audio 2>&1 | tail -10
```

**Step 5: Commit**

```bash
git add src/audio.rs
git commit -m "feat: audio engine with afplay/aplay/paplay fallback chain"
```

---

## Task 8: Unix socket server (daemon core)

**Files:**
- Create: `src/daemon/mod.rs`
- Create: `src/daemon/server.rs`
- Modify: `src/lib.rs`

**Step 1: Napiš test pro event processing loop**

```rust
// src/daemon/server.rs
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
        process_event_with_state(&event, &mut state, &cfg, false);
        assert!(state.xp > 0);
    }

    #[test]
    fn test_process_event_git_commit_increments_commits() {
        let mut state = crate::state::State::default();
        let cfg = crate::config::Config::default();
        let event = make_event(EventKind::GitCommit);
        process_event_with_state(&event, &mut state, &cfg, false);
        assert_eq!(state.commits_total, 1);
    }
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test daemon 2>&1 | tail -10
```

**Step 3: Vytvoř `src/daemon/mod.rs`**

```rust
// src/daemon/mod.rs
pub mod server;
pub use server::run;
```

**Step 4: Implementuj server**

```rust
// src/daemon/server.rs
use crate::audio::{celebration_to_sound, play_sound};
use crate::celebration::{decide, xp_for_level, CelebrationLevel};
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
        if n == 0 { break; }
        buf.extend_from_slice(&tmp[..n]);
        if buf.contains(&b'\n') { break; }
    }

    let line = String::from_utf8_lossy(&buf);
    let line = line.trim();

    // Příkazy (status/stats)
    if let Ok(cmd) = serde_json::from_str::<DaemonCommand>(line) {
        let resp = handle_command(&cmd, &state);
        let json = serde_json::to_string(&resp)?;
        stream.write_all(json.as_bytes()).await?;
        return Ok(());
    }

    // Eventy (fire-and-forget)
    if let Ok(event) = serde_json::from_str::<Event>(line) {
        let tty_path = event.tty_path.clone();
        let (level, achievement) = {
            let mut s = state.lock().unwrap();
            let level = decide(&event, &s, &cfg);
            let xp = xp_for_level(&level);
            if xp > 0 { s.add_xp(xp); }
            if event.event == EventKind::GitCommit { s.record_commit(); }
            if let Some(tool) = &event.tool { s.record_tool_use(tool); }
            if event.event == EventKind::PostToolUse {
                if let Some(code) = event.metadata.get("exit_code").and_then(|v| v.as_i64()) {
                    s.last_bash_exit = Some(code as i32);
                }
            }
            s.save();
            let achievement = format!("{:?}", event.event);
            (level, achievement)
        };

        if level != CelebrationLevel::Off {
            let cfg2 = Arc::clone(&cfg);
            tokio::task::spawn_blocking(move || {
                if cfg2.audio.enabled {
                    if let Some(sound) = celebration_to_sound(&level) {
                        play_sound(&sound, &cfg2.audio.sound_pack);
                    }
                }
                render(&tty_path, &level, &State::load(), Some(&achievement));
            });
        }
    }

    Ok(())
}

pub fn process_event_with_state(event: &Event, state: &mut State, cfg: &Config, render_visual: bool) {
    let level = decide(event, state, cfg);
    let xp = xp_for_level(&level);
    if xp > 0 { state.add_xp(xp); }
    if event.event == EventKind::GitCommit { state.record_commit(); }
    if let Some(tool) = &event.tool { state.record_tool_use(tool); }
    if render_visual && level != CelebrationLevel::Off {
        render(&event.tty_path, &level, state, None);
    }
}

fn handle_command(cmd: &DaemonCommand, state: &Arc<Mutex<State>>) -> DaemonResponse {
    let s = state.lock().unwrap();
    match cmd {
        DaemonCommand::Status => DaemonResponse {
            ok: true,
            data: serde_json::json!({ "running": true, "xp": s.xp, "level": s.level_name }),
        },
        DaemonCommand::Stats => DaemonResponse {
            ok: true,
            data: serde_json::to_value(&*s).unwrap_or_default(),
        },
    }
}
```

Přidej do `src/lib.rs`:
```rust
pub mod daemon;
```

**Step 5: Spusť test — ověř pass**

```bash
cargo test daemon 2>&1 | tail -15
```

**Step 6: Commit**

```bash
git add src/daemon/ src/lib.rs
git commit -m "feat: tokio unix socket daemon server"
```

---

## Task 9: Hook šablony

**Files:**
- Create: `src/hooks/templates/post_tool_use.sh`
- Create: `src/hooks/templates/task_completed.sh`
- Create: `src/hooks/templates/session_end.sh`
- Create: `src/hooks/templates/git_post_commit.sh`
- Create: `src/hooks/templates/git_post_push.sh`

**Step 1: Vytvoř hook pro PostToolUse**

```bash
# src/hooks/templates/post_tool_use.sh
#!/usr/bin/env bash
# cwinner hook: PostToolUse
# Vstup: Claude Code posílá JSON na stdin

set -euo pipefail

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"

# Přečti stdin od Claude Code
INPUT=$(cat)

# Extrahuj metadata pro Bash tool
TOOL=$(echo "$INPUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_name',''))" 2>/dev/null || echo "")
EXIT_CODE=$(echo "$INPUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_response',{}).get('exit_code',0))" 2>/dev/null || echo "0")
SESSION_ID="${CLAUDE_SESSION_ID:-unknown}"

# Sestav event
EVENT=$(python3 -c "
import json, sys
print(json.dumps({
    'event': 'PostToolUse',
    'tool': '$TOOL',
    'session_id': '$SESSION_ID',
    'tty_path': '$TTY_PATH',
    'metadata': {'exit_code': int('$EXIT_CODE' or 0)}
}))
")

# Odešli daemonovi (non-blocking)
if [ -S "$SOCKET" ]; then
    echo "$EVENT" | socat -t 0.1 - "UNIX-CONNECT:$SOCKET" &>/dev/null &
fi
```

**Step 2: Vytvoř hook pro TaskCompleted**

```bash
# src/hooks/templates/task_completed.sh
#!/usr/bin/env bash
# cwinner hook: TaskCompleted

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"
SESSION_ID="${CLAUDE_SESSION_ID:-unknown}"

EVENT=$(python3 -c "import json; print(json.dumps({'event':'TaskCompleted','tool':None,'session_id':'$SESSION_ID','tty_path':'$TTY_PATH','metadata':{}}))")

if [ -S "$SOCKET" ]; then
    echo "$EVENT" | socat -t 0.1 - "UNIX-CONNECT:$SOCKET" &>/dev/null &
fi
```

**Step 3: Vytvoř hook pro SessionEnd**

```bash
# src/hooks/templates/session_end.sh
#!/usr/bin/env bash
# cwinner hook: SessionEnd

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"
SESSION_ID="${CLAUDE_SESSION_ID:-unknown}"

EVENT=$(python3 -c "import json; print(json.dumps({'event':'SessionEnd','tool':None,'session_id':'$SESSION_ID','tty_path':'$TTY_PATH','metadata':{}}))")

if [ -S "$SOCKET" ]; then
    echo "$EVENT" | socat -t 0.1 - "UNIX-CONNECT:$SOCKET" &>/dev/null &
fi
```

**Step 4: Vytvoř git post-commit hook**

```bash
# src/hooks/templates/git_post_commit.sh
#!/usr/bin/env bash
# cwinner git hook: post-commit

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"

EVENT=$(python3 -c "
import json
print(json.dumps({
    'event': 'GitCommit',
    'tool': None,
    'session_id': 'git',
    'tty_path': '$TTY_PATH',
    'metadata': {}
}))
")

if [ -S "$SOCKET" ]; then
    echo "$EVENT" | socat -t 0.1 - "UNIX-CONNECT:$SOCKET" &>/dev/null &
fi
```

**Step 5: Vytvoř git post-push hook**

```bash
# src/hooks/templates/git_post_push.sh
#!/usr/bin/env bash
# cwinner git hook: post-push

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"

EVENT=$(python3 -c "import json; print(json.dumps({'event':'GitPush','tool':None,'session_id':'git','tty_path':'$TTY_PATH','metadata':{}}))")

if [ -S "$SOCKET" ]; then
    echo "$EVENT" | socat -t 0.1 - "UNIX-CONNECT:$SOCKET" &>/dev/null &
fi
```

**Step 6: Ověř syntaxi hooků**

```bash
bash -n src/hooks/templates/post_tool_use.sh && \
bash -n src/hooks/templates/git_post_commit.sh && \
echo "hooks OK"
```

Expected: `hooks OK`

**Step 7: Commit**

```bash
git add src/hooks/
git commit -m "feat: shell hook templates for Claude Code and git"
```

---

## Task 10: Install modul

**Files:**
- Modify: `src/install.rs`

**Step 1: Napiš test pro settings.json merge**

```rust
// src/install.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_merge_claude_settings_empty() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(&settings_path, "{}").unwrap();

        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v["hooks"].is_object());
        assert!(v["hooks"]["PostToolUse"].is_array());
    }

    #[test]
    fn test_merge_claude_settings_existing_hooks() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        std::fs::write(&settings_path, r#"{"hooks":{"PostToolUse":[{"cmd":"existing"}]}}"#).unwrap();

        add_claude_hooks(&settings_path, "/usr/local/bin/cwinner").unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        // Existující hook zachován, cwinner přidán
        assert!(v["hooks"]["PostToolUse"].as_array().unwrap().len() >= 2);
    }
}
```

**Step 2: Spusť test — ověř fail**

```bash
cargo test install 2>&1 | tail -10
```

**Step 3: Implementuj**

```rust
// src/install.rs
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn install(binary_path: &Path) -> Result<()> {
    let binary_str = binary_path.to_str().unwrap_or("cwinner");

    // 1. Claude Code settings
    let claude_settings = dirs::home_dir()
        .context("no home dir")?
        .join(".claude")
        .join("settings.json");
    if claude_settings.exists() {
        add_claude_hooks(&claude_settings, binary_str)?;
        println!("✓ Claude Code hooks přidány do {}", claude_settings.display());
    } else {
        println!("⚠ ~/.claude/settings.json nenalezen — přidej hooks ručně");
    }

    // 2. Git global hooks
    let git_hooks_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("git")
        .join("hooks");
    std::fs::create_dir_all(&git_hooks_dir)?;
    install_git_hook(&git_hooks_dir.join("post-commit"), include_str!("hooks/templates/git_post_commit.sh"))?;
    install_git_hook(&git_hooks_dir.join("post-push"), include_str!("hooks/templates/git_post_push.sh"))?;
    println!("✓ Git hooks nainstalován do {}", git_hooks_dir.display());

    // 3. Default config
    let config_dir = dirs::config_dir()
        .context("no config dir")?
        .join("cwinner");
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, DEFAULT_CONFIG)?;
        println!("✓ Konfigurace vytvořena v {}", config_path.display());
    }

    // 4. State dir
    let state_dir = dirs::data_local_dir()
        .context("no data dir")?
        .join("cwinner");
    std::fs::create_dir_all(&state_dir)?;

    // 5. Systemd / launchd
    register_service(binary_str)?;

    println!("\n🎉 cwinner nainstalován! Spusť: cwinner status");
    Ok(())
}

pub fn add_claude_hooks(settings_path: &Path, binary: &str) -> Result<()> {
    let content = std::fs::read_to_string(settings_path).unwrap_or_else(|_| "{}".into());
    let mut v: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

    let hooks = v["hooks"].as_object_mut().get_or_insert_with(Default::default);

    // Pomocná funkce pro přidání hooku
    fn add_hook(arr: &mut serde_json::Value, cmd: serde_json::Value) {
        if let Some(a) = arr.as_array_mut() {
            // Nepřidávej duplicitní cwinner hook
            let already = a.iter().any(|h| {
                h["cmd"].as_str().map(|s| s.contains("cwinner")).unwrap_or(false)
            });
            if !already {
                a.push(cmd);
            }
        }
    }

    let post_tool = v["hooks"]["PostToolUse"].as_array().cloned().unwrap_or_default();
    let task_completed = v["hooks"]["TaskCompleted"].as_array().cloned().unwrap_or_default();
    let session_end = v["hooks"]["Stop"].as_array().cloned().unwrap_or_default();

    let mut pt = serde_json::Value::Array(post_tool);
    let mut tc = serde_json::Value::Array(task_completed);
    let mut se = serde_json::Value::Array(session_end);

    add_hook(&mut pt, serde_json::json!({"cmd": format!("{} hook post-tool-use", binary)}));
    add_hook(&mut tc, serde_json::json!({"cmd": format!("{} hook task-completed", binary)}));
    add_hook(&mut se, serde_json::json!({"cmd": format!("{} hook session-end", binary)}));

    v["hooks"]["PostToolUse"] = pt;
    v["hooks"]["TaskCompleted"] = tc;
    v["hooks"]["Stop"] = se;

    std::fs::write(settings_path, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

fn install_git_hook(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn register_service(binary: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        register_launchd(binary)?;
        println!("✓ launchd agent registrován");
    }
    #[cfg(target_os = "linux")]
    {
        register_systemd(binary)?;
        println!("✓ systemd user service registrován");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn register_systemd(binary: &str) -> Result<()> {
    let service_dir = dirs::home_dir().context("no home")?.join(".config/systemd/user");
    std::fs::create_dir_all(&service_dir)?;
    let unit = format!(r#"[Unit]
Description=cwinner celebration daemon
After=default.target

[Service]
ExecStart={binary} daemon
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
"#);
    std::fs::write(service_dir.join("cwinner.service"), unit)?;
    let _ = std::process::Command::new("systemctl").args(["--user", "daemon-reload"]).status();
    let _ = std::process::Command::new("systemctl").args(["--user", "enable", "--now", "cwinner"]).status();
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_launchd(binary: &str) -> Result<()> {
    let plist_dir = dirs::home_dir().context("no home")?.join("Library/LaunchAgents");
    std::fs::create_dir_all(&plist_dir)?;
    let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.cwinner.daemon</string>
  <key>ProgramArguments</key>
  <array><string>{binary}</string><string>daemon</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict>
</plist>"#);
    let plist_path = plist_dir.join("com.cwinner.daemon.plist");
    std::fs::write(&plist_path, plist)?;
    let _ = std::process::Command::new("launchctl").args(["load", plist_path.to_str().unwrap_or("")]).status();
    Ok(())
}

const DEFAULT_CONFIG: &str = r#"[intensity]
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

[streaks]
commit_streak_notify = [5, 10, 25, 50, 100]
"#;

pub fn uninstall() -> Result<()> {
    // Zastav service
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("systemctl").args(["--user", "stop", "cwinner"]).status();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("launchctl").args(["unload", "~/Library/LaunchAgents/com.cwinner.daemon.plist"]).status();

    println!("✓ cwinner odinstalován");
    Ok(())
}
```

**Step 4: Přidej include soubory do hook templates** — vytvoř prázdné soubory:

```bash
touch src/hooks/templates/git_post_commit.sh src/hooks/templates/git_post_push.sh
```

**Step 5: Spusť test — ověř pass**

```bash
cargo test install 2>&1 | tail -15
```

**Step 6: Commit**

```bash
git add src/install.rs src/hooks/
git commit -m "feat: install/uninstall with systemd+launchd service registration"
```

---

## Task 11: cwinner CLI (main.rs)

**Files:**
- Modify: `src/main.rs`
- Create: `src/daemon_main.rs`

**Step 1: Implementuj CLI**

```rust
// src/main.rs
use clap::{Parser, Subcommand};
use cwinner_lib::{install, state::State};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cwinner", about = "Gamification pro Claude Code vibe koders")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Nainstaluj cwinner (hooks, daemon, config)
    Install,
    /// Odinstaluj cwinner
    Uninstall,
    /// Zobraz stav daemonu a aktuální statistiky
    Status,
    /// Zobraz celkové statistiky a achievementy
    Stats,
    /// Interní: odešli event daemonovi (volají hook skripty)
    Hook {
        #[arg(value_enum)]
        event: HookEvent,
    },
    /// Spusť daemon přímo (bez service manageru)
    Daemon,
    /// Správa sound packů
    Sounds {
        #[command(subcommand)]
        cmd: SoundsCommands,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum HookEvent {
    PostToolUse,
    TaskCompleted,
    SessionEnd,
}

#[derive(Subcommand)]
enum SoundsCommands {
    /// Zobraz dostupné sound packy
    List,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Install => {
            let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cwinner"));
            if let Err(e) = install::install(&binary) {
                eprintln!("Chyba instalace: {e}");
                std::process::exit(1);
            }
        }
        Commands::Uninstall => {
            if let Err(e) = install::uninstall() {
                eprintln!("Chyba: {e}");
            }
        }
        Commands::Status => {
            let s = State::load();
            println!("cwinner status:");
            println!("  Level:  {} ({})", s.level, s.level_name);
            println!("  XP:     {}", s.xp);
            println!("  Streak: {} dní", s.commit_streak_days);
            println!("  Commity celkem: {}", s.commits_total);
        }
        Commands::Stats => {
            let s = State::load();
            println!("Statistiky:");
            println!("  XP: {} | Level: {} {}", s.xp, s.level, s.level_name);
            println!("  Commity: {} | Streak: {} dní", s.commits_total, s.commit_streak_days);
            println!("  Nástroje použity: {:?}", s.tools_used);
            println!("  Achievements: {}", s.achievements_unlocked.join(", "));
        }
        Commands::Hook { event } => {
            // Načti stdin a odešli event daemonovi
            let tty_path = get_tty();
            send_hook_event(event, &tty_path);
        }
        Commands::Daemon => {
            // Spusť daemon v popředí
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async {
                if let Err(e) = cwinner_lib::daemon::run().await {
                    eprintln!("Daemon error: {e}");
                }
            });
        }
        Commands::Sounds { cmd } => match cmd {
            SoundsCommands::List => {
                let sounds_dir = dirs::config_dir()
                    .unwrap_or_default()
                    .join("cwinner")
                    .join("sounds");
                if let Ok(entries) = std::fs::read_dir(&sounds_dir) {
                    for entry in entries.flatten() {
                        println!("  {}", entry.file_name().to_string_lossy());
                    }
                } else {
                    println!("Žádné sound packy nenalezeny v {}", sounds_dir.display());
                }
            }
        },
    }
}

fn get_tty() -> String {
    std::fs::read_link("/proc/self/fd/0")
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/dev/null".into())
}

fn send_hook_event(event: HookEvent, tty_path: &str) {
    use cwinner_lib::event::{Event, EventKind};
    use cwinner_lib::daemon::server::socket_path;
    use std::collections::HashMap;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    // Přečti stdin (Claude Code posílá JSON)
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    let meta: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();

    let event_kind = match event {
        HookEvent::PostToolUse => EventKind::PostToolUse,
        HookEvent::TaskCompleted => EventKind::TaskCompleted,
        HookEvent::SessionEnd => EventKind::SessionEnd,
    };

    let tool = meta.get("tool_name").and_then(|v| v.as_str()).map(String::from);
    let exit_code = meta.pointer("/tool_response/exit_code").and_then(|v| v.as_i64());
    let mut metadata = HashMap::new();
    if let Some(code) = exit_code {
        metadata.insert("exit_code".into(), serde_json::json!(code));
    }

    let e = Event {
        event: event_kind,
        tool,
        session_id: std::env::var("CLAUDE_SESSION_ID").unwrap_or_else(|_| "unknown".into()),
        tty_path: tty_path.to_string(),
        metadata,
    };

    let socket = socket_path();
    if let Ok(mut stream) = UnixStream::connect(&socket) {
        let json = serde_json::to_string(&e).unwrap_or_default();
        let _ = stream.write_all(format!("{}\n", json).as_bytes());
    }
}
```

**Step 2: Vytvoř `src/daemon_main.rs`**

```rust
// src/daemon_main.rs
fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        if let Err(e) = cwinner_lib::daemon::run().await {
            eprintln!("cwinnerd fatal error: {e}");
            std::process::exit(1);
        }
    });
}
```

**Step 3: Sestav projekt**

```bash
cargo build 2>&1 | tail -20
```

Expected: Úspěšná kompilace obou binárek.

**Step 4: Spusť všechny testy**

```bash
cargo test 2>&1 | tail -20
```

Expected: Všechny testy zelené.

**Step 5: Commit**

```bash
git add src/main.rs src/daemon_main.rs
git commit -m "feat: cwinner CLI with install/status/stats/hook/daemon commands"
```

---

## Task 12: Placeholder zvukové soubory + finální sestavení

**Files:**
- Create: `sounds/default/README.md`

**Step 1: Zdokumentuj sound pack**

```markdown
# sounds/default/

Výchozí sound pack pro cwinner.

## Požadované soubory

- `mini.ogg` — krátký tichý zvuk pro rutinní eventy
- `milestone.ogg` — uspokojivý zvuk pro milníky (commit, task)
- `epic.ogg` — výrazný zvuk pro průlomové momenty
- `fanfare.ogg` — fanfára pro epické oslavy
- `streak.ogg` — speciální zvuk pro streak milníky

## Zdroje zdarma

- https://freesound.org (licence CC0)
- https://opengameart.org

## Formáty

Podporovány: `.ogg`, `.wav`. Preferuj `.ogg` (menší soubory).

## Vlastní pack

Zkopíruj tento adresář do `~/.config/cwinner/sounds/<muj-pack>/`
a nastav v `config.toml`: `sound_pack = "muj-pack"`.
```

**Step 2: Release build**

```bash
cargo build --release 2>&1 | tail -10
```

Expected: `target/release/cwinner` a `target/release/cwinnerd` vytvořeny.

**Step 3: Ověř binárky**

```bash
./target/release/cwinner --help
./target/release/cwinner status
```

Expected: Help výstup a status bez pádu.

**Step 4: Finální test suite**

```bash
cargo test --all 2>&1
```

Expected: Všechny testy pass, žádné chyby.

**Step 5: Finální commit**

```bash
git add sounds/ Cargo.lock
git commit -m "feat: sound pack structure + release build verified"
```

---

## Shrnutí tasků

| # | Task | Klíčový výstup |
|---|---|---|
| 1 | Scaffold | Cargo.toml, binárky, moduly |
| 2 | Event typy | IPC JSON protokol |
| 3 | Config | TOML parsing s defaults |
| 4 | State engine | XP, levely, streaky, persistence |
| 5 | Celebration engine | Kontextová logika intenzity |
| 6 | TTY Renderer | Konfety, splash, progress bar |
| 7 | Audio engine | afplay/aplay/paplay fallback |
| 8 | Daemon server | Tokio Unix socket, event loop |
| 9 | Hook šablony | Shell skripty pro CC + git |
| 10 | Install modul | systemd/launchd, settings.json merge |
| 11 | CLI | clap rozhraní pro všechny příkazy |
| 12 | Sound pack + build | Release binárky, dokumentace |
