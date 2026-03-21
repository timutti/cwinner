# cwinner

[![CI](https://github.com/timutti/cwinner/actions/workflows/ci.yml/badge.svg)](https://github.com/timutti/cwinner/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/timutti/cwinner)](https://github.com/timutti/cwinner/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macOS-lightgrey.svg)]()

Gamification for [Claude Code](https://claude.ai/code). Tracks your progress, awards XP, and plays sounds on commits, completed tasks, and breakthrough moments.

![cwinner demo](assets/demo.gif)

## Features

- **XP and 200 levels** — every action in Claude Code earns points (with 2x streak bonus at 5+ day streaks)
- **5 distinct sounds** — multi-note synthesized WAV melodies generated at runtime, no external assets
- **Visual celebrations** — progress bars, centered toasts, confetti rain + splash boxes (all via alternate screen)
- **38 achievements** — commits, streaks, tools, levels, and Claude Code features
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

## Levels

200 levels across 20 themed arcs. Here are the highlights:

| Level | XP | Name | Arc |
|------:|---:|------|-----|
| 1 | 0 | Vibe Initiate | Vibe Origins |
| 5 | 5,000 | Claude Sensei | Vibe Origins |
| 10 | 75,000 | Singularity | Vibe Origins |
| 15 | 212,000 | Algorithm Ace | Digital Awakening |
| 20 | 372,000 | Kilobyte Knight | Digital Awakening |
| 25 | 560,000 | Hash Hermit | Code Elements |
| 30 | 780,000 | Pipeline Paladin | Code Elements |
| 40 | 1,340,000 | Data Duke | Data Domains |
| 50 | 2,120,000 | Network Nomad | Network Realms |
| 60 | 3,170,000 | Root Regent | System Spirits |
| 70 | 4,620,000 | Endpoint Emperor | Architect |
| 80 | 6,600,000 | Pipeline Pharaoh | Quality |
| 90 | 9,300,000 | Guardian Prime | Security |
| 100 | 13,100,000 | Rustacean | Type System |
| 110 | 18,200,000 | Infrastructure Imperator | Cloud |
| 120 | 25,200,000 | Token Titan | AI & ML |
| 130 | 34,800,000 | Error Eradicator | Mythical Debugging |
| 140 | 47,900,000 | Cosmos Crafter | Cosmic Code |
| 150 | 65,800,000 | Time Lord | Time |
| 160 | 90,300,000 | Atom Ascendant | Elements |
| 170 | 124,000,000 | Tesseract Titan | Dimensional |
| 180 | 170,000,000 | Mythril Monarch | Ancient Power |
| 190 | 233,000,000 | Deathless Debugger | Transcendence |
| 200 | 319,000,000 | Code God | The Pantheon |

<details>
<summary>Full level table (all 200 levels)</summary>

| Level | XP | Name |
|------:|---:|------|
| 1 | 0 | Vibe Initiate |
| 2 | 100 | Prompt Whisperer |
| 3 | 500 | Vibe Architect |
| 4 | 1,500 | Flow State Master |
| 5 | 5,000 | Claude Sensei |
| 6 | 10,000 | Code Whisperer |
| 7 | 20,000 | Vibe Lord |
| 8 | 35,000 | Zen Master |
| 9 | 50,000 | Transcendent |
| 10 | 75,000 | Singularity |
| 11 | 101,000 | Syntax Sage |
| 12 | 128,000 | Debug Dancer |
| 13 | 155,000 | Refactor Ronin |
| 14 | 183,000 | Pattern Prophet |
| 15 | 212,000 | Algorithm Ace |
| 16 | 242,000 | Logic Luminary |
| 17 | 273,000 | Stack Shaman |
| 18 | 305,000 | Binary Bard |
| 19 | 338,000 | Byte Bishop |
| 20 | 372,000 | Kilobyte Knight |
| 21 | 407,000 | Null Navigator |
| 22 | 443,000 | Pointer Pilgrim |
| 23 | 481,000 | Loop Laureate |
| 24 | 520,000 | Recursion Rider |
| 25 | 560,000 | Hash Hermit |
| 26 | 601,000 | Cache Cleric |
| 27 | 644,000 | Thread Thane |
| 28 | 688,000 | Mutex Monk |
| 29 | 733,000 | Buffer Baron |
| 30 | 780,000 | Pipeline Paladin |
| 31 | 828,000 | Schema Scribe |
| 32 | 878,000 | Query Quester |
| 33 | 930,000 | Index Oracle |
| 34 | 983,000 | Table Tactician |
| 35 | 1,040,000 | Row Ranger |
| 36 | 1,100,000 | Column Commander |
| 37 | 1,160,000 | Join Juggernaut |
| 38 | 1,220,000 | Shard Sentinel |
| 39 | 1,280,000 | Replica Rogue |
| 40 | 1,340,000 | Data Duke |
| 41 | 1,410,000 | Packet Pathfinder |
| 42 | 1,480,000 | Socket Sorcerer |
| 43 | 1,550,000 | Port Phantom |
| 44 | 1,620,000 | Protocol Priest |
| 45 | 1,700,000 | Firewall Falcon |
| 46 | 1,780,000 | Gateway Guardian |
| 47 | 1,860,000 | Proxy Prince |
| 48 | 1,940,000 | Latency Lancer |
| 49 | 2,030,000 | Bandwidth Baron |
| 50 | 2,120,000 | Network Nomad |
| 51 | 2,210,000 | Kernel Knight |
| 52 | 2,300,000 | Process Paladin |
| 53 | 2,400,000 | Memory Mage |
| 54 | 2,500,000 | Heap Herald |
| 55 | 2,600,000 | Stack Sovereign |
| 56 | 2,710,000 | Signal Sage |
| 57 | 2,820,000 | Daemon Druid |
| 58 | 2,930,000 | Cron Crusader |
| 59 | 3,050,000 | Shell Shaman |
| 60 | 3,170,000 | Root Regent |
| 61 | 3,290,000 | Module Maven |
| 62 | 3,420,000 | Package Phantom |
| 63 | 3,550,000 | Crate Captain |
| 64 | 3,690,000 | Monolith Monk |
| 65 | 3,830,000 | Microservice Mystic |
| 66 | 3,980,000 | API Apostle |
| 67 | 4,130,000 | REST Ranger |
| 68 | 4,290,000 | GraphQL Guru |
| 69 | 4,450,000 | Webhook Wizard |
| 70 | 4,620,000 | Endpoint Emperor |
| 71 | 4,790,000 | Test Templar |
| 72 | 4,970,000 | Assert Assassin |
| 73 | 5,150,000 | Coverage Centurion |
| 74 | 5,340,000 | Lint Lord |
| 75 | 5,530,000 | Format Friar |
| 76 | 5,730,000 | Review Raven |
| 77 | 5,940,000 | Merge Monarch |
| 78 | 6,150,000 | Deploy Deity |
| 79 | 6,370,000 | CI Champion |
| 80 | 6,600,000 | Pipeline Pharaoh |
| 81 | 6,830,000 | Cipher Centurion |
| 82 | 7,070,000 | Token Templar |
| 83 | 7,320,000 | Auth Archon |
| 84 | 7,580,000 | Vault Vanguard |
| 85 | 7,850,000 | Entropy Envoy |
| 86 | 8,120,000 | Hash Guardian |
| 87 | 8,400,000 | Payload Paladin |
| 88 | 8,690,000 | Sandbox Sage |
| 89 | 8,990,000 | Keymaster |
| 90 | 9,300,000 | Guardian Prime |
| 91 | 9,620,000 | Type Titan |
| 92 | 9,950,000 | Generic Gladiator |
| 93 | 10,300,000 | Trait Tempest |
| 94 | 10,700,000 | Lifetime Lorekeeper |
| 95 | 11,100,000 | Borrow Baron |
| 96 | 11,500,000 | Ownership Oracle |
| 97 | 11,900,000 | Closure Crusader |
| 98 | 12,300,000 | Macro Magus |
| 99 | 12,700,000 | Unsafe Usurper |
| 100 | 13,100,000 | Rustacean |
| 101 | 13,500,000 | Cloud Caller |
| 102 | 14,000,000 | Container Captain |
| 103 | 14,500,000 | Cluster Keeper |
| 104 | 15,000,000 | Pod Prophet |
| 105 | 15,500,000 | Volume Vagrant |
| 106 | 16,000,000 | Ingress Inquisitor |
| 107 | 16,500,000 | Service Scout |
| 108 | 17,000,000 | Helm Harbinger |
| 109 | 17,600,000 | Terraform Titan |
| 110 | 18,200,000 | Infrastructure Imperator |
| 111 | 18,800,000 | Neural Navigator |
| 112 | 19,400,000 | Tensor Templar |
| 113 | 20,000,000 | Gradient Guide |
| 114 | 20,700,000 | Model Maven |
| 115 | 21,400,000 | Epoch Elder |
| 116 | 22,100,000 | Attention Architect |
| 117 | 22,800,000 | Transformer Thane |
| 118 | 23,600,000 | Prompt Paladin |
| 119 | 24,400,000 | Context Commander |
| 120 | 25,200,000 | Token Titan |
| 121 | 26,000,000 | Segfault Slayer |
| 122 | 26,900,000 | Deadlock Destroyer |
| 123 | 27,800,000 | Race Resolver |
| 124 | 28,700,000 | Leak Liberator |
| 125 | 29,600,000 | Panic Purifier |
| 126 | 30,600,000 | Overflow Obliterator |
| 127 | 31,600,000 | Null Nemesis |
| 128 | 32,600,000 | Exception Exorcist |
| 129 | 33,700,000 | Bug Banisher |
| 130 | 34,800,000 | Error Eradicator |
| 131 | 35,900,000 | Stellar Scripter |
| 132 | 37,100,000 | Nebula Namer |
| 133 | 38,300,000 | Quasar Querier |
| 134 | 39,500,000 | Pulsar Programmer |
| 135 | 40,800,000 | Comet Coder |
| 136 | 42,100,000 | Orbit Optimizer |
| 137 | 43,500,000 | Eclipse Engineer |
| 138 | 44,900,000 | Supernova Sage |
| 139 | 46,400,000 | Galaxy Gardener |
| 140 | 47,900,000 | Cosmos Crafter |
| 141 | 49,400,000 | Async Ancestor |
| 142 | 51,000,000 | Future Forger |
| 143 | 52,600,000 | Promise Prophet |
| 144 | 54,300,000 | Await Arbiter |
| 145 | 56,100,000 | Concurrent Consul |
| 146 | 57,900,000 | Parallel Paragon |
| 147 | 59,800,000 | Temporal Titan |
| 148 | 61,700,000 | Chrono Champion |
| 149 | 63,700,000 | Epoch Emperor |
| 150 | 65,800,000 | Time Lord |
| 151 | 67,900,000 | Iron Invoker |
| 152 | 70,100,000 | Silicon Sage |
| 153 | 72,400,000 | Carbon Caster |
| 154 | 74,700,000 | Photon Phantom |
| 155 | 77,100,000 | Plasma Priest |
| 156 | 79,600,000 | Quantum Quester |
| 157 | 82,200,000 | Neutron Noble |
| 158 | 84,800,000 | Proton Prince |
| 159 | 87,500,000 | Electron Emperor |
| 160 | 90,300,000 | Atom Ascendant |
| 161 | 93,200,000 | Void Voyager |
| 162 | 96,200,000 | Matrix Master |
| 163 | 99,300,000 | Vector Virtuoso |
| 164 | 102,000,000 | Scalar Sovereign |
| 165 | 105,000,000 | Tensor Tyrant |
| 166 | 108,000,000 | Dimension Drifter |
| 167 | 112,000,000 | Plane Pathfinder |
| 168 | 116,000,000 | Realm Ruler |
| 169 | 120,000,000 | Sphere Sage |
| 170 | 124,000,000 | Tesseract Titan |
| 171 | 128,000,000 | Code Colossus |
| 172 | 132,000,000 | Digital Demigod |
| 173 | 136,000,000 | Cyber Centurion |
| 174 | 140,000,000 | Silicon Samurai |
| 175 | 145,000,000 | Chrome Chimera |
| 176 | 150,000,000 | Titanium Templar |
| 177 | 155,000,000 | Platinum Prophet |
| 178 | 160,000,000 | Diamond Druid |
| 179 | 165,000,000 | Obsidian Oracle |
| 180 | 170,000,000 | Mythril Monarch |
| 181 | 175,000,000 | Infinite Iterator |
| 182 | 181,000,000 | Eternal Evaluator |
| 183 | 187,000,000 | Boundless Builder |
| 184 | 193,000,000 | Limitless Linker |
| 185 | 199,000,000 | Perpetual Parser |
| 186 | 205,000,000 | Timeless Typer |
| 187 | 212,000,000 | Ageless Allocator |
| 188 | 219,000,000 | Undying Unwrapper |
| 189 | 226,000,000 | Immortal Indexer |
| 190 | 233,000,000 | Deathless Debugger |
| 191 | 240,000,000 | Omega Overseer |
| 192 | 248,000,000 | Alpha Architect |
| 193 | 256,000,000 | Prime Programmer |
| 194 | 264,000,000 | Supreme Scripter |
| 195 | 272,000,000 | Absolute Admin |
| 196 | 281,000,000 | Ultimate Unifier |
| 197 | 290,000,000 | Sovereign Source |
| 198 | 299,000,000 | Eternal Engine |
| 199 | 309,000,000 | Apex Automaton |
| 200 | 319,000,000 | Code God |

</details>

## Sound packs

The default pack ships 5 synthesized multi-note WAV melodies (generated at install time, no external assets). A custom pack is a directory of WAV/OGG/MP3 files under `~/.config/cwinner/sounds/<name>/`:

```
mini.wav        # quick double-tap — played on level-up
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
