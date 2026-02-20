#!/usr/bin/env bash
# cwinner hook: PostToolUse
# Voláno Claude Code po každém použití nástroje.
# Claude Code posílá JSON na stdin.

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"
SESSION_ID="${CLAUDE_SESSION_ID:-unknown}"

# Přečti stdin
INPUT=$(cat)

# Odešli event cwinner CLI (které předá daemonovi)
if command -v cwinner &>/dev/null && [ -S "$SOCKET" ]; then
    echo "$INPUT" | cwinner hook post-tool-use &>/dev/null &
fi
