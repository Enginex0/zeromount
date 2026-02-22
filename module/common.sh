#!/system/bin/sh
# Shared ABI detection. Caller handles unsupported-arch exit.
# After sourcing: $ABI set (or empty), $BIN set (if MODDIR and ABI both set).

case "$(uname -m)" in
    aarch64)       ABI=arm64-v8a ;;
    armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64)        ABI=x86_64 ;;
    i686|i386)     ABI=x86 ;;
    *)             ABI="" ;;
esac

if [ -n "$MODDIR" ] && [ -n "$ABI" ]; then
    BIN="$MODDIR/bin/${ABI}/zeromount"
fi

# USAGE: susfs_hexpatch_prop_name <prop name> <search value> <replace value>
#        <search value> and <replace value> must have the same length.
# Patches the prop name bytes in-place in /dev/__properties__ so apps cannot
# query the prop by its original name (Pixel verification hardening).
# Credit: osm0sis, changhuapeng (LOSPropsGoAway)
susfs_hexpatch_prop_name() {
    local NAME="$1"
    local CURVALUE="$2"
    local NEWVALUE="$3"
    [ ${#CURVALUE} -ne ${#NEWVALUE} ] && return 1

    if [ -f /dev/__properties__ ]; then
        local PROPFILE=/dev/__properties__
    else
        local PROPFILE="/dev/__properties__/$(resetprop -Z "$NAME")"
    fi

    if [ -f "$PROPFILE" ]; then
        NAME=${NAME##*.}
        while true; do
            local NAMEOFFSET=$(echo $(strings -t d "$PROPFILE" | grep "$NAME") | cut -d ' ' -f 1)
            if [ -z "${NAMEOFFSET}" ]; then
                break
            fi
            local NEWSTR=$(echo "$NAME" | sed 's/'"$CURVALUE"'/'"$NEWVALUE"'/g')
            local NAMELEN=${#NAME}
            local NEWHEX=$(printf "$NEWSTR" | od -A n -t x1 -v | tr -d ' \n')
            echo -ne $(printf "$NEWHEX" | sed -e 's/.\{2\}/&\\x/g' -e 's/^/\\x/' -e 's/\\x$//') | dd obs=1 count=$NAMELEN seek=$NAMEOFFSET conv=notrunc of="$PROPFILE"
        done
    fi
}
