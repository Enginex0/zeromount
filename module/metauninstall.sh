#!/system/bin/sh
# Runs when ANOTHER module is uninstalled -- clean VFS rules and SUSFS entries
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ARCH=arm64 ;; armv7*|armv8l) ARCH=arm ;;
    x86_64) ARCH=x86_64 ;; i686|i386) ARCH=x86 ;;
esac

"$MODDIR/zm-${ARCH}" module scan --cleanup "$1" 2>/dev/null
