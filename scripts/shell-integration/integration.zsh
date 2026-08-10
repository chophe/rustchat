_rustchat_zsh() {
    if [[ -n "$BUFFER" ]]; then
        local _old=$BUFFER
        BUFFER+="⌛"
        zle -I && zle redisplay
        BUFFER=$(rustchat -e "$_old")
        zle end-of-line
    fi
}
zle -N _rustchat_zsh
bindkey '\ee' _rustchat_zsh