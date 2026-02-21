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
│  │afplay/      │  │ TOML soubor  │                      │
│  │pw-play/     │  │              │                      │
│  │paplay/aplay │  │              │                      │
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

**cwinnerd** — hlavní Rust daemon, běží jako systemd user service (Linux) nebo launchd agent (macOS). Naslouchá na Unix socketu. Drží veškerý stav v paměti, persistuje do `~/.local/share/cwinner/state.json`. Sleduje per-session informace (`SessionInfo`) — počet commitů v session a milníky délky trvání session (1h/3h/8h).

**Hook skripty** — tenké shell skripty (bash) pro git hooks a Rust CLI subcommand `cwinner hook` pro Claude Code hooks. Instalované do `~/.claude/settings.json` (CC hooks) a `~/.config/git/hooks/` (git hooks). Git hooks používají `socat` nebo `nc` pro komunikaci se socketem. CC hook:
1. Přečte JSON ze stdin (Claude Code posílá metadata)
2. Zjistí `tty_path` procházením process tree (`/proc/PID/fd/`) a hledáním `/dev/pts/N`
3. Odešle Event přes Unix socket daemonovi
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

`EventKind` enum používá PascalCase serde serializaci (`#[serde(rename_all = "PascalCase")]`):
- `PostToolUse`
- `PostToolUseFailure`
- `TaskCompleted`
- `SessionEnd`
- `GitCommit`
- `GitPush`
- `UserDefined`

Daemon odpovídá synchronně jen pro `status` a `stats` příkazy (`DaemonCommand`). Eventy jsou fire-and-forget.

---

## Triggery

| Trigger | Zdroj | Výchozí intenzita |
|---|---|---|
| `PostToolUse: Write/Edit/Read` | CC hook | routine (výchozí off) |
| `PostToolUse: Bash` + exit 0 (běžný) | CC hook | routine (výchozí off) |
| `PostToolUse: Bash` + exit 0 po předchozím failu | CC hook | breakthrough (epic) |
| `PostToolUse: Bash` + shoda s custom trigger | CC hook | dle trigger konfigurace |
| `TaskCompleted` | CC hook | milestone (medium) |
| `GitCommit` | git post-commit hook | milestone (medium) |
| `GitPush` | git pre-push hook | breakthrough (epic) |
| `SessionEnd` | CC hook | milestone (medium) |
| `SessionEnd` s >=1 commitem v session | daemon | epic |
| Commit streak milestone (5 / 10 / 25 / 100 dní) | daemon | epic |
| Session délka milestone (1h / 3h / 8h) | daemon | medium / medium / epic |
| Uživatelem definovaný trigger (`[[triggers.custom]]`) | config | nastavitelné |

Daemon detekuje "průlom" pokud byl předchozí stav chybový (`last_bash_exit != 0` -> nový exit 0).

Custom triggery mají přednost před ostatní logikou — pokud bash příkaz odpovídá pattern, použije se intenzita triggeru (i když by jinak proběhl průlom).

---

## Celebration Engine

Kontextová logika rozhodující o intenzitě oslavy (`celebration.rs`):

```
event přijde
  ├── PostToolUse: Bash?
  │     ├── odpovídá custom trigger? → intenzita dle triggeru
  │     ├── exit 0 a předchozí fail? → BREAKTHROUGH (epic)
  │     ├── exit 0? → ROUTINE (default off)
  │     └── exit != 0? → OFF
  ├── PostToolUse: Write/Edit/Read? → ROUTINE (default off)
  ├── TaskCompleted? → MILESTONE (medium)
  ├── GitCommit? → MILESTONE (medium)
  ├── GitPush? → BREAKTHROUGH (epic)
  ├── SessionEnd? → MILESTONE (medium)
  ├── PostToolUseFailure? → OFF
  └── ostatní → ROUTINE (default off)
```

V `server.rs` se navíc aplikují upgrady:
- `SessionEnd` s >=1 commitem v session → upgrade na Epic
- Streak milestone (5/10/25/100 dní) při `GitCommit` → upgrade na Epic
- Duration milestone (1h/3h/8h session) → upgrade na nejvyšší z aktuální a milestone úrovně

### XP systém

- Off: 0 XP
- Mini: 5 XP
- Medium: 25 XP
- Epic: 100 XP
- Streak bonus: 2x multiplikátor pokud `commit_streak_days >= 5`

