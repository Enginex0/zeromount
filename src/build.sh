#!/bin/bash
# ============================================================
# ZeroMount zm Binary Build Script
# ============================================================
# Mimics the original zeromount build process using zig compiler
# with extreme size optimization for minimal binary size.
#
# Requirements:
#   - zig compiler (install via: sudo apt install zig OR download from ziglang.org)
#   - Optional: sstrip from ELFkickers for extra stripping
#
# Usage:
#   ./build.sh          # Build both ARM64 and ARM32
#   ./build.sh arm64    # Build ARM64 only
#   ./build.sh arm      # Build ARM32 only
#   ./build.sh clean    # Remove built binaries
# ============================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_FILE="$SCRIPT_DIR/zm.c"
OUT_DIR="$SCRIPT_DIR/../module"

# Zig compiler flags for minimal binary size (from original zeromount)
ZIG_FLAGS=(
    -Oz                           # Optimize for size
    -static                       # Static linking
    -nostdlib                     # No standard library
    -ffreestanding                # Freestanding environment
    -fno-stack-protector          # No stack protection
    -fno-unwind-tables            # No unwind tables
    -fno-asynchronous-unwind-tables
    -fomit-frame-pointer          # Omit frame pointer
    -fno-ident                    # No ident section
    -flto                         # Link-time optimization
    -fmerge-all-constants         # Merge constants
    -Wl,--gc-sections             # Garbage collect sections
    -Wl,--build-id=none           # No build ID
    -Wl,-z,norelro                # No RELRO
    -Wl,-s                        # Strip symbols
    -Wl,--strip-all               # Strip all
    -Wl,--entry=_start            # Custom entry point
)

# Check for zig compiler
check_zig() {
    if ! command -v zig &> /dev/null; then
        echo "ERROR: zig compiler not found"
        echo ""
        echo "Install options:"
        echo "  Ubuntu/Debian: sudo apt install zig"
        echo "  Or download from: https://ziglang.org/download/"
        echo ""
        exit 1
    fi
    echo "Found zig: $(zig version)"
}

# Download and build sstrip if not present
setup_sstrip() {
    if [ -f "$SCRIPT_DIR/sstrip" ]; then
        echo "sstrip already available"
        return 0
    fi

    echo "Downloading ELFkickers for sstrip..."
    cd "$SCRIPT_DIR"

    if curl -L https://www.muppetlabs.com/~breadbox/pub/software/ELFkickers-3.2.tar.gz -o sstrip.tar.gz 2>/dev/null; then
        tar xf sstrip.tar.gz
        make -C ELFkickers-3.2 sstrip 2>/dev/null || true
        if [ -f ELFkickers-3.2/sstrip/sstrip ]; then
            cp ELFkickers-3.2/sstrip/sstrip .
            rm -rf ELFkickers-3.2 sstrip.tar.gz
            chmod +x sstrip
            echo "sstrip built successfully"
        else
            echo "WARNING: Could not build sstrip, skipping extra stripping"
            rm -rf ELFkickers-3.2 sstrip.tar.gz
        fi
    else
        echo "WARNING: Could not download sstrip, skipping extra stripping"
    fi
}

# Build for ARM64
build_arm64() {
    echo ""
    echo "=== Building ARM64 (aarch64) ==="
    zig cc -target aarch64-linux "${ZIG_FLAGS[@]}" "$SRC_FILE" -o "$OUT_DIR/zm-arm64"

    echo "  Before sstrip: $(wc -c < "$OUT_DIR/zm-arm64") bytes"

    if [ -f "$SCRIPT_DIR/sstrip" ]; then
        "$SCRIPT_DIR/sstrip" -z "$OUT_DIR/zm-arm64" 2>/dev/null || true
        echo "  After sstrip:  $(wc -c < "$OUT_DIR/zm-arm64") bytes"
    fi

    file "$OUT_DIR/zm-arm64"
}

# Build for ARM32
build_arm() {
    echo ""
    echo "=== Building ARM32 (arm) ==="
    zig cc -target arm-linux "${ZIG_FLAGS[@]}" "$SRC_FILE" -o "$OUT_DIR/zm-arm"

    echo "  Before sstrip: $(wc -c < "$OUT_DIR/zm-arm") bytes"

    if [ -f "$SCRIPT_DIR/sstrip" ]; then
        "$SCRIPT_DIR/sstrip" -z "$OUT_DIR/zm-arm" 2>/dev/null || true
        echo "  After sstrip:  $(wc -c < "$OUT_DIR/zm-arm") bytes"
    fi

    file "$OUT_DIR/zm-arm"
}

# Clean built files
clean() {
    echo "Cleaning built binaries..."
    rm -f "$OUT_DIR/zm-arm64" "$OUT_DIR/zm-arm"
    rm -f "$SCRIPT_DIR/sstrip" "$SCRIPT_DIR/sstrip.tar.gz"
    rm -rf "$SCRIPT_DIR/ELFkickers-3.2"
    echo "Done"
}

# Main
main() {
    echo "============================================================"
    echo " ZeroMount zm Binary Builder"
    echo "============================================================"

    mkdir -p "$OUT_DIR"

    case "${1:-all}" in
        arm64)
            check_zig
            setup_sstrip
            build_arm64
            ;;
        arm)
            check_zig
            setup_sstrip
            build_arm
            ;;
        all)
            check_zig
            setup_sstrip
            build_arm64
            build_arm
            ;;
        clean)
            clean
            exit 0
            ;;
        *)
            echo "Usage: $0 [arm64|arm|all|clean]"
            exit 1
            ;;
    esac

    echo ""
    echo "=== Build Complete ==="
    echo "Binaries located in: $OUT_DIR/"
    ls -la "$OUT_DIR"/zm-* 2>/dev/null || true
}

main "$@"
