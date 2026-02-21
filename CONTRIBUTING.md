# Contributing to cwinner

Thanks for your interest in contributing!

## Building from source

```bash
git clone https://github.com/timutti/cwinner.git
cd cwinner
cargo build --release
```

## Running tests

```bash
cargo test
cargo clippy
```

## Making changes

1. Fork the repo and create a feature branch from `master`
2. Make your changes
3. Add tests for new functionality
4. Run `cargo test` and `cargo clippy` — both must pass
5. Open a pull request against `master`

## Code style

- Follow standard Rust conventions (`rustfmt` defaults)
- Keep functions focused and small
- Use `anyhow` for error handling in application code
- Prefer returning `Result` over `unwrap()`/`expect()` in library code

## Architecture overview

- `src/main.rs` — CLI entry point (clap-based)
- `src/lib.rs` — library root
- `src/daemon/` — background daemon that receives events over a Unix socket
- `src/event.rs` — event types and celebration level logic
- `src/achievements.rs` — achievement definitions and checking
- `src/renderer.rs` — terminal UI (alternate screen, confetti, progress bars)
- `src/sounds.rs` — WAV synthesis and audio playback
- `src/install.rs` — install/uninstall logic (hooks, systemd/launchd, config)
- `src/state.rs` — persistent state (XP, level, streaks, achievements)

## Reporting issues

Use [GitHub Issues](https://github.com/timutti/cwinner/issues). Include:
- OS and architecture
- Rust version (`rustc --version`)
- Steps to reproduce
- Expected vs actual behavior