### Session tracking

Daemon udržuje `SessionInfo` per `session_id` (runtime-only, neperzistováno):
- `started_at: Instant` — začátek session
- `commits: u32` — počet commitů v session
- `duration_milestones_fired: Vec<u64>` — minuty již oslavených milníků

Duration milníky (`DURATION_MILESTONES`):
- 60 min (1h) → Medium
- 180 min (3h) → Medium
- 480 min (8h) → Epic

Při `SessionEnd` se session odstraní z mapy. Milníky se kontrolují při každém eventu.

---

## TTY Renderer

Daemon přijme `tty_path`, otevře descriptor, zapíše ANSI sekvence přes crossterm. Všechny vizuální oslavy používají alternate screen buffer pro kompatibilitu s Claude Code diferenciálním rendererem.

### Render lock a cooldown

Globální `RENDER_LOCK` (Mutex) zabraňuje souběžným přepínáním alternate screenu. Minimální cooldown mezi rendery: **5 sekund** (`RENDER_COOLDOWN`). Před renderem je **200ms pre-render delay** (čeká se než Claude Code dokončí svůj render).

### Celebration levels → vizuální output

| CelebrationLevel | Vizuální efekt |
|---|---|
| Off | nic |
| Mini | progress bar na spodním řádku (3s, alternate screen) |
| Medium | toast overlay uprostřed obrazovky (1.5s, nebo 2.5s s achievementem) |
| Epic | konfety (1.5s) + splash box (2s), vše v jednom alternate screen |

### Progress Bar (Mini)
- Pozice: spodní řádek terminálu
- Alternate screen buffer
- Formát: `⚡ {level_name} │ {bar} │ {xp} XP` (nebo `MAX` pro max level)
- XP bar: 15 znaků (`█` vyplněný, `░` prázdný)
- Trvání: 3 sekundy

### Toast (Medium)
- Pozice: střed obrazovky (vycentrováno)
- Alternate screen buffer
- Bez achievementu: `⚡ {level_name} │ {bar} │ {xp} XP` (cyan, 1.5s)
- S achievementem: `🏆 {achievement} │ {level_name} │ {xp} XP` (yellow, 2.5s)

### Epic celebration
- Fáze 1: konfety (1.5s) — znaky `✦ ★ ♦ ● * + # ✿ ❋` v 7 barvách, 15 framů, náhodné pozice
- Fáze 2: splash box (2s) — boxový rámeček (`╔═╗║╚╝`) s názvem achievementu, levelem a XP
- Vše v jednom alternate screen (bez flicker)
- Cursor skrytý po celou dobu

---

## Audio Engine

5 zvuků (`SoundKind`): `mini`, `milestone`, `epic`, `fanfare`, `streak`.

Mapování CelebrationLevel na zvuk (`celebration_to_sound`):
- Off → žádný zvuk
- Mini → `mini`
- Medium → `milestone` (nebo `epic` pokud je achievement)
- Epic → `fanfare` (nebo `streak` pokud je streak milestone)

### Přehrávání

Prioritní fallback řetězec přehrávačů:

**macOS:** `afplay`
**Linux:** `pw-play` → `paplay` → `aplay` → `mpg123` → `mpg321` → ticho

Detekce přehrávače přes `which`. Přehrávání je fire-and-forget (`Command::spawn`).

### Sound packy

Hledání zvukového souboru: `~/.config/cwinner/sounds/{pack-name}/{kind}.{ogg|wav|mp3}`.

Pokud soubor v pack adresáři neexistuje, fallback: **generování `.wav` za běhu** přes sinusovou syntézu do `/tmp/cwinner/{kind}.wav`. Parametry:

| Zvuk | Frekvence | Délka |
|---|---|---|
| mini | 880 Hz (A5) | 0.3s |
| milestone | 523.25 Hz (C5) | 0.8s |
| epic | 659.25 Hz (E5) | 1.2s |
| fanfare | 783.99 Hz (G5) | 1.5s |
| streak | 1046.5 Hz (C6) | 1.5s |

WAV soubory: mono, 16-bit PCM, 44100 Hz sample rate, lineární fade-out envelope. Generované přes `sounds::generate_wav()` a `sounds::encode_wav()`.

Příkaz `cwinner install` extrahuje výchozí pack (generované `.wav`) do `~/.config/cwinner/sounds/default/`.

---

## State Engine

