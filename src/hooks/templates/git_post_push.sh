#!/usr/bin/env bash
# cwinner git hook: post-push
# Instalován do ~/.config/git/hooks/post-push

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"

if command -v cwinner &>/dev/null && [ -S "$SOCKET" ]; then
    echo '{}' | cwinner hook session-end &>/dev/null &
fi
