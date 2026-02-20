# cwinner — Design Document

**Datum:** 2026-02-20
**Status:** Schváleno
**Jazyk:** Rust
**Platformy:** Linux, macOS

---

## Přehled

cwinner je gamifikační aplikace která oslavuje úspěchy při používání Claude Code a vibe coding workflow. Skládá se z perzistentního Rust daemonu (`cwinnerd`) a tenkých hook skriptů. Daemon agreguje eventy ze všech běžících Claude Code instancí na stroji, vyhodnocuje kontext a spouští kontextově přiměřené oslavy — zvuky, ASCII konfety, splash screeny a progress bar — přímo v terminálu kde event vznikl.

---

## Architektura

```
┌─────────────────────────────────────────────────────────┐
│                    cwinnerd (Rust daemon)                │
│                                                         │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ State Engine│  │ Celebration  │  │ TTY Renderer  │  │
│  │ XP, streaks │  │ Engine       │  │ konfety,      │  │
│  │ per session │  │ kontextové   │  │ splash,       │  │
│  │ + globálně  │  │ rozhodování  │  │ progress bar  │  │
│  └─────────────┘  └──────────────┘  └───────────────┘  │
│                                                         │
│  ┌─────────────┐  ┌──────────────┐                      │
│  │ Audio Engine│  │ Config       │                      │
│  │afplay/aplay │  │ TOML soubor  │                      │
│  └─────────────┘  └──────────────┘                      │
│                                                         │
│  Unix socket: ~/.local/share/cwinner/cwinner.sock       │
└──────────────────────────┬──────────────────────────────┘
                           │ IPC (JSON over Unix socket)
         ┌─────────────────┼──────────────────────┐
         │                 │                      │
   ┌─────┴──────┐   ┌──────┴─────┐   ┌────────────┴───┐
   │ CC hook #1 │   │ CC hook #2 │   │  git hook      │
   │ tty: pts/3 │   │ tty: pts/7 │   │  post-commit   │
   └────────────┘   └────────────┘   └────────────────┘
```

### Komponenty

**cwinnerd** — hlavní Rust daemon, běží jako systemd user service (Linux) nebo launchd agent (macOS). Naslouchá na Unix socketu. Drží veškerý stav v paměti, persistuje do `~/.local/share/cwinner/state.json`.

**Hook skripty** — tenké shell skripty (bash), instalované do `~/.claude/settings.json` a `~/.config/git/hooks/`. Každý hook:
1. Serializuje event do JSON
2. Přidá `tty_path` (`/dev/pts/N` nebo `/dev/ttysN`)
3. Odešle přes Unix socket daemonovi
4. Okamžitě skončí (neblokuje Claude Code)

**cwinner CLI** — uživatelský příkaz pro instalaci, konfiguraci, zobrazení statistik a správu sound packů.

---

## IPC Protokol

JSON zprávy přes Unix socket. Každá zpráva má:

```json
{
  "event": "PostToolUse",
  "tool": "Bash",
  "session_id": "abc123",
  "tty_path": "/dev/pts/3",
  "metadata": {
    "exit_code": 0,
    "command": "cargo test"
  }
}
```

Daemon odpovídá synchronně jen pro `/status` a `/stats` příkazy. Eventy jsou fire-and-forget.

---

## Triggery

| Trigger | Zdroj | Výchozí intenzita |
|---|---|---|
| `PostToolUse: Write` (file edit) | CC hook | mini (volitelně off) |
| `PostToolUse: Bash` + exit 0 po předchozím failu | CC hook | střední |
| `PostToolUse: Bash` — test suite pass | CC hook | střední |
| `TaskCompleted` | CC hook | střední |
| `git commit` | git post-commit hook | střední |
| `git push` | git post-push hook | velká |
| `SessionEnd` s ≥1 commitem | CC hook | velká |
| Git commit streak (5 / 10 / 25 / 100) | daemon | epická |
| První použití nového nástroje | daemon | střední |
| Session délka milestone (1h / 3h / 8h) | daemon | střední → epická |
| Uživatelem definovaný trigger | config | nastavitelné |

Daemon detekuje "průlom" pokud byl předchozí stav chybový (test fail → test pass, bash non-zero → zero).

---

## Celebration Engine

Kontextová logika rozhodující o intenzitě oslavy:

```
event přijde
  ├── průlom? (po selhání, první krát, streak milestone)
  │     → EPICKÁ: splash screen (2s) + konfety (1.5s) + fanfára
  ├── milník? (commit, task done, test pass, push)
  │     → STŘEDNÍ: progress bar blikne + krátký zvuk
  └── rutina? (file write, bash call)
        → MINI: tichý zvuk (default OFF)
```

Uživatel může přebít intenzitu v konfiguraci nebo přidat vlastní pravidla.

---

## TTY Renderer

Daemon přijme `tty_path`, otevře descriptor, zapíše ANSI sekvence, zavře. Renderer nikdy neblokuje déle než dobu animace a vždy obnoví původní stav terminálu (cursor, scroll region).

### Konfety
- Znaky: `✦ ★ ♦ ● * + # ✿ ❋`
- Barvy: 16 ANSI barev, náhodně
- Trvání: 1.5 s, padají shora v náhodných sloupcích
- Cleanup: cursor home + erase lines po skončení

