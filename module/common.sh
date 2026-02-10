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
