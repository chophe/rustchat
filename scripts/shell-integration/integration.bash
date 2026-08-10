_rustchat_bash() {
    if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE=$(rustchat -e "$READLINE_LINE")
        READLINE_POINT=${#READLINE_LINE}
    fi
}
bind -x '"\ee": _rustchat_bash'