# cwinner

[![CI](https://github.com/timutti/cwinner/actions/workflows/ci.yml/badge.svg)](https://github.com/timutti/cwinner/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/timutti/cwinner)](https://github.com/timutti/cwinner/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macOS-lightgrey.svg)]()

Gamification for [Claude Code](https://claude.ai/code). Tracks your progress, awards XP, and plays sounds on commits, completed tasks, and breakthrough moments.

## Features

- **XP and 10 levels** — every action in Claude Code earns points (with 2x streak bonus at 5+ day streaks)
- **5 distinct sounds** — multi-note synthesized WAV melodies generated at runtime, no external assets
- **Visual celebrations** — progress bars, centered toasts, confetti rain + splash boxes (all via alternate screen)
- **26 achievements** — commits, streaks, tools, levels, and Claude Code features
- **Commit streaks** — tracks consecutive days, streak milestones at 5/10/25/100 days
- **Session tracking** — duration milestones at 1h/3h/8h, epic celebration for sessions with commits
- **Custom triggers** — config-based substring matching on bash commands
- **Daemon** — auto-starts in background, receives events over a Unix socket

## Install

### Quick install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/timutti/cwinner/master/install.sh | bash
```

The script downloads the latest release binary for your platform to `~/.local/bin/` and runs `cwinner install` automatically (hooks, daemon, config, sounds).

### From crates.io

```bash
cargo install cwinner
cwinner install
```

### From source

```bash
git clone https://github.com/timutti/cwinner.git
cd cwinner
cargo build --release
./target/release/cwinner install
```

`cwinner install` does everything automatically:
- adds hooks to `~/.claude/settings.json`
- sets up status line XP bar (wraps your existing statusline script)
- detects git commit/push from Claude Code hooks (no git hook installation needed)
- generates a default sound pack to `~/.config/cwinner/sounds/default/`
- daemon auto-starts from hooks (Linux) or registers a launchd agent (macOS)

## Commands

```
cwinner status        # current level, XP, streak
cwinner stats         # detailed stats and achievements
cwinner statusline    # XP progress for Claude Code status line
cwinner update        # self-update to latest release
cwinner sounds list   # available sound packs
cwinner install       # install
cwinner uninstall     # uninstall
```

## Configuration

`~/.config/cwinner/config.toml`:

```toml
[intensity]
routine = "mini"          # off | mini | medium | epic
task_completed = "medium" # separate from milestone to avoid toast spam during agent work
milestone = "medium"
breakthrough = "epic"

[audio]
enabled = true
sound_pack = "default"
volume = 0.8

[visual]
confetti = true
splash_screen = true
progress_bar = true
confetti_duration_ms = 1500
splash_duration_ms = 2000

# Custom triggers -- celebrate specific bash commands (substring match)
# [[triggers.custom]]
# name = "deploy"
# pattern = "git push"
# intensity = "epic"
```

## Sound packs

The default pack ships 5 synthesized multi-note WAV melodies (generated at install time, no external assets). A custom pack is a directory of WAV/OGG/MP3 files under `~/.config/cwinner/sounds/<name>/`:

```
mini.wav        # quick double-tap (reserved, not currently played)
milestone.wav   # rising chime — Medium celebration without achievement
epic.wav        # C major chord swell — Medium celebration with achievement
fanfare.wav     # ascending trumpet call — Epic celebration
streak.wav      # rapid ascending scale — Epic + streak milestone
```

Mini celebrations are silent (visual only). If a sound file is missing from the configured pack, cwinner falls back to generating a WAV into `/tmp/cwinner/`.

## Architecture

```
cwinner hook <event>   →   Unix socket   →   cwinnerd daemon
                                               ├ decide celebration level
                                               ├ detect git commit/push from Bash commands
                                               ├ check achievements
                                               ├ update XP/state
                                               ├ play sound (async)
                                               └ render visual (alternate screen)
```

The daemon auto-starts from hook events as a detached background process (inherits the session's audio context for reliable sound playback). Claude Code hooks use the `cwinner hook` CLI subcommand. Git commit and push are detected directly from Bash command strings — no git hooks needed. All hooks are fire-and-forget.

## Development

```bash
cargo test
cargo clippy
cargo build --release
```

## License

MIT
