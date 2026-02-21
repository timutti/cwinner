#!/usr/bin/env bash
# cwinner git hook: post-commit
# Installed to ~/.config/git/hooks/post-commit

SOCKET="${XDG_DATA_HOME:-$HOME/.local/share}/cwinner/cwinner.sock"
TTY_PATH="$(tty 2>/dev/null || echo /dev/null)"

if [ -S "$SOCKET" ] && command -v socat &>/dev/null; then
    printf '{"event":"GitCommit","tool":null,"session_id":"git","tty_path":"%s","metadata":{}}\n' \
        "$TTY_PATH" | socat -t 0.5 - "UNIX-CONNECT:$SOCKET" &>/dev/null &
elif [ -S "$SOCKET" ] && command -v nc &>/dev/null; then
    JSON=$(printf '{"event":"GitCommit","tool":null,"session_id":"git","tty_path":"%s","metadata":{}}' "$TTY_PATH")
    if [ "$(uname)" = "Darwin" ]; then
        printf '%s\n' "$JSON" | nc -U "$SOCKET" &>/dev/null &
    else
        printf '%s\n' "$JSON" | nc -U -q 1 "$SOCKET" &>/dev/null &
    fi
fi
