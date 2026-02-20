# cwinner

Gamification for [Claude Code](https://claude.ai/code). Tracks your progress, awards XP, and plays sounds on commits, completed tasks, and breakthrough moments.

## Features

- **XP and levels** — every action in Claude Code earns points
- **Sound effects** — WAV files generated at runtime, no external assets or dependencies
- **Commit streaks** — counts consecutive days with a commit
- **Achievements** — unlocked at milestones
- **Daemon** — runs in the background as a systemd/launchd service, receives events over a Unix socket

## Install

```bash
cargo build --release
./target/release/cwinner install
```

`install` does everything automatically:
- adds hooks to `~/.claude/settings.json`
- installs git hooks (`post-commit`, `post-push`)
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
routine = "off"       # off | mini | medium | epic
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
```

## Sound packs

A custom pack is a directory of WAV/OGG/MP3 files under `~/.config/cwinner/sounds/<name>/`:

```
mini.wav        # routine action
milestone.wav   # completed task, commit
epic.wav        # breakthrough (bash fail → pass)
fanfare.wav     # git push
streak.wav      # commit streak
```

## Architecture

```
cwinner hook <event>   →   Unix socket   →   cwinnerd daemon
                                               ├ celebrate()
                                               ├ update state
                                               └ play sound
```

The daemon (`cwinnerd`) runs persistently. Hook scripts are lightweight — they just send a JSON event to the socket and exit.

## Development

```bash
cargo test
cargo build --release
```
