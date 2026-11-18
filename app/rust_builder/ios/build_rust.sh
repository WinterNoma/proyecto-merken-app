#!/bin/sh
set -eu

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
workspace="$(cd "$SRCROOT/../.." && pwd)"
export CARGO_TARGET_DIR="$SRCROOT/../build/rust/ios"
export IPHONEOS_DEPLOYMENT_TARGET

case "$PLATFORM_NAME" in
    iphoneos) target=aarch64-apple-ios ;;
    iphonesimulator) target=aarch64-apple-ios-sim ;;
    *) echo "error: Unsupported Pulsys Rust platform $PLATFORM_NAME" >&2; exit 1 ;;
esac

profile=release
set -- build --locked --manifest-path "$workspace/crates/bridge/Cargo.toml" --target "$target"
if [ "$CONFIGURATION" = Debug ]; then
    profile=debug
else
    set -- "$@" --release
fi

cargo "$@"
mkdir -p "$(dirname "$SCRIPT_OUTPUT_FILE_0")"
cp "$CARGO_TARGET_DIR/$target/$profile/libpulsys_bridge.a" "$SCRIPT_OUTPUT_FILE_0"
