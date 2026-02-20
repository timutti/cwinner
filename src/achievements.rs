use crate::event::{Event, EventKind};
use crate::state::State;

pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub static REGISTRY: &[Achievement] = &[
    // Commits (4)
    Achievement { id: "first_commit",  name: "First Commit",       description: "Made your first git commit" },
    Achievement { id: "commit_10",     name: "Getting Committed",   description: "10 commits total" },
    Achievement { id: "commit_50",     name: "Commit Machine",      description: "50 commits total" },
    Achievement { id: "commit_100",    name: "Centurion",           description: "100 commits total" },
    // Streaks (3)
    Achievement { id: "streak_5",      name: "On a Roll",           description: "5-day commit streak" },
    Achievement { id: "streak_10",     name: "Unstoppable",         description: "10-day commit streak" },
    Achievement { id: "streak_25",     name: "Dedicated",           description: "25-day commit streak" },
    // Push (1)
    Achievement { id: "first_push",    name: "Shipped It",          description: "First git push" },
    // Breakthrough (1)
    Achievement { id: "test_whisperer",name: "Test Whisperer",      description: "Fixed a failing bash command" },
    // Tools (2)
    Achievement { id: "tool_explorer", name: "Tool Explorer",       description: "Used 5 different tools" },
    Achievement { id: "tool_master",   name: "Tool Master",         description: "Used 10 different tools" },
    // Levels (4)
    Achievement { id: "level_2",       name: "Prompt Whisperer",    description: "Reached level 2" },
    Achievement { id: "level_3",       name: "Vibe Architect",      description: "Reached level 3" },
    Achievement { id: "level_4",       name: "Flow State Master",   description: "Reached level 4" },
    Achievement { id: "level_5",       name: "Claude Sensei",       description: "Reached level 5" },
    // Claude Code basics (4)
    Achievement { id: "first_subagent",     name: "Delegator",       description: "Spawned a subagent with Task tool" },
    Achievement { id: "web_surfer",         name: "Web Surfer",      description: "Used WebSearch" },
    Achievement { id: "researcher",         name: "Deep Researcher", description: "Used WebFetch" },
    Achievement { id: "mcp_pioneer",        name: "MCP Pioneer",     description: "Used an MCP tool" },
    // Claude Code advanced (5)
    Achievement { id: "notebook_scientist", name: "Data Scientist",  description: "Used NotebookEdit" },
    Achievement { id: "todo_master",        name: "Organized",       description: "Used TodoWrite" },
    Achievement { id: "first_skill",        name: "Skilled Up",      description: "Invoked a skill or slash command" },
    Achievement { id: "first_team",         name: "Team Player",     description: "Created an agent team" },
    Achievement { id: "team_communicator",  name: "Team Lead",       description: "Sent a message to a teammate" },
];

/// Returns achievements newly unlocked by this event (not already in state.achievements_unlocked).
pub fn check_achievements<'a>(state: &State, event: &Event) -> Vec<&'a Achievement> {
    REGISTRY.iter()
        .filter(|a| !state.achievements_unlocked.contains(&a.id.to_string()))
        .filter(|a| is_unlocked(a, state, event))
        .collect()
}

fn is_unlocked(a: &Achievement, state: &State, event: &Event) -> bool {
    let tool = event.tool.as_deref().unwrap_or("");
    match a.id {
        "first_commit"  => state.commits_total >= 1,
        "commit_10"     => state.commits_total >= 10,
        "commit_50"     => state.commits_total >= 50,
        "commit_100"    => state.commits_total >= 100,
        "streak_5"      => state.commit_streak_days >= 5,
        "streak_10"     => state.commit_streak_days >= 10,
        "streak_25"     => state.commit_streak_days >= 25,
        "first_push"    => event.event == EventKind::GitPush,
        "test_whisperer" => {
            event.event == EventKind::PostToolUse
                && tool == "Bash"
                && state.last_bash_exit.map(|c| c == 0).unwrap_or(false)
        }
        "tool_explorer" => state.tools_used.len() >= 5,
        "tool_master"   => state.tools_used.len() >= 10,
        "level_2" => state.level >= 2,
        "level_3" => state.level >= 3,
        "level_4" => state.level >= 4,
        "level_5" => state.level >= 5,
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
