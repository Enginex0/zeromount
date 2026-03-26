#!/usr/bin/env bash
# Full build pipeline: cross-compile Rust (debug + release), build WebUI, package module ZIPs.
# Usage: ./scripts/package.sh --build [--version v2.0.0-dev] [--clean]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MODULE_DIR="$PROJECT_ROOT/module"
WEBUI_DIR="$PROJECT_ROOT/webui"
RELEASE_DIR="$PROJECT_ROOT/release"

CURRENT_VERSION="$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
VERSION=""
BUILD=false
CLEAN=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --build)   BUILD=true; shift ;;
        --clean)   CLEAN=true; shift ;;
        *)         echo "Unknown arg: $1"; exit 1 ;;
    esac
done

# Auto-bump patch version unless explicitly provided
if [ -z "$VERSION" ]; then
    IFS='.-' read -r major minor patch pre <<< "$CURRENT_VERSION"
    patch=$((patch + 1))
    if [ -n "$pre" ]; then
        NEW_VERSION="${major}.${minor}.${patch}-${pre}"
    else
        NEW_VERSION="${major}.${minor}.${patch}"
    fi

    sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$PROJECT_ROOT/Cargo.toml"

    vcode="${NEW_VERSION%%-*}"
    vcode="${vcode//./}"
    sed -i "s/^version=.*/version=v${NEW_VERSION}/" "$MODULE_DIR/module.prop"
    sed -i "s/^versionCode=.*/versionCode=${vcode}/" "$MODULE_DIR/module.prop"

    VERSION="v${NEW_VERSION}"
    echo "==> Version bumped: v${CURRENT_VERSION} → ${VERSION}"
else
    VERSION="${VERSION#v}"
    VERSION="v${VERSION}"
fi

mkdir -p "$RELEASE_DIR/debug" "$RELEASE_DIR/release"

if [ "$CLEAN" = true ]; then
    echo "==> Cleaning old releases"
    rm -f "$RELEASE_DIR"/debug/zeromount-*.zip "$RELEASE_DIR"/release/zeromount-*.zip
fi

SCRIPTS=(
    action.sh
    boot-completed.sh
    common.sh
    install_func.sh
    install_i18n.sh
    post-fs-data.sh
    metamount.sh
    service.sh
    uninstall.sh
    metainstall.sh
    metauninstall.sh
    customize.sh
)

declare -A ABI_TARGET=(
    [arm64-v8a]=aarch64-linux-android
    [armeabi-v7a]=armv7-linux-androideabi
    [x86_64]=x86_64-linux-android
    [x86]=i686-linux-android
)

setup_toolchain() {
    export NDK_BIN="/opt/android-ndk-r25b/toolchains/llvm/prebuilt/linux-x86_64/bin"
    if [ ! -d "$NDK_BIN" ]; then
        echo "FATAL: Android NDK not found at /opt/android-ndk-r25b" >&2
        exit 1
    fi
    if [ -f "/home/president/.cargo/bin/cargo" ]; then
        export RUSTUP_HOME=/home/president/.rustup
        export CARGO_HOME=/home/president/.cargo
        CARGO="/home/president/.cargo/bin/cargo"
    else
        CARGO="cargo"
    fi
    export PATH="$NDK_BIN:$PATH"
}

# Build Rust for one profile across all ABIs
build_rust() {
    local profile="$1"
    local cargo_flag=""
    local target_subdir="debug"

    if [ "$profile" = "release" ]; then
        cargo_flag="--release"
        target_subdir="release"
    fi

    for abi in "${!ABI_TARGET[@]}"; do
        target="${ABI_TARGET[$abi]}"
        echo "==> [$profile] Building $abi ($target)"
        "$CARGO" build --manifest-path "$PROJECT_ROOT/Cargo.toml" \
            --target "$target" $cargo_flag 2>&1
    done
    echo "==> [$profile] All Rust targets built"
}