### Splash Screen
- Trvání: 2 s
- Celá obrazovka, vycentrovaný ASCII název achievementu
- Barevný rámeček, název levelu, získané XP
- Cleanup: full screen erase, cursor restore

### Progress Bar
- Pozice: spodní řádek terminálu (alternate screen buffer)
- Zobrazuje: aktuální XP, level, název posledního achievementu
- Zmizí po: 3 s nebo při dalším vstupu uživatele

---

## Audio Engine

Prioritní fallback řetězec:

**macOS:** `afplay`
**Linux:** `paplay` → `aplay` → `mpg123` → `mpg321` → ticho

Zvukové soubory jsou `.ogg` nebo `.wav`. Sound pack = adresář v `~/.config/cwinner/sounds/<pack-name>/` s pojmenovanými soubory:

```
mini.ogg
milestone.ogg
epic.ogg
fanfare.ogg
streak.ogg
```

Výchozí pack je součástí instalace. Uživatel může přidat vlastní nebo stáhnout komunity packy.

---

## State Engine

Perzistentní stav v `~/.local/share/cwinner/state.json`:

```json
{
  "xp": 1250,
  "level": 3,
  "level_name": "Vibe Architect",
  "commits_today": 7,
  "commit_streak_days": 4,
  "sessions_total": 42,
  "achievements_unlocked": ["first_commit", "streak_5", "test_whisperer"],
  "tools_used": ["Bash", "Write", "Read", "Glob", "Task"],
  "last_event_at": "2026-02-20T19:45:00Z"
}
```

XP systém:
- Mini event: 5 XP
- Milník: 25 XP
- Epický: 100 XP
- Streak bonus: 2× multiplikátor

Levely (pracovní názvy):
1. Vibe Initiate (0–100 XP)
2. Prompt Whisperer (100–500 XP)
3. Vibe Architect (500–1500 XP)
4. Flow State Master (1500–5000 XP)
5. Claude Sensei (5000+ XP)

---

## Konfigurace

Soubor `~/.config/cwinner/config.toml`:

```toml
[intensity]
routine = "off"
milestone = "medium"
breakthrough = "epic"

[audio]
enabled = true
sound_pack = "default"
volume = 0.8  # 0.0–1.0

[visual]
confetti = true
splash_screen = true
progress_bar = true
confetti_duration_ms = 1500
splash_duration_ms = 2000

[triggers]
# Vlastní trigger — oslava při specifickém bash příkazu
[[triggers.custom]]
name = "deploy"
pattern = "git push.*production"
intensity = "epic"

[streaks]
commit_streak_notify = [5, 10, 25, 50, 100]
```

---

## Instalace

Jednopříkazová instalace:

```bash
cwinner install
```

Automaticky provede:
1. Spustí `cwinnerd` a registruje jako systemd user service (Linux) nebo launchd plist (macOS)
2. Přidá PostToolUse, TaskCompleted, SessionEnd hooks do `~/.claude/settings.json`
3. Přidá `post-commit` a `post-push` hooks do `~/.config/git/hooks/`
4. Vytvoří default konfiguraci v `~/.config/cwinner/config.toml`
5. Rozbalí výchozí sound pack

```bash
cwinner uninstall   # odstraní vše
cwinner status      # stav daemonu + statistiky session
cwinner stats       # celkové statistiky, achievements
cwinner sounds list # dostupné sound packy
```

---

## Adresářová struktura projektu

```
cwinner/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs              # cwinner CLI entry point
│   ├── daemon/
│   │   ├── mod.rs           # cwinnerd entry point
│   │   ├── server.rs        # Unix socket server
│   │   ├── state.rs         # State engine
│   │   ├── celebration.rs   # Celebration engine (kontextová logika)
│   │   ├── renderer.rs      # TTY renderer (konfety, splash, progress bar)
│   │   └── audio.rs         # Audio engine
│   ├── hooks/
│   │   └── templates/       # Shell hook šablony
│   ├── install.rs           # Instalační logika
│   └── config.rs            # Config parsing (TOML)
├── sounds/
│   └── default/             # Výchozí sound pack
│       ├── mini.ogg
│       ├── milestone.ogg
│       ├── epic.ogg
│       ├── fanfare.ogg
│       └── streak.ogg
└── docs/
    └── plans/
        └── 2026-02-20-cwinner-design.md
```

---

## Technické závislosti (Rust crates)

| Crate | Účel |
|---|---|
| `tokio` | Async runtime pro Unix socket server |
| `serde` + `serde_json` | JSON serializace IPC zpráv |
| `toml` | Parsing konfiguračního souboru |
| `crossterm` | ANSI terminal manipulation (konfety, progress bar) |
| `clap` | CLI argument parsing |

Žádné runtime závislosti — všechno staticky linkováno v binárce. Sound playback přes systémové příkazy (`Command::new("afplay")`).

---

## Odlišení od existujících projektů

| | cwinner | Claude Quest | Claude Code Achievements |
|---|---|---|---|
| Zvuky | ✓ plné | ✗ | jen macOS notifikace |
| ASCII konfety | ✓ | ✗ | ✗ |
| Splash screen | ✓ | ✗ | ✗ |
| Multi-instance aware | ✓ | ✗ | ✗ |
| Git hooks | ✓ | ✗ | ✗ |
| Rust daemon | ✓ | ✗ | ✗ |
| Sound packy | ✓ | ✗ | ✗ |
| Vlastní triggery | ✓ | ✗ | ✗ |
