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
