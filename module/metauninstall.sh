#!/system/bin/sh
# Runs when ANOTHER module is uninstalled -- clean VFS rules and SUSFS entries
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ABI=arm64-v8a ;; armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64) ABI=x86_64 ;; i686|i386) ABI=x86 ;;
esac

"$MODDIR/bin/${ABI}/zeromount" module scan --cleanup "$1" 2>/dev/null
