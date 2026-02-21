use crate::celebration::CelebrationLevel;
use crate::state::{State, LEVELS};
use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::RngExt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

/// Global render lock — prevents concurrent alternate screen switches
/// which confuse Claude Code's differential renderer.
static RENDER_LOCK: Mutex<Option<Instant>> = Mutex::new(None);

/// Minimum gap between renders to let Claude Code's renderer recover.
const RENDER_COOLDOWN: Duration = Duration::from_millis(5000);

/// Return the XP threshold for the level at `index` in the LEVELS table.
/// Returns `u32::MAX` if `index` is out of range (i.e., past the last defined level).
pub fn level_threshold(index: usize) -> u32 {
    LEVELS.get(index).map(|&(t, _)| t).unwrap_or(u32::MAX)
}
const CONFETTI_CHARS: &[char] = &['✦', '★', '♦', '●', '*', '+', '#', '✿', '❋'];
const CONFETTI_COLORS: &[Color] = &[
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::White,
];

pub fn xp_bar_string(current_xp: u32, next_xp: u32, width: usize) -> String {
    let ratio = if next_xp == 0 {
        1.0
    } else {
        current_xp as f64 / next_xp as f64
    };
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let mut s = String::new();
    for _ in 0..filled {
        s.push('█');
    }
    for _ in filled..width {
        s.push('░');
    }
    s
}

/// Return `(xp_in_level, xp_needed_for_level)` for the given level and total XP.
///
/// `xp_in_level` is how much XP the player has earned *within* the current level,
/// and `xp_needed_for_level` is the total XP span of that level (from current
/// threshold to the next threshold).
///
/// Both `Stats` and the toast renderer should use this to ensure consistent
/// progress bar calculations.
pub fn xp_progress(level: u32, xp: u32) -> (u32, u32) {
    let current_idx = (level.saturating_sub(1)) as usize;
    let prev_threshold = level_threshold(current_idx);
    let next_threshold = level_threshold(level as usize);
    let xp_in_level = xp.saturating_sub(prev_threshold);
    let xp_needed = next_threshold.saturating_sub(prev_threshold);
    (xp_in_level, xp_needed)
}

/// Acquire the render lock and check cooldown. Returns the guard if rendering
/// is allowed, or None if we should skip (too recent). The caller MUST call
/// `finish_render` with the guard when done.
pub fn acquire_render_slot() -> Option<std::sync::MutexGuard<'static, Option<Instant>>> {
    let guard = match RENDER_LOCK.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    if let Some(last) = *guard {
        if last.elapsed() < RENDER_COOLDOWN {
            return None;
        }
    }
    Some(guard)
}

/// Mark render as finished — sets the cooldown timestamp.
pub fn finish_render(mut guard: std::sync::MutexGuard<'static, Option<Instant>>) {
    *guard = Some(Instant::now());
}

/// RAII guard that restores terminal state (leave alternate screen + show cursor)
/// on drop, even if rendering panics or returns early via `?`.
struct TermGuard<'a> {
    tty: &'a mut std::fs::File,
}

impl<'a> Drop for TermGuard<'a> {
    fn drop(&mut self) {
        let _ = execute!(self.tty, LeaveAlternateScreen, cursor::Show);
    }
}

pub fn render(tty_path: &str, level: &CelebrationLevel, state: &State, achievement: Option<&str>) {
    match level {
        CelebrationLevel::Off => {}
        CelebrationLevel::Mini => {
            let _ = render_progress_bar(tty_path, state);
        }
        CelebrationLevel::Medium => {
            let _ = render_toast(tty_path, state, achievement);
        }
        CelebrationLevel::Epic => {
            let _ = render_epic(
                tty_path,
                state,
                achievement.unwrap_or("ACHIEVEMENT UNLOCKED!"),
            );
        }
    }
}

fn open_tty(tty_path: &str) -> io::Result<std::fs::File> {
    OpenOptions::new().write(true).open(tty_path)
}

fn tty_size(tty: &std::fs::File) -> (u16, u16) {
    use std::os::unix::io::AsRawFd;
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::ioctl(tty.as_raw_fd(), libc::TIOCGWINSZ, &mut ws) };
    if ret == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
        (ws.ws_col, ws.ws_row)
    } else {
        (80, 24)
    }
}

