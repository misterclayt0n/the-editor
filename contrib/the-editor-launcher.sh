#!/usr/bin/env sh
# Launcher script for the-editor that sets runtime directory
# This script finds the runtime directory relative to the script location
# and ensures the-editor can find its runtime files when distributed as a tarball

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Set runtime directory to be sibling to this script
# (Assumes both launcher and runtime/ are in the same directory after extraction)
THE_EDITOR_RUNTIME="$SCRIPT_DIR/runtime" exec "$SCRIPT_DIR/the-editor" "$@"
