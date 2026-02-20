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
        let (level, achievement) = {
            let mut s = state.lock().unwrap();
            let level = decide(&event, &s, &cfg);
            let xp = xp_for_level(&level);
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
                if let Some(code) = event
                    .metadata
                    .get("exit_code")
                    .and_then(|v| v.as_i64())
                {
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

pub fn process_event_with_state(
    event: &Event,
    state: &mut State,
    cfg: &Config,
    render_visual: bool,
) {
    let level = decide(event, state, cfg);
    let xp = xp_for_level(&level);
    if xp > 0 {
        state.add_xp(xp);
    }
    if event.event == EventKind::GitCommit {
        state.record_commit();
    }
    if let Some(tool) = &event.tool {
        state.record_tool_use(tool);
    }
    if render_visual && level != CelebrationLevel::Off {
        render(&event.tty_path, &level, state, None);
    }
}

fn handle_command(cmd: &DaemonCommand, state: &Arc<Mutex<State>>) -> DaemonResponse {
    let s = state.lock().unwrap();
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