build_axon() {
    local axon_src="${AXON_SRC:-$PROJECT_ROOT/external/axon}"
    if [ ! -f "$axon_src/inject.c" ]; then
        echo "WARN: axon source not found at $axon_src, skipping" >&2
        return 0
    fi

    local api=23
    local build_dir="$PROJECT_ROOT/target/axon"

    # ptrace.h only supports aarch64 + x86_64 — ARM32/x86 hit #error
    declare -A CLANG_TARGET=(
        [arm64-v8a]=aarch64-linux-android
        [x86_64]=x86_64-linux-android
    )

    for abi in "${!CLANG_TARGET[@]}"; do
        local prefix="${CLANG_TARGET[$abi]}"
        local cc="$NDK_BIN/${prefix}${api}-clang"
        local ar="$NDK_BIN/llvm-ar"
        local out="$build_dir/$abi"

        if [ ! -x "$cc" ]; then
            echo "WARN: clang not found for $abi ($cc), skipping" >&2
            continue
        fi

        echo "==> [axon] Building $abi"
        mkdir -p "$out"

        local cflags="-fvisibility=hidden -Os -DANDROID"

        "$cc" $cflags -c "$axon_src/external/plthook/plthook_elf.c" \
            -I"$axon_src/external/plthook" -o "$out/plthook_elf.o"
        "$ar" rcs "$out/libplthook.a" "$out/plthook_elf.o"

        "$cc" $cflags -s \
            "$axon_src/inject.c" "$axon_src/ptrace.c" "$axon_src/utils.c" \
            -o "$out/axon_inject"

        "$cc" $cflags -shared -s \
            "$axon_src/axon_init.c" "$axon_src/utils.c" \
            -I"$axon_src/external/plthook" -L"$out" -lplthook \
            -o "$out/libaxon_init.so"

        "$cc" $cflags -shared -s \
            "$axon_src/axon_adbd.c" "$axon_src/utils.c" \
            -ldl -o "$out/libaxon_adbd.so"

        mkdir -p "$MODULE_DIR/bin/$abi" "$MODULE_DIR/lib/$abi"
        cp "$out/axon_inject" "$MODULE_DIR/bin/$abi/"
        cp "$out/libaxon_init.so" "$MODULE_DIR/lib/$abi/"
        cp "$out/libaxon_adbd.so" "$MODULE_DIR/lib/$abi/"
    done
    echo "==> [axon] All targets built"
}




