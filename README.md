# cwinner

Gamification pro [Claude Code](https://claude.ai/code). Sleduje tvůj postup, uděluje XP a přehrává zvuky při commitech, dokončených úkolech a průlomových momentech.

## Co dělá

- **XP a levely** — každá akce v Claude Code přidává body
- **Zvukové efekty** — WAV soubory generované za běhu, žádné externí závislosti
- **Commit streaky** — počítá po sobě jdoucí dny s commitem
- **Achievements** — odemykají se za milníky
- **Daemon** — běží na pozadí jako systemd/launchd service, přijímá eventy přes Unix socket

## Instalace

```bash
cargo build --release
./target/release/cwinner install
```

`install` udělá automaticky:
- přidá hooks do `~/.claude/settings.json`
- nainstaluje git hooks (`post-commit`, `post-push`)
- vygeneruje sound pack do `~/.config/cwinner/sounds/default/`
- zaregistruje systemd user service (Linux) nebo launchd agent (macOS)

## Příkazy

```
cwinner status        # aktuální level, XP, streak
cwinner stats         # podrobné statistiky a achievements
cwinner sounds list   # dostupné sound packy
cwinner install       # instalace
cwinner uninstall     # odinstalace
```

## Konfigurace

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

## Sound packy

Vlastní pack = adresář WAV/OGG/MP3 souborů v `~/.config/cwinner/sounds/<název>/`:

```
mini.wav        # rutinní akce
milestone.wav   # dokončený úkol, commit
epic.wav        # průlom (bash fail → pass)
fanfare.wav     # git push
streak.wav      # commit streak
```

## Architektura

```
cwinner hook <event>   →   Unix socket   →   cwinnerd daemon
                                               ├ celebrate()
                                               ├ update state
                                               └ play sound
```

Daemon (`cwinnerd`) běží trvale, hook skripty jsou odlehčené — jen pošlou JSON event na socket a skončí.

## Vývoj

```bash
cargo test
cargo build --release
```
