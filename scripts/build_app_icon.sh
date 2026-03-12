#!/bin/zsh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SOURCE="$ROOT/the-swift/App/TheEditor.icon"

echo "The macOS app icon is now sourced from:"
echo "  $ICON_SOURCE"
echo
echo "Xcode compiles this .icon bundle directly. No raster generation step is required."
