# cwinner

Gamification for [Claude Code](https://claude.ai/code). Tracks your progress, awards XP, and plays sounds on commits, completed tasks, and breakthrough moments.

## Features

- **XP and 10 levels** — every action in Claude Code earns points (with 2x streak bonus at 5+ day streaks)
- **5 distinct sounds** — multi-note synthesized WAV melodies generated at runtime, no external assets
- **Visual celebrations** — progress bars, centered toasts, confetti rain + splash boxes (all via alternate screen)
- **26 achievements** — commits, streaks, tools, levels, Claude Code features
- **Commit streaks** — tracks consecutive days, streak milestones at 5/10/25/100 days
- **Session tracking** — duration milestones at 1h/3h/8h, epic celebration for sessions with commits
- **Custom triggers** — config-based substring matching on bash commands
- **Daemon** — runs in the background as a systemd/launchd service, receives events over a Unix socket

## Install

```bash
cargo build --release
./target/release/cwinner install
```

`install` does everything automatically:
- adds hooks to `~/.claude/settings.json`
- installs git hooks (`post-commit`, `pre-push`)
- generates a default sound pack to `~/.config/cwinner/sounds/default/`
- registers a systemd user service (Linux) or launchd agent (macOS)

## Commands

```
cwinner status        # current level, XP, streak
cwinner stats         # detailed stats and achievements
cwinner sounds list   # available sound packs
cwinner install       # install
cwinner uninstall     # uninstall
```

## Configuration

`~/.config/cwinner/config.toml`:

```toml
[intensity]
routine = "off"           # off | mini | medium | epic
task_completed = "off"    # separate from milestone to avoid toast spam during agent work
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
mini.wav        # quick double-tap notification (Mini celebration)
milestone.wav   # rising chime (Medium, no achievement)
epic.wav        # C major chord swell (Medium with achievement)
fanfare.wav     # ascending trumpet call (Epic)
streak.wav      # rapid ascending scale (Epic + streak milestone)
```

If a sound file is missing from the configured pack, cwinner falls back to generating a WAV into `/tmp/cwinner/`.

## Architecture

```
cwinner hook <event>   →   Unix socket   →   cwinnerd daemon
git post-commit hook   →                      ├ decide celebration level
                                               ├ check achievements
                                               ├ update XP/state
                                               ├ play sound (async)
                                               └ render visual (alternate screen)
```

The daemon (`cwinnerd`) runs persistently as a systemd user service (Linux) or launchd agent (macOS). Claude Code hooks use the `cwinner hook` Rust CLI subcommand. Git hooks are bash scripts that use `socat` or `nc` to send events. All hooks are fire-and-forget.

## Development

```bash
cargo test
cargo build --release
```
