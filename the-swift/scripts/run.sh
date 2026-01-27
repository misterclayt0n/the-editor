#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XCODE_DIR="/Applications/Xcode.app/Contents/Developer"

if [ -d "${XCODE_DIR}" ]; then
  export DEVELOPER_DIR="${XCODE_DIR}"
fi

export SWIFT_MODULE_CACHE_PATH="${SWIFT_MODULE_CACHE_PATH:-/tmp/swift-module-cache}"
mkdir -p "${SWIFT_MODULE_CACHE_PATH}"
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-/tmp/clang-module-cache}"
mkdir -p "${CLANG_MODULE_CACHE_PATH}"

cd "${ROOT_DIR}"
swift run the-swift
