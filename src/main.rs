use clap::{Parser, Subcommand};
use cwinner_lib::{install, state::State};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cwinner", about = "Gamification pro Claude Code vibe koderů")]
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
    #[value(name = "post-tool-use")]
    PostToolUse,
    #[value(name = "task-completed")]
    TaskCompleted,
    #[value(name = "session-end")]
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
            let next_xp = cwinner_lib::renderer::xp_for_next_level(s.level);
            let level_idx = (s.level.saturating_sub(1)) as usize;
            let prev_threshold = cwinner_lib::renderer::LEVEL_THRESHOLDS
                .get(level_idx)
                .copied()
                .unwrap_or(0);
            let xp_in_level = s.xp.saturating_sub(prev_threshold);
            let xp_needed = next_xp.saturating_sub(prev_threshold);
            let bar = cwinner_lib::renderer::xp_bar_string(xp_in_level, xp_needed, 20);

            println!("Stats:");
            println!("  XP:      {} [{}] → {}", s.xp, bar, next_xp);
            println!("  Level:   {} — {}", s.level, s.level_name);
            println!("  Commits: {} │ Streak: {} days", s.commits_total, s.commit_streak_days);
            println!("  Tools used: {}", s.tools_used.len());
            println!();

            let unlocked = &s.achievements_unlocked;
            if unlocked.is_empty() {
                println!("Achievements: none yet");
            } else {
                println!("Achievements ({}/{}):", unlocked.len(), cwinner_lib::achievements::REGISTRY.len());
                for id in unlocked {
                    if let Some(a) = cwinner_lib::achievements::REGISTRY.iter().find(|a| a.id == id.as_str()) {
                        println!("  ✓ {} — {}", a.name, a.description);
                    } else {
                        println!("  ✓ {}", id);
                    }
                }
            }

            println!();
            let locked: Vec<_> = cwinner_lib::achievements::REGISTRY.iter()
                .filter(|a| !unlocked.iter().any(|id| id == a.id))
                .collect();
            if !locked.is_empty() {
                println!("Locked ({}):", locked.len());
                for a in locked {
                    println!("  ○ {} — {}", a.name, a.description);
                }
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
                    println!("Žádné sound packy v {}", sounds_dir.display());
                }
            }
        },
    }
}

fn get_tty() -> String {
    // Linux: /proc/self/fd/0 je symlink na TTY
    if let Ok(path) = std::fs::read_link("/proc/self/fd/0") {
        let s = path.to_string_lossy().to_string();
        if s.starts_with("/dev/") {
            return s;
        }
    }
    // macOS fallback: použij /dev/tty
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

    // Přečti stdin (Claude Code posílá JSON)
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
    let exit_code = meta
        .pointer("/tool_response/exit_code")
        .and_then(|v| v.as_i64());
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
    // Pokud daemon neběží, tiše selžeme
}