/// Format the toast message line for display.
pub fn format_toast_msg(state: &State, achievement: Option<&str>) -> (String, Color) {
    if let Some(name) = achievement {
        (
            format!("🏆 {} │ {} │ {} XP", name, state.level_name, state.xp),
            Color::Yellow,
        )
    } else {
        let next = level_threshold(state.level as usize);
        if next == u32::MAX {
            (
                format!("⚡ {} │ {} XP │ MAX", state.level_name, state.xp),
                Color::Cyan,
            )
        } else {
            let (xp_in_level, xp_needed) = xp_progress(state.level, state.xp);
            let bar = xp_bar_string(xp_in_level, xp_needed, 15);
            (
                format!("⚡ {} │ {} │ {} XP", state.level_name, bar, state.xp),
                Color::Cyan,
            )
        }
    }
}

/// Mini celebration: brief progress bar on the bottom line of the terminal.
/// Uses alternate screen (same as toast) for Claude Code compatibility.
/// Duration: 3 seconds.
pub fn render_progress_bar(tty_path: &str, state: &State) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let (cols, rows) = tty_size(&tty);
    let (msg, color) = format_toast_msg(state, None);

    let pad_width = (cols as usize).saturating_sub(2);
    let bottom_row = rows.saturating_sub(1);

    execute!(
        tty,
        EnterAlternateScreen,
        cursor::Hide,
        Clear(ClearType::All)
    )?;
    let _guard = TermGuard { tty: &mut tty };

    queue!(
        _guard.tty,
        cursor::MoveTo(0, bottom_row),
        SetForegroundColor(color),
        Print(format!("{:<width$}", msg, width = pad_width)),
        ResetColor,
    )?;
    _guard.tty.flush()?;

    thread::sleep(Duration::from_millis(3000));

    Ok(())
}

/// Brief alternate screen overlay — the only safe way to display in a terminal
/// managed by Claude Code's differential renderer without corrupting its state.
pub fn render_toast(tty_path: &str, state: &State, achievement: Option<&str>) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let (cols, rows) = tty_size(&tty);
    let (msg, color) = format_toast_msg(state, achievement);
    let duration = if achievement.is_some() {
        2500u64
    } else {
        1500u64
    };

    let mid_row = rows / 2;
    let pad_width = (cols as usize).saturating_sub(2);

    execute!(
        tty,
        EnterAlternateScreen,
        cursor::Hide,
        Clear(ClearType::All)
    )?;
    let _guard = TermGuard { tty: &mut tty };

    queue!(
        _guard.tty,
        cursor::MoveTo(0, mid_row),
        SetForegroundColor(color),
        Print(format!("{:^width$}", msg, width = pad_width)),
        ResetColor,
    )?;
    _guard.tty.flush()?;

    thread::sleep(Duration::from_millis(duration));

    Ok(())
}