Perzistentní stav v `~/.local/share/cwinner/state.json`:

```json
{
  "xp": 1250,
  "level": 3,
  "level_name": "Vibe Architect",
  "commits_total": 47,
  "commit_streak_days": 4,
  "last_commit_date": "2026-02-20",
  "sessions_total": 42,
  "achievements_unlocked": ["first_commit", "streak_5", "test_whisperer"],
  "tools_used": ["Bash", "Write", "Read", "Glob", "Task"],
  "last_event_at": "2026-02-20T19:45:00Z",
  "last_bash_exit": 0
}
```

### Streak milníky

Konstantní pole `STREAK_MILESTONES`: `[5, 10, 25, 100]` dní.

Metoda `record_commit()` vrací `CommitResult`:
- `first_today: bool` — první commit dne
- `streak_milestone: Option<u32>` — pokud streak právě dosáhl milníku

### Levely (10 levelů)

| Level | XP práh | Název |
|---|---|---|
| 1 | 0 | Vibe Initiate |
| 2 | 100 | Prompt Whisperer |
| 3 | 500 | Vibe Architect |
| 4 | 1 500 | Flow State Master |
| 5 | 5 000 | Claude Sensei |
| 6 | 10 000 | Code Whisperer |
| 7 | 20 000 | Vibe Lord |
| 8 | 35 000 | Zen Master |
| 9 | 50 000 | Transcendent |
| 10 | 75 000 | Singularity |

---

## Achievements (26)

| ID | Název | Podmínka |
|---|---|---|
| `first_commit` | First Commit | Prvním git commit |
| `commit_10` | Getting Committed | 10 commitů celkem |
| `commit_50` | Commit Machine | 50 commitů celkem |
| `commit_100` | Centurion | 100 commitů celkem |
| `streak_5` | On a Roll | 5denní commit streak |
| `streak_10` | Unstoppable | 10denní commit streak |
| `streak_25` | Dedicated | 25denní commit streak |
| `first_push` | Shipped It | První git push |
| `test_whisperer` | Test Whisperer | Bash exit 0 po předchozím failu |
| `tool_explorer` | Tool Explorer | 5 různých nástrojů |
| `tool_master` | Tool Master | 10 různých nástrojů |
| `level_2` | Prompt Whisperer | Dosažení level 2 |
| `level_3` | Vibe Architect | Dosažení level 3 |
| `level_4` | Flow State Master | Dosažení level 4 |
| `level_5` | Claude Sensei | Dosažení level 5 |
| `level_7` | Vibe Lord | Dosažení level 7 |
| `level_10` | Singularity | Dosažení level 10 |
| `first_subagent` | Delegator | Použití Task nástroje (subagent) |
| `web_surfer` | Web Surfer | Použití WebSearch |
| `researcher` | Deep Researcher | Použití WebFetch |
| `mcp_pioneer` | MCP Pioneer | Použití MCP nástroje (`mcp__*`) |
| `notebook_scientist` | Data Scientist | Použití NotebookEdit |
| `todo_master` | Organized | Použití TodoWrite |
| `first_skill` | Skilled Up | Použití Skill (slash command) |
| `first_team` | Team Player | Použití TeamCreate |
| `team_communicator` | Team Lead | Použití SendMessage |

Kontrola achievementů (`check_achievements`) probíhá při každém eventu PŘED aktualizací `last_bash_exit` (aby `test_whisperer` mohl porovnat starý stav).

---

## Konfigurace

Soubor `~/.config/cwinner/config.toml`:

```toml
[intensity]
routine = "off"          # off | mini | medium | epic
milestone = "medium"
breakthrough = "epic"

[audio]
enabled = true
sound_pack = "default"
volume = 0.8             # 0.0–1.0

[visual]
confetti = true
splash_screen = true
progress_bar = true
confetti_duration_ms = 1500
splash_duration_ms = 2000

# Vlastní triggery — oslava při specifickém bash příkazu (substring match)
[[triggers.custom]]
name = "deploy"
pattern = "git push.*production"
intensity = "epic"
```

### Config struct

```rust
pub struct Config {
    pub intensity: IntensityConfig,    // routine, milestone, breakthrough
    pub audio: AudioConfig,            // enabled, sound_pack, volume
    pub visual: VisualConfig,          // confetti, splash_screen, progress_bar, durations
    pub triggers: TriggersConfig,      // custom: Vec<CustomTrigger>
}
```