# Package one ZIP from a given Rust profile
package_zip() {
    local profile="$1"
    local target_subdir="debug"
    [ "$profile" = "release" ] && target_subdir="release"

    local suffix=""
    [ "$profile" = "debug" ] && suffix="-debug"

    local out_name="zeromount-${VERSION}${suffix}.zip"
    local out_path="$RELEASE_DIR/$profile/$out_name"
    local staging
    staging="$(mktemp -d)"

    echo ""
    echo "==> Packaging $profile: $out_name"

    for script in "${SCRIPTS[@]}"; do
        local src="$MODULE_DIR/$script"
        if [ ! -f "$src" ]; then
            echo "FATAL: missing $script" >&2
            rm -rf "$staging"
            exit 1
        fi
        cp "$src" "$staging/$script"
    done

    if [ ! -f "$MODULE_DIR/module.prop" ]; then
        echo "FATAL: missing module.prop" >&2
        rm -rf "$staging"
        exit 1
    fi
    cp "$MODULE_DIR/module.prop" "$staging/module.prop"

    if [ -f "$MODULE_DIR/sepolicy.rule" ]; then
        cp "$MODULE_DIR/sepolicy.rule" "$staging/sepolicy.rule"
    fi

    sed -i "s/^version=.*/version=${VERSION}/" "$staging/module.prop"
    local vcode="${VERSION#v}"
    vcode="${vcode%%-*}"
    vcode="${vcode//.}"
    sed -i "s/^versionCode=.*/versionCode=${vcode}/" "$staging/module.prop"

    local found_bins=0
    for abi in "${!ABI_TARGET[@]}"; do
        local target="${ABI_TARGET[$abi]}"
        local bin_src="$PROJECT_ROOT/target/$target/$target_subdir/zeromount"
        mkdir -p "$staging/bin/$abi"

        if [ -f "$bin_src" ]; then
            cp "$bin_src" "$staging/bin/$abi/zeromount"
            found_bins=$((found_bins + 1))
        elif [ -f "$MODULE_DIR/bin/$abi/zeromount" ]; then
            cp "$MODULE_DIR/bin/$abi/zeromount" "$staging/bin/$abi/zeromount"
            found_bins=$((found_bins + 1))
        fi

        if [ -f "$MODULE_DIR/bin/$abi/aapt" ]; then
            cp "$MODULE_DIR/bin/$abi/aapt" "$staging/bin/$abi/aapt"
        fi

        # axon prebuilt staging (binaries present when available)
        if [ -f "$MODULE_DIR/bin/$abi/axon_inject" ]; then
            cp "$MODULE_DIR/bin/$abi/axon_inject" "$staging/bin/$abi/axon_inject"
        fi
        if [ -d "$MODULE_DIR/lib/$abi" ]; then
            mkdir -p "$staging/lib/$abi"
            for so in "$MODULE_DIR/lib/$abi"/libaxon_init.so "$MODULE_DIR/lib/$abi"/libaxon_adbd.so; do
                [ -f "$so" ] && cp "$so" "$staging/lib/$abi/"
            done
        fi

    done

    if [ "$found_bins" -ne 4 ]; then
        echo "FATAL: [$profile] found $found_bins/4 binaries" >&2
        rm -rf "$staging"
        exit 1
    fi


    # WebUI
    local webroot_src=""
    if [ -d "$MODULE_DIR/webroot" ]; then
        webroot_src="$MODULE_DIR/webroot"
    elif [ -d "$PROJECT_ROOT/staging/webroot" ]; then
        webroot_src="$PROJECT_ROOT/staging/webroot"
    fi
    if [ -n "$webroot_src" ]; then
        cp -r "$webroot_src" "$staging/webroot"
    else
        echo "FATAL: webroot/ not found" >&2
        rm -rf "$staging"
        exit 1
    fi

    # Banner
    if [ -f "$MODULE_DIR/banner.png" ]; then
        cp "$MODULE_DIR/banner.png" "$staging/banner.png"
    fi

    # LKM
    if [ -d "$MODULE_DIR/lkm" ] && ls "$MODULE_DIR/lkm"/*.ko >/dev/null 2>&1; then
        mkdir -p "$staging/lkm"
        cp "$MODULE_DIR/lkm"/*.ko "$staging/lkm/"
    fi

    # Emoji font
    if [ -d "$MODULE_DIR/emoji" ]; then
        cp -r "$MODULE_DIR/emoji" "$staging/emoji"
    fi

    # META-INF
    mkdir -p "$staging/META-INF/com/google/android"
    cat > "$staging/META-INF/com/google/android/update-binary" << 'UPDATER'
#!/sbin/sh

OUTFD=/proc/self/fd/$2
ZIPFILE="$3"

ui_print() { echo -e "ui_print $1\nui_print" >> $OUTFD; }

MODPATH="${MODPATH:-/data/adb/modules/meta-zeromount}"
mkdir -p "$MODPATH"
unzip -o "$ZIPFILE" -d "$MODPATH" >&2
chmod 755 "$MODPATH"/*.sh "$MODPATH"/bin/*/zeromount 2>/dev/null || true
ui_print "ZeroMount installed via recovery"
exit 0
UPDATER
    echo "" > "$staging/META-INF/com/google/android/updater-script"

    # Verify no eliminated scripts
    local eliminated=(logging.sh susfs_integration.sh sync.sh zm-diag.sh zm-init.sh)
    for dead in "${eliminated[@]}"; do
        if [ -f "$staging/$dead" ]; then
            echo "FATAL: eliminated script $dead in staging!" >&2
            rm -rf "$staging"
            exit 1
        fi
    done

    rm -f "$out_path"
    (cd "$staging" && zip -r9 "$out_path" .)
    rm -rf "$staging"

    # Summary
    local lkm_count=0
    if [ -d "$MODULE_DIR/lkm" ]; then
        lkm_count=$(ls "$MODULE_DIR/lkm"/*.ko 2>/dev/null | wc -l)
    fi

    echo "    Output:  $out_path"
    echo "    Size:    $(du -h "$out_path" | cut -f1)"
    echo "    Bins:    $found_bins/4"
    echo "    WebUI:   present"
    echo "    LKM:     $lkm_count kernel modules"
}

# -- Main --
echo "==> ZeroMount $VERSION build pipeline"
echo ""

if [ "$BUILD" = true ]; then
    setup_toolchain

    build_rust "debug"
    build_rust "release"

    build_axon

    if [ -f "$WEBUI_DIR/package.json" ]; then
        echo "==> Building WebUI"
        (cd "$WEBUI_DIR" && npm install && npm run build)
        echo "==> WebUI built"
    else
        echo "WARN: webui/package.json not found, skipping WebUI build" >&2
    fi
fi

package_zip "debug"
package_zip "release"

echo ""
echo "==> Build complete"
echo "    Debug:   $RELEASE_DIR/debug/zeromount-${VERSION}-debug.zip"
echo "    Release: $RELEASE_DIR/release/zeromount-${VERSION}.zip"
