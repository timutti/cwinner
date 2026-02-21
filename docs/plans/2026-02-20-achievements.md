# Achievements System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a 24-achievement registry that fires from the daemon on every event, displays unlock splash screens, plays the streak sound, and awards 2× XP streak bonuses.

**Architecture:** New `src/achievements.rs` module holds a static registry of 24 `Achievement` structs. After every state mutation in `daemon/server.rs`, `check_achievements(&state, &event)` returns newly unlocked achievements. The first unlocked achievement's name replaces the generic event-name in the splash screen. Streak bonus (2× XP when `commit_streak_days >= 5`) is applied in `celebration.rs`.

**Tech Stack:** Rust, existing `State`/`Event`/`CelebrationLevel` types — zero new dependencies.

---

## Task 1: Create `src/achievements.rs` with registry and checker

**Files:**
- Create: `src/achievements.rs`
- Modify: `src/lib.rs` — add `pub mod achievements;`

**Step 1: Write failing tests**

Add to `src/achievements.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;
    use crate::event::{Event, EventKind};
    use std::collections::HashMap;

    fn ev(kind: EventKind, tool: Option<&str>) -> Event {
        Event {
            event: kind,
            tool: tool.map(String::from),
            session_id: "s".into(),
            tty_path: "/dev/null".into(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_registry_has_24_achievements() {
        assert_eq!(REGISTRY.len(), 24);
    }

    #[test]
    fn test_first_commit_unlocks_on_first_commit() {
        let mut s = State::default();
        s.commits_total = 1;
        let unlocked = check_achievements(&s, &ev(EventKind::GitCommit, None));
        assert!(unlocked.iter().any(|a| a.id == "first_commit"));
    }

    #[test]
    fn test_streak_5_unlocks_at_5_days() {
        let mut s = State::default();
        s.commit_streak_days = 5;
        let unlocked = check_achievements(&s, &ev(EventKind::GitCommit, None));
        assert!(unlocked.iter().any(|a| a.id == "streak_5"));
    }

    #[test]
    fn test_no_duplicate_unlocks() {
        let mut s = State::default();
        s.commits_total = 1;
        s.achievements_unlocked = vec!["first_commit".into()];
        let unlocked = check_achievements(&s, &ev(EventKind::GitCommit, None));
        assert!(!unlocked.iter().any(|a| a.id == "first_commit"));
    }

    #[test]
    fn test_first_subagent_unlocks_on_task_tool() {
        let mut s = State::default();
        s.tools_used.insert("Task".into());
        let unlocked = check_achievements(&s, &ev(EventKind::PostToolUse, Some("Task")));
        assert!(unlocked.iter().any(|a| a.id == "first_subagent"));
    }

    #[test]
    fn test_mcp_pioneer_unlocks_on_mcp_tool() {
        let mut s = State::default();
        s.tools_used.insert("mcp__github__search".into());
        let unlocked = check_achievements(&s, &ev(EventKind::PostToolUse, Some("mcp__github__search")));
        assert!(unlocked.iter().any(|a| a.id == "mcp_pioneer"));
    }

    #[test]
    fn test_level_2_unlocks_at_level_2() {
        let mut s = State::default();
        s.level = 2;
        let unlocked = check_achievements(&s, &ev(EventKind::TaskCompleted, None));
        assert!(unlocked.iter().any(|a| a.id == "level_2"));
    }

    #[test]
    fn test_tool_explorer_at_5_tools() {
        let mut s = State::default();
        for t in ["Bash", "Read", "Write", "Glob", "Task"] {
            s.tools_used.insert(t.into());
        }
        let unlocked = check_achievements(&s, &ev(EventKind::PostToolUse, Some("Task")));
        assert!(unlocked.iter().any(|a| a.id == "tool_explorer"));
    }

    #[test]
    fn test_achievement_has_name_and_description() {
        let a = REGISTRY.iter().find(|a| a.id == "first_commit").unwrap();
        assert!(!a.name.is_empty());
        assert!(!a.description.is_empty());
    }
}
```

**Step 2: Run tests to see them fail**

```bash
cargo test achievements -- --nocapture 2>&1 | head -20
```