Všechna pole mají `#[serde(default)]` — chybějící sekce se doplní výchozími hodnotami.

---

## Instalace

Jednopříkazová instalace:

```bash
cwinner install
```

Automaticky provede:
1. Přidá PostToolUse, TaskCompleted, Stop hooks do `~/.claude/settings.json`
2. Nainstaluje `post-commit` a `pre-push` git hooks do `~/.config/git/hooks/`
3. Vytvoří default konfiguraci v `~/.config/cwinner/config.toml`
4. Vygeneruje výchozí sound pack (`.wav`) do `~/.config/cwinner/sounds/default/`
5. Vytvoří adresář stavu `~/.local/share/cwinner/`
6. Registruje `cwinnerd` jako systemd user service (Linux) nebo launchd agent (macOS)

```bash
cwinner uninstall   # odstraní service, zastaví daemon
cwinner status      # stav: level, XP, streak, commity
cwinner stats       # celkové statistiky, progress bar, achievements (locked/unlocked)
cwinner sounds list # dostupné sound packy
cwinner daemon      # spusť daemon přímo (bez service manageru)
```

---

## Adresářová struktura projektu

```
cwinner/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs              # cwinner CLI entry point (clap)
│   ├── daemon_main.rs       # cwinnerd standalone entry point
│   ├── lib.rs               # knihovní crate (cwinner_lib)
│   ├── config.rs            # Config parsing (TOML)
│   ├── event.rs             # Event, EventKind, DaemonCommand, DaemonResponse
│   ├── state.rs             # State engine (XP, levely, streaky, persistence)
│   ├── celebration.rs       # Celebration engine (kontextová logika, XP přidělování)
│   ├── renderer.rs          # TTY renderer (progress bar, toast, epic, render lock)
│   ├── audio.rs             # Audio engine (player detection, playback, sound mapping)
│   ├── sounds.rs            # WAV generátor (sinusová syntéza)
│   ├── achievements.rs      # 26 achievements — REGISTRY a check_achievements
│   ├── install.rs           # Instalační logika (hooks, service, config, sounds)
│   ├── daemon/
│   │   ├── mod.rs           # re-export server::run
│   │   └── server.rs        # Unix socket server, SessionInfo, process_event_with_state
│   └── hooks/
│       └── templates/
│           ├── git_post_commit.sh   # Git post-commit hook šablona
│           └── git_pre_push.sh      # Git pre-push hook šablona
└── docs/
    └── plans/
        └── 2026-02-20-cwinner-design.md
```

---

## Technické závislosti (Rust crates)

| Crate | Účel |
|---|---|
| `tokio` | Async runtime pro Unix socket server |
| `serde` + `serde_json` | JSON serializace IPC zpráv a stavu |
| `toml` | Parsing konfiguračního souboru |
| `crossterm` | Terminal manipulation (alternate screen, cursor, barvy) |
| `clap` | CLI argument parsing |
| `chrono` | Datum/čas pro streak tracking |
| `rand` | Náhodné pozice/barvy konfet |
| `dirs` | XDG cesty (config, data, home) |
| `anyhow` | Error handling |
| `libc` | TIOCGWINSZ ioctl pro zjištění velikosti terminálu |

Dev dependencies: `tempfile`.

Žádné runtime závislosti — všechno staticky linkováno v binárce. Sound playback přes systémové příkazy (`Command::new("pw-play")` apod.).

Dva binární targety: `cwinner` (CLI) a `cwinnerd` (daemon). Sdílený kód v `cwinner_lib`.

---

## Odlišení od existujících projektů

| | cwinner | Claude Quest | Claude Code Achievements |
|---|---|---|---|
| Zvuky | plné (5 druhů + syntéza) | -- | jen macOS notifikace |
| ASCII konfety | ano | -- | -- |
| Splash screen | ano | -- | -- |
| Progress bar | ano | -- | -- |
| Multi-instance aware | ano (Unix socket) | -- | -- |
| Git hooks | ano | -- | -- |
| Rust daemon | ano | -- | -- |
| Sound packy | ano | -- | -- |
| Vlastní triggery | ano | -- | -- |
| 26 achievements | ano | -- | -- |
| 10 levelů | ano | -- | -- |
| Session duration tracking | ano | -- | -- |
| Streak milestones | ano | -- | -- |
