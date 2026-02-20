#!/usr/bin/env bash
# cwinner git hook: post-commit
# Instalován do ~/.config/git/hooks/post-commit

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"

if command -v cwinner &>/dev/null && [ -S "$SOCKET" ]; then
    echo '{}' | cwinner hook post-tool-use &>/dev/null &
fi
