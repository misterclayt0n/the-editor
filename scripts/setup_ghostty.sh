#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
GHOSTTY_DIR="$PROJECT_DIR/ghostty"
SWIFT_FRAMEWORK_DIR="$PROJECT_DIR/the-swift/Frameworks"
TARGET_XCFRAMEWORK="$SWIFT_FRAMEWORK_DIR/GhosttyKit.xcframework"

cd "$PROJECT_DIR"

if ! git -C "$GHOSTTY_DIR" rev-parse --verify HEAD >/dev/null 2>&1; then
  echo "error: ghostty submodule not found. Run: git submodule update --init --recursive"
  exit 1
fi

if ! command -v zig >/dev/null 2>&1; then
  echo "error: zig is required. Install with: brew install zig"
  exit 1
fi

if ! xcrun -sdk macosx --find metal >/dev/null 2>&1; then
  echo "error: Metal toolchain is not installed."
  echo "run: sudo xcodebuild -downloadComponent MetalToolchain"
  exit 1
fi

mkdir -p "$SWIFT_FRAMEWORK_DIR"

GHOSTTY_SHA="$(git -C "$GHOSTTY_DIR" rev-parse HEAD)"
CACHE_ROOT="${THE_EDITOR_GHOSTTY_CACHE_DIR:-$HOME/.cache/the-editor/ghosttykit}"
CACHE_DIR="$CACHE_ROOT/$GHOSTTY_SHA"
CACHE_XCFRAMEWORK="$CACHE_DIR/GhosttyKit.xcframework"
LOCAL_XCFRAMEWORK="$GHOSTTY_DIR/macos/GhosttyKit.xcframework"
ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-$HOME/.cache/the-editor/zig-global-cache}"
ZIG_LOCAL_CACHE_DIR="${ZIG_LOCAL_CACHE_DIR:-.zig-cache}"
BUILD_RETRIES="${THE_EDITOR_GHOSTTY_BUILD_RETRIES:-5}"

mkdir -p "$CACHE_ROOT"
mkdir -p "$ZIG_GLOBAL_CACHE_DIR"

if [[ ! -d "$CACHE_XCFRAMEWORK" ]]; then
  echo "==> building GhosttyKit for commit $GHOSTTY_SHA"
  build_ok=0
  for attempt in $(seq 1 "$BUILD_RETRIES"); do
    if (
      cd "$GHOSTTY_DIR"
      DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}" \
        ZIG_GLOBAL_CACHE_DIR="$ZIG_GLOBAL_CACHE_DIR" \
        ZIG_LOCAL_CACHE_DIR="$ZIG_LOCAL_CACHE_DIR" \
        zig build -Demit-xcframework=true -Demit-macos-app=false -Dxcframework-target=native -Doptimize=ReleaseFast
    ); then
      build_ok=1
      break
    fi
    if [[ "$attempt" -lt "$BUILD_RETRIES" ]]; then
      sleep_seconds=$((attempt * 2))
      echo "warning: GhosttyKit build failed (attempt $attempt/$BUILD_RETRIES); retrying in ${sleep_seconds}s..."
      sleep "$sleep_seconds"
    fi
  done
  if [[ "$build_ok" -ne 1 ]]; then
    echo "error: failed to build GhosttyKit after $BUILD_RETRIES attempts."
    exit 1
  fi

  if [[ ! -d "$LOCAL_XCFRAMEWORK" ]]; then
    echo "error: GhosttyKit.xcframework not produced at $LOCAL_XCFRAMEWORK"
    exit 1
  fi

  mkdir -p "$CACHE_DIR"
  rm -rf "$CACHE_XCFRAMEWORK"
  cp -R "$LOCAL_XCFRAMEWORK" "$CACHE_XCFRAMEWORK"
fi

rm -rf "$TARGET_XCFRAMEWORK"
cp -R "$CACHE_XCFRAMEWORK" "$TARGET_XCFRAMEWORK"
echo "==> installed $TARGET_XCFRAMEWORK"
