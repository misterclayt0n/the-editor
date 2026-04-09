#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "==> Initializing ghostty submodule..."
git submodule update --init --recursive ghostty

"$SCRIPT_DIR/ensure-ghosttykit.sh"

echo "==> Ghostty setup complete"
