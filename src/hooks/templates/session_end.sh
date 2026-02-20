#!/usr/bin/env bash
# cwinner hook: SessionEnd (Stop)

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
SESSION_ID="${CLAUDE_SESSION_ID:-unknown}"

INPUT=$(cat)

if command -v cwinner &>/dev/null && [ -S "$SOCKET" ]; then
    echo "$INPUT" | cwinner hook session-end &>/dev/null &
fi
