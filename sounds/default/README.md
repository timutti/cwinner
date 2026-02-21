# sounds/default/

Výchozí sound pack pro cwinner. Zvuky jsou multi-notové melodie generované sinusovou syntézou při `cwinner install`.

## Soubory (generovány automaticky)

- `mini.wav` — quick double-tap notification (E6 → G6, 0.2s)
- `milestone.wav` — rising two-note chime (C5 → E5, 0.6s)
- `epic.wav` — C major chord with swell (C4+E4+G4+C5, 1.0s)
- `fanfare.wav` — ascending four-note trumpet call (C5 → E5 → G5 → C6, 1.2s)
- `streak.wav` — rapid ascending scale with echo + final chord (1.6s)

## Mapování na celebration levels

- **Mini** → `mini.wav`
- **Medium** (bez achievementu) → `milestone.wav`
- **Medium** (s achievementem) → `epic.wav`
- **Epic** → `fanfare.wav`
- **Epic** (streak milestone) → `streak.wav`

## Formáty

Podporovány: `.ogg`, `.wav`, `.mp3`. Výchozí pack používá `.wav` (mono, 16-bit PCM, 44100 Hz).

Pokud soubor v pack adresáři chybí, cwinner vygeneruje `.wav` do `/tmp/cwinner/` jako fallback.

## Vlastní pack

Zkopíruj tento adresář do `~/.config/cwinner/sounds/<muj-pack>/`
a nastav v `config.toml`: `sound_pack = "muj-pack"`.

## Zdroje pro vlastní zvuky (CC0 licence)

- https://freesound.org
- https://opengameart.org