Expected: compile error (module doesn't exist yet)

**Step 3: Implement `src/achievements.rs`**

```rust
use crate::event::{Event, EventKind};
use crate::state::State;

pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub static REGISTRY: &[Achievement] = &[
    // Commits
    Achievement { id: "first_commit",  name: "First Commit",        description: "Made your first git commit" },
    Achievement { id: "commit_10",     name: "Getting Committed",    description: "10 commits total" },
    Achievement { id: "commit_50",     name: "Commit Machine",       description: "50 commits total" },
    Achievement { id: "commit_100",    name: "Centurion",            description: "100 commits total" },
    // Streaks
    Achievement { id: "streak_5",      name: "On a Roll",            description: "5-day commit streak" },
    Achievement { id: "streak_10",     name: "Unstoppable",          description: "10-day commit streak" },
    Achievement { id: "streak_25",     name: "Dedicated",            description: "25-day commit streak" },
    // Push
    Achievement { id: "first_push",    name: "Shipped It",           description: "First git push" },
    // Breakthrough
    Achievement { id: "test_whisperer",name: "Test Whisperer",       description: "Fixed a failing bash command" },
    // Tools
    Achievement { id: "tool_explorer", name: "Tool Explorer",        description: "Used 5 different tools" },
    Achievement { id: "tool_master",   name: "Tool Master",          description: "Used 10 different tools" },
    // Levels
    Achievement { id: "level_2",       name: "Prompt Whisperer",     description: "Reached level 2" },
    Achievement { id: "level_3",       name: "Vibe Architect",       description: "Reached level 3" },
    Achievement { id: "level_4",       name: "Flow State Master",    description: "Reached level 4" },
    Achievement { id: "level_5",       name: "Claude Sensei",        description: "Reached level 5" },
    // Claude Code basics
    Achievement { id: "first_subagent",name: "Delegator",            description: "Spawned a subagent with Task tool" },
    Achievement { id: "web_surfer",    name: "Web Surfer",           description: "Used WebSearch" },
    Achievement { id: "researcher",    name: "Deep Researcher",      description: "Used WebFetch" },
    Achievement { id: "mcp_pioneer",   name: "MCP Pioneer",          description: "Used an MCP tool" },
    // Claude Code advanced
    Achievement { id: "notebook_scientist", name: "Data Scientist",  description: "Used NotebookEdit" },
    Achievement { id: "todo_master",   name: "Organized",            description: "Used TodoWrite" },
    Achievement { id: "first_skill",   name: "Skilled Up",           description: "Invoked a skill or slash command" },
    Achievement { id: "first_team",    name: "Team Player",          description: "Created an agent team" },
    Achievement { id: "team_communicator", name: "Team Lead",        description: "Sent a message to a teammate" },
];

/// Returns achievements newly unlocked by this event (not already in state).
pub fn check_achievements<'a>(state: &State, event: &Event) -> Vec<&'a Achievement> {
    REGISTRY.iter()
        .filter(|a| !state.achievements_unlocked.contains(&a.id.to_string()))
        .filter(|a| is_unlocked(a, state, event))
        .collect()
}

fn is_unlocked(a: &Achievement, state: &State, event: &Event) -> bool {
    let tool = event.tool.as_deref().unwrap_or("");
    match a.id {
        // Commit milestones
        "first_commit"  => state.commits_total >= 1,
        "commit_10"     => state.commits_total >= 10,
        "commit_50"     => state.commits_total >= 50,
        "commit_100"    => state.commits_total >= 100,
        // Streaks
        "streak_5"      => state.commit_streak_days >= 5,
        "streak_10"     => state.commit_streak_days >= 10,
        "streak_25"     => state.commit_streak_days >= 25,
        // Push
        "first_push"    => event.event == EventKind::GitPush,
        // Breakthrough
        "test_whisperer" => {
            event.event == EventKind::PostToolUse
                && tool == "Bash"
                && state.last_bash_exit.map(|c| c == 0).unwrap_or(false)
                // prev_failed is stored before this event in server.rs, checked via last_bash_exit
        }
        // Tool counts
        "tool_explorer" => state.tools_used.len() >= 5,
        "tool_master"   => state.tools_used.len() >= 10,
        // Levels
        "level_2" => state.level >= 2,
        "level_3" => state.level >= 3,
        "level_4" => state.level >= 4,
        "level_5" => state.level >= 5,
        // Claude Code tools
        "first_subagent"      => state.tools_used.contains("Task"),
        "web_surfer"          => state.tools_used.contains("WebSearch"),
        "researcher"          => state.tools_used.contains("WebFetch"),
        "mcp_pioneer"         => state.tools_used.iter().any(|t| t.starts_with("mcp__")),
        "notebook_scientist"  => state.tools_used.contains("NotebookEdit"),
        "todo_master"         => state.tools_used.contains("TodoWrite"),
        "first_skill"         => state.tools_used.contains("Skill"),
        "first_team"          => state.tools_used.contains("TeamCreate"),
        "team_communicator"   => state.tools_used.contains("SendMessage"),
        _                     => false,
    }
}
```

**Step 4: Add module to `src/lib.rs`**

Add after `pub mod sounds;`:
```rust
pub mod achievements;
```

**Step 5: Run tests**

```bash
cargo test achievements -- --nocapture
```

Expected: all 8 tests pass

**Step 6: Commit**

```bash
git add src/achievements.rs src/lib.rs
git commit -m "feat: add achievement registry with 24 achievements"
```

---

## Task 2: Add streak XP bonus to `src/celebration.rs`

**Files:**
- Modify: `src/celebration.rs`

**Step 1: Write failing test**

Add to `src/celebration.rs` tests:

```rust
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
```

**Step 2: Run to see fail**

```bash
cargo test celebration -- --nocapture 2>&1 | grep "FAILED\|error"
```

Expected: compile error — `xp_for_event` doesn't exist yet

**Step 3: Add `xp_for_event` to `src/celebration.rs`**

Add after `xp_for_level`:

```rust
/// XP with 2× streak bonus when commit_streak_days >= 5
pub fn xp_for_event(level: &CelebrationLevel, state: &State) -> u32 {
    let base = xp_for_level(level);
    if base > 0 && state.commit_streak_days >= 5 {
        base * 2
    } else {
        base
    }
}
```

Also add `use crate::state::State;` to imports if not already present.

**Step 4: Run tests**

```bash
cargo test celebration -- --nocapture
```

Expected: all tests pass

**Step 5: Commit**

```bash
git add src/celebration.rs
git commit -m "feat: add 2x streak XP bonus when commit streak >= 5 days"
```

---

## Task 3: Wire achievements into `src/daemon/server.rs`

**Files:**
- Modify: `src/daemon/server.rs`

This is the core wiring task. After state mutations, we call `check_achievements`, unlock them in state, and pass the first achievement name to the renderer instead of the generic event debug string.

**Step 1: Write failing test**

Add to `src/daemon/server.rs` tests:

```rust
#[test]
fn test_first_commit_achievement_fires() {
    let mut state = crate::state::State::default();
    let cfg = crate::config::Config::default();
    let mut event = make_event(EventKind::GitCommit);

    process_event_with_state(&event, &mut state, &cfg, false);

    assert!(state.achievements_unlocked.contains(&"first_commit".to_string()));
}

#[test]
fn test_level_up_achievement_fires() {
    let mut state = crate::state::State::default();
    state.xp = 95; // just below level 2 (100 XP)
    let cfg = crate::config::Config::default();
    let event = make_event(EventKind::TaskCompleted); // adds 25 XP → level 2

    process_event_with_state(&event, &mut state, &cfg, false);

    assert!(state.achievements_unlocked.contains(&"level_2".to_string()));
}

#[test]
fn test_streak_bonus_applied_in_process_event() {
    let mut state = crate::state::State::default();
    state.commit_streak_days = 5;
    let cfg = crate::config::Config::default();
    let event = make_event(EventKind::TaskCompleted);

    process_event_with_state(&event, &mut state, &cfg, false);

    // 25 XP * 2 streak bonus = 50 XP
    assert_eq!(state.xp, 50);
}
```

**Step 2: Run to see fail**

```bash
cargo test server -- --nocapture 2>&1 | grep "FAILED\|error\|ok"
```

**Step 3: Modify `src/daemon/server.rs`**

Add import at top:
```rust
use crate::achievements::check_achievements;
use crate::celebration::xp_for_event;
```

Replace the event processing block (lines 74-99) in `handle_connection`:

```rust
if let Ok(event) = serde_json::from_str::<Event>(line) {
    let tty_path = event.tty_path.clone();
    let (level, celebration_name) = {
        let mut s = state.lock().unwrap();
        let level = decide(&event, &s, &cfg);
        let xp = xp_for_event(&level, &s);
        if xp > 0 {
            s.add_xp(xp);
        }
        if event.event == EventKind::GitCommit {
            s.record_commit();
        }
        if let Some(tool) = &event.tool {
            s.record_tool_use(tool);
        }
        if event.event == EventKind::PostToolUse {
            if let Some(code) = event.metadata.get("exit_code").and_then(|v| v.as_i64()) {
                s.last_bash_exit = Some(code as i32);
            }
        }
        // Check and unlock achievements
        let newly_unlocked = check_achievements(&s, &event);
        let celebration_name = newly_unlocked
            .first()
            .map(|a| a.name.to_string())
            .unwrap_or_else(|| format!("{:?}", event.event));
        for a in &newly_unlocked {
            s.unlock_achievement(a.id);
        }
        s.save();
        (level, celebration_name)
    };

    if level != CelebrationLevel::Off {
        let cfg2 = Arc::clone(&cfg);
        tokio::task::spawn_blocking(move || {
            if cfg2.audio.enabled {
                if let Some(sound) = celebration_to_sound(&level) {
                    play_sound(&sound, &cfg2.audio.sound_pack);
                }
            }
            render(&tty_path, &level, &State::load(), Some(&celebration_name));
        });
    }
}
```

Also update `process_event_with_state` to use `xp_for_event` and check achievements:

```rust
pub fn process_event_with_state(
    event: &Event,
    state: &mut State,
    cfg: &Config,
    render_visual: bool,
) {
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
    if event.event == EventKind::PostToolUse {
        if let Some(code) = event.metadata.get("exit_code").and_then(|v| v.as_i64()) {
            state.last_bash_exit = Some(code as i32);
        }
    }
    let newly_unlocked = check_achievements(state, event);
    for a in &newly_unlocked {
        state.unlock_achievement(a.id);
    }
    if render_visual && level != CelebrationLevel::Off {
        let name = newly_unlocked.first().map(|a| a.name.to_string());
        render(&event.tty_path, &level, state, name.as_deref());
    }
}
```

**Step 4: Run tests**

```bash
cargo test server -- --nocapture
```

Expected: all server tests pass (including the 3 new ones)

**Step 5: Run full test suite**

```bash
cargo test
```

Expected: all tests pass

**Step 6: Commit**

```bash
git add src/daemon/server.rs
git commit -m "feat: wire achievements and streak bonus into daemon event processing"
```

---

## Task 4: Display achievements in `cwinner stats`

**Files:**
- Modify: `src/main.rs`

**Step 1: Update stats command**

Find the `Commands::Stats` handler and replace with richer output:

```rust
Commands::Stats => {
    let s = State::load();
    let next_xp = cwinner_lib::renderer::xp_for_next_level(s.level);
    let prev_xp = cwinner_lib::renderer::xp_for_next_level(s.level.saturating_sub(1));
    let xp_in_level = s.xp.saturating_sub(prev_xp);
    let xp_needed = next_xp.saturating_sub(prev_xp);
    let bar = cwinner_lib::renderer::xp_bar_string(xp_in_level, xp_needed, 20);

    println!("Stats:");
    println!("  XP:     {} / {} │ {}", s.xp, next_xp, bar);
    println!("  Level:  {} {}", s.level, s.level_name);
    println!("  Commits: {} │ Streak: {} days", s.commits_total, s.commit_streak_days);
    println!("  Tools used: {}", s.tools_used.len());
    println!();

    if s.achievements_unlocked.is_empty() {
        println!("Achievements: none yet");
    } else {
        println!("Achievements ({}):", s.achievements_unlocked.len());
        for id in &s.achievements_unlocked {
            if let Some(a) = cwinner_lib::achievements::REGISTRY.iter().find(|a| a.id == *id) {
                println!("  ✓ {} — {}", a.name, a.description);
            } else {
                println!("  ✓ {}", id);
            }
        }
    }

    println!();
    let locked: Vec<_> = cwinner_lib::achievements::REGISTRY.iter()
        .filter(|a| !s.achievements_unlocked.contains(&a.id.to_string()))
        .collect();
    println!("Locked ({}):", locked.len());
    for a in locked {
        println!("  ○ {} — {}", a.name, a.description);
    }
},
```

**Step 2: Build and verify**

```bash
cargo build --release 2>&1 | tail -3
./target/release/cwinner stats
```

Expected: clean output with unlocked + locked achievements

**Step 3: Run full test suite**

```bash
cargo test
```

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: show unlocked/locked achievements in cwinner stats"
```

---

## Task 5: Verification, install, push

**Step 1: Full test suite**

```bash
cargo test 2>&1 | grep "test result"
```

Expected: all pass, 0 failed

**Step 2: Build release**

```bash
cargo build --release
```

**Step 3: Simulate achievements via live events**

```bash
# Send a task-completed event (should unlock level achievements if XP crosses threshold)
echo '{}' | ./target/release/cwinner hook task-completed
sleep 1
./target/release/cwinner stats
```

**Step 4: Install locally**

```bash
./target/release/cwinner install
systemctl --user restart cwinner
systemctl --user status cwinner | grep Active
```

**Step 5: Push to GitHub**

```bash
git push
```
