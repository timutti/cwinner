# CLAUDE.md

## Project overview

cwinner is a Rust gamification overlay for Claude Code. It tracks progress, awards XP, plays sounds, and renders visual celebrations (progress bars, toasts, confetti) via Claude Code hooks. A background daemon (`cwinnerd`) receives events over a Unix socket.

## Build & test

```bash
cargo build --release
cargo test
cargo clippy
cargo fmt --all -- --check
```

CI runs fmt check, clippy, and tests on every push.

## Demo GIF generation

The demo GIF in `assets/demo.gif` is recorded from `examples/record.rs` (non-interactive, plays all 4 celebration types in sequence).

### Steps

1. **Pre-build** the example so cargo output doesn't appear in the recording:
   ```bash
   cargo build --release --example record
   ```

2. **Record** with asciinema using the pre-built binary directly:
   ```bash
   asciinema rec -c "./target/release/examples/record" --cols 80 --rows 24 /tmp/cwinner-demo.cast
   ```

3. **Convert** to GIF with agg (asciinema GIF generator), using Dracula theme:
   ```bash
   agg --cols 80 --rows 24 --speed 1.0 --theme dracula /tmp/cwinner-demo.cast assets/demo.gif
   ```

Available themes: `asciinema`, `dracula`, `github-dark`, `github-light`, `monokai`, `nord`, `solarized-dark`, `solarized-light`, `gruvbox-dark`.

### Important notes

- Always run the pre-built binary (`./target/release/examples/record`), NOT `cargo run --example record`, to avoid "Compiling/Finished" messages in the GIF.
- The GIF must match real daemon behavior — if renderer code changes, rebuild the example AND reinstall the daemon binary before re-recording.
- Emoji characters (🚀, 🏆, ⚡) are 2 display columns wide but 1 char in Rust. The `center_padded()` helper in `renderer.rs` accounts for this when drawing box borders.

## Key architecture notes

- Claude Code `PostToolUse` hook does NOT fire for failed Bash commands — only successful tool uses trigger hooks.
- The `render()` function signature: `render(tty_path, level, state, achievement, label)` where `achievement` is a newly unlocked achievement name and `label` is the event context (e.g. "Git Push").
- Epic splash shows both label and achievement on separate lines when both are present.
- Mini celebrations are visual-only (no alternate screen). The `mini.wav` sound is used exclusively for level-up events.
- Sound mapping: Mini=level-up, Milestone=medium-no-achievement, Epic=medium-with-achievement, Fanfare=epic, Streak=epic+streak-milestone.
