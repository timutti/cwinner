//! Non-interactive demo for recording.
//! Run: cargo run --example record

use cwinner_lib::celebration::CelebrationLevel;
use cwinner_lib::renderer::{render, render_progress_bar};
use cwinner_lib::state::State;
use std::thread;
use std::time::Duration;

fn main() {
    let mut state = State::default();
    state.xp = 1325;
    state.level = 3;
    state.level_name = "Vibe Architect".into();
    state.commits_total = 12;
    state.commit_streak_days = 3;

    let tty = "/dev/tty".to_string();

    println!("\x1b[1;36m  cwinner — gamification for Claude Code\x1b[0m");
    println!();
    thread::sleep(Duration::from_millis(1500));

    println!("\x1b[33m  ▸ Mini — progress bar (bottom of screen)\x1b[0m");
    thread::sleep(Duration::from_millis(800));
    let _ = render_progress_bar(&tty, &state);
    thread::sleep(Duration::from_millis(1000));

    println!("\x1b[33m  ▸ Medium — task completed\x1b[0m");
    thread::sleep(Duration::from_millis(800));
    render(
        &tty,
        &CelebrationLevel::Medium,
        &state,
        None,
        Some("✓ Task Completed"),
    );
    thread::sleep(Duration::from_millis(1000));

    println!("\x1b[33m  ▸ Medium — achievement unlocked\x1b[0m");
    thread::sleep(Duration::from_millis(800));
    render(
        &tty,
        &CelebrationLevel::Medium,
        &state,
        Some("First Commit — Made your first git commit"),
        Some("📝 Git Commit"),
    );
    thread::sleep(Duration::from_millis(1000));

    println!("\x1b[33m  ▸ Epic — git push celebration\x1b[0m");
    thread::sleep(Duration::from_millis(800));
    render(
        &tty,
        &CelebrationLevel::Epic,
        &state,
        Some("Shipped It — First git push"),
        Some("🚀 Git Push"),
    );
    thread::sleep(Duration::from_millis(500));

    println!();
    println!("\x1b[1;32m  ✓ All celebrations complete!\x1b[0m");
    thread::sleep(Duration::from_millis(1500));
}
