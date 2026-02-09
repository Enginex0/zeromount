#!/system/bin/sh
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ABI=arm64-v8a ;;
    armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64) ABI=x86_64 ;;
    i686|i386) ABI=x86 ;;
    *) exit 0 ;;
esac

BIN="$MODDIR/bin/${ABI}/zeromount"
[ -x "$BIN" ] || exit 0

# Post-boot tasks: UID blocking, WebUI symlink, module watcher
"$BIN" mount --post-boot
