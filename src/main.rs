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
            println!("Statistiky:");
            println!("  XP: {} | Level: {} {}", s.xp, s.level, s.level_name);
            println!("  Commity: {} | Streak: {} dní", s.commits_total, s.commit_streak_days);
            println!("  Nástroje použity: {}", s.tools_used.len());
            println!("  Achievements: {}", s.achievements_unlocked.join(", "));
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
