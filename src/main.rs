use clap::{Parser, Subcommand};
use cwinner_lib::{install, state::State};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cwinner", about = "Gamification overlay for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install cwinner (hooks, daemon, config)
    Install,
    /// Uninstall cwinner
    Uninstall,
    /// Show daemon status and current statistics
    Status,
    /// Show overall statistics and achievements
    Stats,
    /// Internal: send event to daemon (called by hook scripts)
    Hook {
        #[arg(value_enum)]
        event: HookEvent,
    },
    /// Output XP progress for Claude Code status line
    Statusline,
    /// Update cwinner to the latest release
    Update,
    /// Run daemon directly (without service manager)
    Daemon,
    /// Manage sound packs
    Sounds {
        #[command(subcommand)]
        cmd: SoundsCommands,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum HookEvent {
    #[value(name = "post-tool-use")]
    PostToolUse,
    #[value(name = "task-completed")]
    TaskCompleted,
    #[value(name = "session-end")]
    SessionEnd,
}

#[derive(Subcommand)]
enum SoundsCommands {
    /// List available sound packs
    List,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Install => {
            let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cwinner"));
            if let Err(e) = install::install(&binary) {
                eprintln!("Install error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Uninstall => {
            if let Err(e) = install::uninstall() {
                eprintln!("Uninstall error: {e}");
            }
        }
        Commands::Status => {
            let s = State::load();
            println!("cwinner status:");
            println!("  Level:  {} ({})", s.level, s.level_name);
            println!("  XP:     {}", s.xp);
            println!("  Streak: {} days", s.commit_streak_days);
            println!("  Total commits: {}", s.commits_total);
        }
        Commands::Stats => {
            let s = State::load();
            let (xp_in_level, xp_needed) = cwinner_lib::renderer::xp_progress(s.level, s.xp);
            let next_xp = cwinner_lib::renderer::level_threshold(s.level as usize);
            let bar = cwinner_lib::renderer::xp_bar_string(xp_in_level, xp_needed, 20);

            println!("Stats:");
            if next_xp == u32::MAX {
                println!("  XP:      {} [{}] MAX", s.xp, bar);
            } else {
                println!("  XP:      {} [{}] → {}", s.xp, bar, next_xp);
            }
            println!("  Level:   {} — {}", s.level, s.level_name);
            println!(
                "  Commits: {} │ Streak: {} days",
                s.commits_total, s.commit_streak_days
            );
            println!("  Tools used: {}", s.tools_used.len());
            println!();

            let unlocked = &s.achievements_unlocked;
            // Build HashSet once for O(1) lookups
            let unlocked_set: std::collections::HashSet<&str> =
                unlocked.iter().map(|s| s.as_str()).collect();

            if unlocked.is_empty() {
                println!("Achievements: none yet");
            } else {
                println!(
                    "Achievements ({}/{}):",
                    unlocked.len(),
                    cwinner_lib::achievements::REGISTRY.len()
                );
                for id in unlocked {
                    if let Some(a) = cwinner_lib::achievements::REGISTRY
                        .iter()
                        .find(|a| a.id == id.as_str())
                    {
                        println!("  ✓ {} — {}", a.name, a.description);
                    } else {
                        println!("  ✓ {}", id);
                    }
                }
            }

            println!();
            let locked: Vec<_> = cwinner_lib::achievements::REGISTRY
                .iter()
                .filter(|a| !unlocked_set.contains(a.id))
                .collect();
            if !locked.is_empty() {
                println!("Locked ({}):", locked.len());
                for a in locked {
                    println!("  ○ {} — {}", a.name, a.description);
                }
            }
        }
        Commands::Statusline => {
            let s = State::load();
            let (xp_in_level, xp_needed) = cwinner_lib::renderer::xp_progress(s.level, s.xp);
            let next_xp = cwinner_lib::renderer::level_threshold(s.level as usize);
            let bar = cwinner_lib::renderer::xp_bar_string(xp_in_level, xp_needed, 8);
            if next_xp == u32::MAX {
                print!("⚡ {} [{}] {} XP MAX", s.level_name, bar, s.xp);
            } else {
                print!("⚡ {} [{}] {} XP", s.level_name, bar, s.xp);
            }
        }
        Commands::Update => {
            let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cwinner"));
            if let Err(e) = cwinner_lib::update::update(&binary) {
                eprintln!("Update error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Hook { event } => {
            let tty_path = get_tty();
            send_hook_event(event, &tty_path);
        }
        Commands::Daemon => {
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
                    println!("No sound packs in {}", sounds_dir.display());
                }
            }
        },
    }
}

fn get_tty() -> String {
    #[cfg(target_os = "linux")]
    {
        // Walk up the process tree looking for an ancestor with a /dev/pts/* fd.
        // Claude Code hooks have redirected fds, so we must climb to find the terminal.
        let mut pid = std::process::id().to_string();
        for _ in 0..10 {
            for fd in [0, 1, 2] {
                if let Ok(path) = std::fs::read_link(format!("/proc/{}/fd/{}", pid, fd)) {
                    let s = path.to_string_lossy().to_string();
                    if s.starts_with("/dev/pts/") {
                        return s;
                    }
                }
            }
            // Move to parent process
            let stat_path = format!("/proc/{}/stat", pid);
            if let Ok(stat) = std::fs::read_to_string(&stat_path) {
                if let Some(ppid) = stat
                    .split(") ")
                    .last()
                    .and_then(|s| s.split_whitespace().nth(1))
                {
                    if ppid == "0" || ppid == "1" || ppid == pid {
                        break;
                    }
                    pid = ppid.to_string();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Try ttyname() on inherited file descriptors
        for fd in [0i32, 1, 2] {
            let name = unsafe { libc::ttyname(fd) };
            if !name.is_null() {
                let s = unsafe { std::ffi::CStr::from_ptr(name) }
                    .to_string_lossy()
                    .to_string();
                if s.starts_with("/dev/ttys") || s.starts_with("/dev/tty") {
                    return s;
                }
            }
        }

        // Claude Code hooks redirect all fds, so ttyname() fails.
        // Walk up the process tree via `ps` to find an ancestor with a real TTY
        // (same strategy as the Linux /proc walk above).
        let mut pid = std::process::id().to_string();
        for _ in 0..10 {
            if let Ok(output) = std::process::Command::new("ps")
                .args(["-o", "ppid=,tty=", "-p", &pid])
                .output()
            {
                let line = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    let ppid = parts[0];
                    let tty = parts[1];
                    if tty != "??" && !tty.is_empty() {
                        let dev_path = format!("/dev/{tty}");
                        if std::path::Path::new(&dev_path).exists() {
                            return dev_path;
                        }
                    }
                    if ppid == "0" || ppid == "1" || ppid == pid {
                        break;
                    }
                    pid = ppid.to_string();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    // Universal fallback
    if std::path::Path::new("/dev/tty").exists() {
        return "/dev/tty".into();
    }
    "/dev/null".into()
}

fn send_hook_event(event: HookEvent, tty_path: &str) {
    use cwinner_lib::daemon::server::socket_path;
    use cwinner_lib::event::{Event, EventKind};
    use std::collections::HashMap;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    // Read stdin (Claude Code sends JSON)
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    let meta: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();

    let event_kind = match event {
        HookEvent::PostToolUse => EventKind::PostToolUse,
        HookEvent::TaskCompleted => EventKind::TaskCompleted,
        HookEvent::SessionEnd => EventKind::SessionEnd,
    };

    let tool = meta
        .get("tool_name")
        .and_then(|v| v.as_str())
        .map(String::from);
    // Claude Code sends PostToolUse only on success (failures go to PostToolUseFailure),
    // and doesn't include exit_code in tool_response. Default to 0 for PostToolUse.
    let exit_code = meta
        .pointer("/tool_response/exit_code")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let mut metadata = HashMap::new();
    metadata.insert("exit_code".into(), serde_json::json!(exit_code));
    // Pass bash command text for custom trigger matching
    if let Some(input) = meta.get("tool_input") {
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            metadata.insert("command".into(), serde_json::json!(cmd));
        }
    }

    let e = Event {
        event: event_kind,
        tool,
        session_id: std::env::var("CLAUDE_SESSION_ID").unwrap_or_else(|_| "unknown".into()),
        tty_path: tty_path.to_string(),
        metadata,
    };

    let socket = socket_path();

    // Try connecting; auto-start daemon if not running
    let mut stream = match UnixStream::connect(&socket) {
        Ok(s) => s,
        Err(_) => {
            if !try_start_daemon(&socket) {
                return;
            }
            match UnixStream::connect(&socket) {
                Ok(s) => s,
                Err(_) => return,
            }
        }
    };

    let json = serde_json::to_string(&e).unwrap_or_default();
    let _ = stream.write_all(format!("{}\n", json).as_bytes());
}

/// Start the daemon as a detached background process so it inherits the
/// current session's audio context (PipeWire/PulseAudio).  Systemd user
/// services run in an isolated cgroup that cannot reach the audio server
/// on many setups, so session-spawned is the reliable default.
fn try_start_daemon(socket: &std::path::Path) -> bool {
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};

    // Remove stale socket if present
    if socket.exists() {
        let _ = std::fs::remove_file(socket);
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cwinner"));
    let res = unsafe {
        Command::new(&exe)
            .arg("daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .pre_exec(|| {
                // New session so daemon survives hook exit
                libc::setsid();
                Ok(())
            })
            .spawn()
    };

    if res.is_err() {
        return false;
    }

    // Wait up to 1s for daemon to be ready
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if UnixStream::connect(socket).is_ok() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tty_returns_non_empty() {
        let tty = get_tty();
        assert!(
            !tty.is_empty(),
            "get_tty() should return a non-empty string"
        );
    }

    #[test]
    fn test_get_tty_returns_valid_path() {
        let tty = get_tty();
        // Should always return a path starting with /dev/
        assert!(
            tty.starts_with("/dev/"),
            "get_tty() should return a /dev/ path, got: {tty}"
        );
    }
}
