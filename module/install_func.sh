#!/system/bin/sh
# Volume-key selection and config stash/restore for install flow.
# Sourced by customize.sh. Expects $ZM_DATA and $TMPDIR to be set.

choose_config() {
    command -v getevent >/dev/null 2>&1 || return 0
    local vol_tmp="$TMPDIR/vol_key"
    local seconds="${1:-10}"
    local ge_pid=""

    : > "$vol_tmp"
    getevent -qlc 1 > "$vol_tmp" 2>/dev/null &
    ge_pid=$!

    while [ "$seconds" -gt 0 ] || [ "$1" = "0" ]; do
        sleep 1
        if ! kill -0 "$ge_pid" 2>/dev/null; then
            local key
            key=$(awk '/KEY_/{print $3}' "$vol_tmp" 2>/dev/null)
            case "$key" in
                KEY_VOLUMEUP)
                    rm -f "$vol_tmp"
                    return 0
                    ;;
                KEY_VOLUMEDOWN)
                    rm -f "$vol_tmp"
                    return 1
                    ;;
            esac
            : > "$vol_tmp"
            getevent -qlc 1 > "$vol_tmp" 2>/dev/null &
            ge_pid=$!
        fi
        [ "$1" = "0" ] || seconds=$((seconds - 1))
    done

    kill "$ge_pid" 2>/dev/null
    wait "$ge_pid" 2>/dev/null
    rm -f "$vol_tmp"
    return 0
}

stash_config() {
    local stash="$ZM_DATA/.stash"
    mkdir -p "$stash"
    cp "$ZM_DATA/config.toml" "$stash/config.toml" 2>/dev/null
    cp "$ZM_DATA/config.toml.bak" "$stash/config.toml.bak" 2>/dev/null
}

restore_stash() {
    local stash="$ZM_DATA/.stash"
    [ -f "$stash/config.toml" ] || return 1
    cp "$stash/config.toml" "$ZM_DATA/config.toml"
    cp "$stash/config.toml.bak" "$ZM_DATA/config.toml.bak" 2>/dev/null
    rm -rf "$stash"
    return 0
}
