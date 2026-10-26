#!/usr/bin/env bash
# Build an experimental Android APK for Valocraft.
#
# This is intentionally a local/CI helper, not part of the desktop release
# workflow.  It needs a host JDK, the Android SDK command-line tools, an NDK,
# CMake, Ninja, and cargo-apk (0.10.x).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
API="${ANDROID_API_LEVEL:-26}"
ABI="${ANDROID_ABI:-arm64-v8a}"
TARGET="${ANDROID_RUST_TARGET:-aarch64-linux-android}"
TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64"
CLANG="$TOOLCHAIN/bin/${TARGET}${API}-clang"
CLANGXX="$TOOLCHAIN/bin/${TARGET}${API}-clang++"
SHADERC_BUILD="$ROOT/target-android/shaderc"
SHADERC_LIB="$SHADERC_BUILD/lib"

if [[ -z "$NDK" || ! -d "$NDK" ]]; then
    echo "ANDROID_NDK_HOME must point at an Android NDK (r27c is recommended)." >&2
    exit 1
fi
if [[ -z "$SDK" || ! -d "$SDK" ]]; then
    echo "ANDROID_HOME must point at an Android SDK." >&2
    exit 1
fi
if [[ ! -x "$CLANG" ]]; then
    echo "Missing Android compiler: $CLANG" >&2
    exit 1
fi

# LFS pointers are valid text files but not usable game data.  Check both a
# canary and a real texture so a pointer-only checkout cannot silently produce
# an APK that launches without assets.
if ! head -n 1 "$ROOT/assets/common/canary.canary" | grep -qx "VELOREN_CANARY_MAGIC"; then
    echo "Android assets are missing or are Git LFS pointers." >&2
    echo "Run 'git lfs pull' in the Android worktree before building." >&2
    exit 1
fi
if [[ ! -s "$ROOT/assets/voxygen/texture/noise.png" ]] || \
   [[ "$(stat -c%s "$ROOT/assets/voxygen/texture/noise.png")" -lt 1000 ]]; then
    echo "Android assets/voxygen/texture/noise.png is not real game data." >&2
    echo "Run 'git lfs pull' in the Android worktree before building." >&2
    exit 1
fi
if ! command -v cargo-apk >/dev/null; then
    echo "Install cargo-apk first: cargo +stable install cargo-apk --version 0.10.0 --locked" >&2
    exit 1
fi

# Download the crate sources before looking for shaderc-sys.  A fresh CI
# checkout has no registry sources yet, and cargo-apk invokes the shaderc build
# only after this script has already prepared the static library.
cargo fetch --locked

# shaderc-sys can use a prebuilt static library through this variable.  Build
# it ourselves with the NDK toolchain instead of letting shaderc-sys fall back
# to a host build (the crate's build script does not configure CMake for
# cross-compilation on its own).
if [[ ! -f "$SHADERC_LIB/libshaderc_combined.a" ]]; then
    SHADERC_SRC="$(find "$HOME/.cargo/registry/src" -maxdepth 4 -type d \
        -path '*shaderc-sys-0.10.1/build' | head -n 1)"
    if [[ -z "$SHADERC_SRC" ]]; then
        echo "Could not find shaderc-sys sources; run cargo fetch first." >&2
        exit 1
    fi
    mkdir -p "$SHADERC_BUILD"
    cmake -S "$SHADERC_SRC" -B "$SHADERC_BUILD/build" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_TOOLCHAIN_FILE="$NDK/build/cmake/android.toolchain.cmake" \
        -DANDROID_ABI="$ABI" \
        -DANDROID_PLATFORM="android-$API" \
        -DANDROID_STL=c++_static \
        -DCMAKE_INSTALL_LIBDIR=lib \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DENABLE_SPVREMAPPER=OFF \
        -DENABLE_GLSLANG_BINARIES=OFF \
        -DSHADERC_SKIP_TESTS=ON \
        -DSPIRV_SKIP_EXECUTABLES=ON \
        -DSPIRV_SKIP_TESTS=ON \
        -DSPIRV_WERROR=OFF
    cmake --build "$SHADERC_BUILD/build" --target shaderc_combined -j"${JOBS:-4}"
    mkdir -p "$SHADERC_LIB"
    find "$SHADERC_BUILD/build" -name 'libshaderc_combined.a' -exec cp {} "$SHADERC_LIB/" \;
    if [[ ! -f "$SHADERC_LIB/libshaderc_combined.a" ]]; then
        echo "shaderc build did not produce libshaderc_combined.a" >&2
        exit 1
    fi
fi

export ANDROID_NDK_HOME="$NDK"
export ANDROID_NDK_ROOT="$NDK"
export ANDROID_HOME="$SDK"
export SHADERC_LIB_DIR="$SHADERC_LIB"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CLANG"
# shaderc_combined is a C++ static library.  Rust does not otherwise pull in
# the NDK C++ runtime, so add it only for the Android target (not host build
# scripts).
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS:-} -C link-arg=-lc++_static -C link-arg=-lc++abi"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-android}"
export ANDROID_SDK_ROOT="$SDK"
export PKG_CONFIG_ALLOW_CROSS=1

if [[ -z "${CARGO_APK_RELEASE_KEYSTORE:-}" && -f "$HOME/.android/debug.keystore" ]]; then
    # A debug key is enough for local sideloading, not for a store release.
    export CARGO_APK_RELEASE_KEYSTORE="$HOME/.android/debug.keystore"
    export CARGO_APK_RELEASE_KEYSTORE_PASSWORD="${CARGO_APK_RELEASE_KEYSTORE_PASSWORD:-android}"
fi
if [[ -z "${CARGO_APK_RELEASE_KEYSTORE:-}" ]]; then
    echo "Set CARGO_APK_RELEASE_KEYSTORE and CARGO_APK_RELEASE_KEYSTORE_PASSWORD for a release APK." >&2
    echo "For local testing: keytool -genkeypair -keystore ~/.android/debug.keystore -storepass android -keypass android -alias androiddebugkey -dname 'CN=Android Debug,O=Android,C=US' -keyalg RSA -validity 10000" >&2
    exit 1
fi

# cargo-apk builds a cdylib (the NativeActivity entry point) and packages the
# assets configured in voxygen/Cargo.toml.  Pass the target-specific CC/AR
# variables with hyphens through `env`: bash cannot export names containing
# hyphens directly.
cd "$ROOT"
env "CC_${TARGET}=$CLANG" "CXX_${TARGET}=$CLANGXX" \
    "AR_${TARGET}=$TOOLCHAIN/bin/llvm-ar" \
    "RANLIB_${TARGET}=$TOOLCHAIN/bin/llvm-ranlib" \
    cargo apk build \
    --release \
    --lib \
    --package veloren-voxygen \
    --target "$TARGET" \
    --no-default-features \
    --features singleplayer,simd

echo
signed_apk=$(find "$CARGO_TARGET_DIR" -path '*/release/apk/Valocraft.apk' -print -quit)
echo "APK: ${signed_apk:-$CARGO_TARGET_DIR/release/apk/Valocraft.apk}"
