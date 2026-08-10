function _rustchat_fish
    set -l _old (commandline)
    if test -n $_old
        echo -n "⌛"
        commandline -f repaint
        commandline (rustchat -e $_old)
    end
end
bind \ee _rustchat_fish