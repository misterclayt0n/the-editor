#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIB_NAME="libthe_editor_ffi.a"
LIB_PATH="$ROOT/target/release/$LIB_NAME"
HEADERS_PATH="$ROOT/the-ffi/include"
OUTPUT_PATH="$ROOT/the-swift/RustBridge/TheEditorFFI.xcframework"

pushd "$ROOT" >/dev/null
export MACOSX_DEPLOYMENT_TARGET="14.0"
cargo build -p the-ffi --release
rm -rf "$OUTPUT_PATH"
xcodebuild -create-xcframework \
  -library "$LIB_PATH" \
  -headers "$HEADERS_PATH" \
  -output "$OUTPUT_PATH"
popd >/dev/null

echo "Built $OUTPUT_PATH"
