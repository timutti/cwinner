use crate::celebration::CelebrationLevel;
use crate::state::State;
use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::Rng;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

pub const LEVEL_THRESHOLDS: &[u32] = &[0, 100, 500, 1500, 5000, u32::MAX];
const CONFETTI_CHARS: &[char] = &['✦', '★', '♦', '●', '*', '+', '#', '✿', '❋'];
const CONFETTI_COLORS: &[Color] = &[
    Color::Red, Color::Green, Color::Yellow, Color::Blue,
    Color::Magenta, Color::Cyan, Color::White,
];

pub fn xp_bar_string(current_xp: u32, next_xp: u32, width: usize) -> String {
    let ratio = if next_xp == 0 { 1.0 } else { current_xp as f64 / next_xp as f64 };
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let mut s = String::new();
    for _ in 0..filled { s.push('█'); }
    for _ in filled..width { s.push('░'); }
    s
}

pub fn xp_for_next_level(level: u32) -> u32 {
    LEVEL_THRESHOLDS.get(level as usize).copied().unwrap_or(u32::MAX)
}

pub fn render(tty_path: &str, level: &CelebrationLevel, state: &State, achievement: Option<&str>) {
    match level {
        CelebrationLevel::Off => {}
        CelebrationLevel::Mini => { let _ = render_progress_bar(tty_path, state); }
        CelebrationLevel::Medium => { let _ = render_progress_bar(tty_path, state); }
        CelebrationLevel::Epic => {
            let _ = render_confetti(tty_path);
            let _ = render_splash(tty_path, state, achievement.unwrap_or("ACHIEVEMENT UNLOCKED!"));
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

pub fn render_progress_bar(tty_path: &str, state: &State) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let (_, rows) = tty_size(&tty);
    let bottom = rows.saturating_sub(1);
    let level_idx = (state.level.saturating_sub(1)) as usize;
    let prev_threshold = LEVEL_THRESHOLDS.get(level_idx).copied().unwrap_or(0);
    let next_xp = xp_for_next_level(state.level);
    let xp_in_level = state.xp.saturating_sub(prev_threshold);
    let xp_needed = next_xp.saturating_sub(prev_threshold);
    let bar = xp_bar_string(xp_in_level, xp_needed, 20);

    queue!(tty,
        cursor::SavePosition,
        cursor::MoveTo(0, bottom),
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
        cursor::MoveTo(0, bottom),
        Clear(ClearType::CurrentLine),
        cursor::RestorePosition,
    )?;
    tty.flush()
}

pub fn render_confetti(tty_path: &str) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let mut rng = rand::thread_rng();
    let (cols, rows) = tty_size(&tty);
    let frames = 15u64;
    let frame_ms = 1500 / frames;

    execute!(tty, EnterAlternateScreen, cursor::Hide)?;

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

    Ok(())
}

pub fn render_splash(tty_path: &str, state: &State, achievement: &str) -> io::Result<()> {
    let mut tty = open_tty(tty_path)?;
    let (cols, rows) = tty_size(&tty);
    let mid_row = rows / 2;

    // Clear alternate screen for splash (confetti already entered alternate screen)
    execute!(tty, Clear(ClearType::All), cursor::Hide)?;

    let inner_width = (cols as usize).saturating_sub(2);
    let border = "═".repeat(inner_width);
    let top = format!("╔{}╗", border);
    let bot = format!("╚{}╝", border);

    queue!(tty,
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
        Print(format!("║{:^width$}║",
            format!("Lvl {} {} ✦ {} XP", state.level, state.level_name, state.xp),
            width = inner_width)),
        cursor::MoveTo(0, mid_row + 1),
        SetForegroundColor(Color::Yellow),
        Print(format!("║{:^width$}║", "", width = inner_width)),
        cursor::MoveTo(0, mid_row + 2),
        Print(&bot),
        ResetColor,
    )?;
    tty.flush()?;
    thread::sleep(Duration::from_millis(2000));
    // Leave alternate screen — original terminal content is restored
    execute!(tty, cursor::Show, LeaveAlternateScreen, ResetColor)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(xp_for_next_level(1), 100);
        assert_eq!(xp_for_next_level(2), 500);
        assert_eq!(xp_for_next_level(3), 1500);
    }
}