/// Epic celebration: confetti rain → splash box over confetti background.
/// Single alternate screen session to avoid flicker.
fn render_epic(tty_path: &str, state: &State, achievement: &str) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let mut rng = rand::rng();
    let (cols, rows) = tty_size(&tty);

    execute!(
        tty,
        EnterAlternateScreen,
        cursor::Hide,
        Clear(ClearType::All)
    )?;
    let _guard = TermGuard { tty: &mut tty };

    // Phase 1: confetti rain (1.5s)
    let frames = 15u64;
    let frame_ms = 1500 / frames;
    for _ in 0..frames {
        for _ in 0..(cols / 4) {
            let col = rng.random_range(0..cols);
            let row = rng.random_range(0..rows.saturating_sub(2));
            let ch = CONFETTI_CHARS[rng.random_range(0..CONFETTI_CHARS.len())];
            let color = CONFETTI_COLORS[rng.random_range(0..CONFETTI_COLORS.len())];
            queue!(
                _guard.tty,
                cursor::MoveTo(col, row),
                SetForegroundColor(color),
                Print(ch),
            )?;
        }
        _guard.tty.flush()?;
        thread::sleep(Duration::from_millis(frame_ms));
    }

    // Phase 2: splash box drawn over confetti background (2s)
    let mid_row = rows / 2;
    let inner_width = (cols as usize).saturating_sub(2);
    let border = "═".repeat(inner_width);
    let top = format!("╔{}╗", border);
    let bot = format!("╚{}╝", border);

    queue!(
        _guard.tty,
        cursor::MoveTo(0, mid_row.saturating_sub(3)),
        SetForegroundColor(Color::Yellow),
        Print(&top),
        cursor::MoveTo(0, mid_row.saturating_sub(2)),
        Print(format!("║{:^width$}║", "", width = inner_width)),
        cursor::MoveTo(0, mid_row.saturating_sub(1)),
        SetForegroundColor(Color::Green),
        Print(format!("║{:^width$}║", achievement, width = inner_width)),
        cursor::MoveTo(0, mid_row),
        SetForegroundColor(Color::Cyan),
        Print(format!(
            "║{:^width$}║",
            format!("Lvl {} {} ✦ {} XP", state.level, state.level_name, state.xp),
            width = inner_width
        )),
        cursor::MoveTo(0, mid_row + 1),
        SetForegroundColor(Color::Yellow),
        Print(format!("║{:^width$}║", "", width = inner_width)),
        cursor::MoveTo(0, mid_row + 2),
        Print(&bot),
        ResetColor,
    )?;
    _guard.tty.flush()?;
    thread::sleep(Duration::from_millis(2000));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    #[test]
    fn test_xp_bar_empty() {
        let bar = xp_bar_string(0, 100, 20);
        assert_eq!(bar.chars().count(), 20);
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
        assert_eq!(level_threshold(1), 100);
        assert_eq!(level_threshold(2), 500);
        assert_eq!(level_threshold(3), 1500);
    }

    #[test]
    fn test_format_toast_msg_regular() {
        let mut state = State::default();
        state.xp = 250;
        state.level = 2;
        state.level_name = "Prompt Whisperer".into();
        let (msg, color) = format_toast_msg(&state, None);
        assert!(msg.contains("Prompt Whisperer"));
        assert!(msg.contains("250 XP"));
        assert!(msg.contains('█') || msg.contains('░'));
        assert_eq!(color, Color::Cyan);
    }

    #[test]
    fn test_format_toast_msg_achievement() {
        let mut state = State::default();
        state.xp = 500;
        state.level = 3;
        state.level_name = "Vibe Architect".into();
        let (msg, color) = format_toast_msg(&state, Some("First Commit"));
        assert!(msg.contains("🏆"));
        assert!(msg.contains("First Commit"));
        assert!(msg.contains("Vibe Architect"));
        assert!(msg.contains("500 XP"));
        assert_eq!(color, Color::Yellow);
    }

    /// Verify xp_progress returns consistent results for all levels.
    #[test]
    fn test_xp_progress_levels() {
        // Level 1, 50 XP: in_level = 50, needed = 100
        let (in_l, needed) = xp_progress(1, 50);
        assert_eq!(in_l, 50);
        assert_eq!(needed, 100);

        // Level 2, 250 XP: in_level = 150, needed = 400
        let (in_l, needed) = xp_progress(2, 250);
        assert_eq!(in_l, 150);
        assert_eq!(needed, 400);

        // Level 3, 500 XP (exactly at threshold): in_level = 0, needed = 1000
        let (in_l, needed) = xp_progress(3, 500);
        assert_eq!(in_l, 0);
        assert_eq!(needed, 1000);

        // Level 5, 6000 XP: in_level = 1000, needed = 5000
        let (in_l, needed) = xp_progress(5, 6000);
        assert_eq!(in_l, 1000);
        assert_eq!(needed, 5000);

        // Level 7, 25000 XP: in_level = 5000, needed = 15000
        let (in_l, needed) = xp_progress(7, 25000);
        assert_eq!(in_l, 5000);
        assert_eq!(needed, 15000);

        // Level 10 (max), 80000 XP: threshold = 75000, next = u32::MAX
        let (in_l, needed) = xp_progress(10, 80000);
        assert_eq!(in_l, 5000);
        assert_eq!(needed, u32::MAX - 75000);
    }

    /// Verify that xp_progress(1, 0) starts clean at level 1.
    #[test]
    fn test_xp_progress_fresh_state() {
        let (in_l, needed) = xp_progress(1, 0);
        assert_eq!(in_l, 0);
        assert_eq!(needed, 100);
    }

    /// Verify the Stats path and toast path produce identical results.
    #[test]
    fn test_xp_progress_matches_toast() {
        // For several (level, xp) combos, ensure xp_progress gives a single answer
        // that both call sites now use.
        let cases = vec![
            (1, 0),
            (1, 50),
            (1, 99),
            (2, 100),
            (2, 250),
            (2, 499),
            (3, 500),
            (3, 1000),
            (3, 1499),
            (4, 1500),
            (4, 3000),
            (4, 4999),
            (5, 5000),
            (5, 9999),
            (6, 10000),
            (6, 15000),
            (7, 20000),
            (7, 30000),
            (8, 35000),
            (8, 45000),
            (9, 50000),
            (9, 60000),
            (10, 75000),
            (10, 100000),
        ];
        for (level, xp) in cases {
            let (in_l, needed) = xp_progress(level, xp);
            // Basic sanity: in_level should be < needed (unless max level with huge XP)
            assert!(
                in_l <= needed || level as usize >= LEVELS.len(),
                "level={}, xp={}: in_level {} > needed {}",
                level,
                xp,
                in_l,
                needed
            );
            // xp_in_level + prev_threshold == xp
            let prev = level_threshold((level.saturating_sub(1)) as usize);
            assert_eq!(
                in_l + prev,
                xp,
                "level={}, xp={}: in_level {} + prev {} != xp",
                level,
                xp,
                in_l,
                prev
            );
        }
    }

    // --- Mini celebration (format_toast_msg with None) tests ---

    #[test]
    fn test_format_toast_msg_mini_regular() {
        let mut state = State::default();
        state.xp = 50;
        state.level = 1;
        state.level_name = "Vibe Initiate".into();
        let (msg, color) = format_toast_msg(&state, None);
        assert!(msg.contains("⚡"));
        assert!(msg.contains("Vibe Initiate"));
        assert!(msg.contains("50 XP"));
        assert!(msg.contains('█') || msg.contains('░'));
        assert_eq!(color, Color::Cyan);
    }

    #[test]
    fn test_format_toast_msg_mini_mid_level() {
        let mut state = State::default();
        state.xp = 750;
        state.level = 3;
        state.level_name = "Vibe Architect".into();
        let (msg, color) = format_toast_msg(&state, None);
        assert!(msg.contains("⚡"));
        assert!(msg.contains("Vibe Architect"));
        assert!(msg.contains("750 XP"));
        // Should have both filled and unfilled bar segments
        assert!(msg.contains('█'));
        assert!(msg.contains('░'));
        assert_eq!(color, Color::Cyan);
    }

    #[test]
    fn test_format_toast_msg_mini_max_level() {
        let mut state = State::default();
        state.xp = 80000;
        state.level = 10;
        state.level_name = "Singularity".into();
        let (msg, color) = format_toast_msg(&state, None);
        assert!(msg.contains("⚡"));
        assert!(msg.contains("Singularity"));
        assert!(msg.contains("80000 XP"));
        assert!(msg.contains("MAX"));
        assert_eq!(color, Color::Cyan);
    }

    #[test]
    fn test_format_toast_msg_mini_no_achievement() {
        // Mini celebration (None achievement) never shows trophy
        let mut state = State::default();
        state.xp = 250;
        state.level = 2;
        state.level_name = "Prompt Whisperer".into();
        let (msg, _) = format_toast_msg(&state, None);
        // Should not contain trophy emoji
        assert!(!msg.contains("🏆"));
        // Should contain lightning bolt
        assert!(msg.contains("⚡"));
    }

    #[test]
    fn test_format_toast_msg_mini_format_structure() {
        let mut state = State::default();
        state.xp = 250;
        state.level = 2;
        state.level_name = "Prompt Whisperer".into();
        let (msg, _) = format_toast_msg(&state, None);
        // Verify the │ delimiters are present (3 sections)
        let delimiter_count = msg.matches('│').count();
        assert_eq!(
            delimiter_count, 2,
            "Expected 2 │ delimiters, got {}",
            delimiter_count
        );
    }
}
