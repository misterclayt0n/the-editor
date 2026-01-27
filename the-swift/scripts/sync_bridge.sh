#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BRIDGE_DIR="${ROOT_DIR}/the-swift/Bridge"
SWIFT_DIR="${ROOT_DIR}/the-swift/Sources/TheEditorFFIBridge"
INCLUDE_DIR="${BRIDGE_DIR}/include"
GEN_DIR="${ROOT_DIR}/the-ffi/generated"

mkdir -p "${INCLUDE_DIR}" "${SWIFT_DIR}"

cp "${GEN_DIR}/SwiftBridgeCore.swift" "${SWIFT_DIR}/SwiftBridgeCore.swift"
cp "${GEN_DIR}/the-ffi/the-ffi.swift" "${SWIFT_DIR}/the-ffi.swift"
cp "${GEN_DIR}/SwiftBridgeCore.h" "${INCLUDE_DIR}/SwiftBridgeCore.h"
cp "${GEN_DIR}/the-ffi/the-ffi.h" "${INCLUDE_DIR}/the-ffi.h"
cat <<'EOF' > "${INCLUDE_DIR}/TheEditorFFI.h"
// Umbrella header for the Swift/C bridge.
// Keep SwiftBridgeCore before the-ffi to ensure all core types are visible.
#include "SwiftBridgeCore.h"
#include "the-ffi.h"
EOF

if ! rg -q "^import TheEditorFFI" "${SWIFT_DIR}/SwiftBridgeCore.swift"; then
  printf "import TheEditorFFI\n%s" "$(cat "${SWIFT_DIR}/SwiftBridgeCore.swift")" > "${SWIFT_DIR}/SwiftBridgeCore.swift"
fi

if ! rg -q "^import TheEditorFFI" "${SWIFT_DIR}/the-ffi.swift"; then
  printf "import Foundation\nimport TheEditorFFI\n\n%s" "$(cat "${SWIFT_DIR}/the-ffi.swift")" > "${SWIFT_DIR}/the-ffi.swift"
fi

echo "swift-bridge files synced to ${SWIFT_DIR} and ${INCLUDE_DIR}"
