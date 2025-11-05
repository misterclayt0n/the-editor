# Installing the-editor

This guide covers installation methods for the-editor alpha releases.

## Pre-built Binary (Linux x86_64)

### Quick Start

1. Download the latest release tarball from the [releases page](https://github.com/YourUsername/the-editor/releases)
2. Extract the archive:
   ```bash
   tar xzf the-editor-v0.1.0-alpha.1-linux-x86_64.tar.gz
   cd the-editor
   ```
3. Run the editor:
   ```bash
   ./the-editor-launch <file>
   ```

The launcher script automatically finds the runtime directory and starts the editor.

### System-wide Installation

To install the-editor system-wide (available via `/usr/bin/the-editor`):

```bash
# Extract the tarball first
tar xzf the-editor-*.tar.gz
cd the-editor

# Install to /usr/lib/the-editor
sudo mkdir -p /usr/lib/the-editor
sudo cp -r runtime /usr/lib/the-editor/
sudo cp the-editor /usr/lib/the-editor/

# Create launcher script in /usr/bin
sudo tee /usr/bin/the-editor >/dev/null <<'EOF'
#!/usr/bin/env sh
THE_EDITOR_RUNTIME=/usr/lib/the-editor/runtime exec /usr/lib/the-editor/the-editor "$@"
EOF

sudo chmod +x /usr/bin/the-editor
```

Now you can run `the-editor` from anywhere.

### User-local Installation

To install for your user only (available via `~/.local/bin/the-editor`):

```bash
# Extract the tarball first
tar xzf the-editor-*.tar.gz
cd the-editor

# Create directories
mkdir -p ~/.local/lib/the-editor
mkdir -p ~/.local/bin

# Install binary and runtime
cp -r runtime ~/.local/lib/the-editor/
cp the-editor ~/.local/lib/the-editor/

# Create launcher script
cat > ~/.local/bin/the-editor <<'EOF'
#!/usr/bin/env sh
THE_EDITOR_RUNTIME="$HOME/.local/lib/the-editor/runtime" exec "$HOME/.local/lib/the-editor/the-editor" "$@"
EOF

chmod +x ~/.local/bin/the-editor
```

Make sure `~/.local/bin` is in your `$PATH`:
```bash
# Add to your ~/.bashrc or ~/.zshrc
export PATH="$HOME/.local/bin:$PATH"
```

## Runtime Directory

the-editor searches for runtime files (themes, tree-sitter queries, language configs) in this order:

1. **`~/.config/the-editor/runtime/`** - User overrides (highest priority)
2. **`$THE_EDITOR_RUNTIME`** - Environment variable override
3. **Compile-time path** - Set during build (e.g., `/usr/lib/the-editor/runtime`)
4. **`./runtime`** - Sibling to the binary (lowest priority)

### Customizing Themes and Configs

To customize themes or language configurations:

```bash
# Copy runtime to config directory
mkdir -p ~/.config/the-editor
cp -r runtime ~/.config/the-editor/

# Now edit files in ~/.config/the-editor/runtime/
# Your changes will override the system runtime files
```

## Building from Source

### Prerequisites

- **Rust 1.83+** (install via [rustup](https://rustup.rs/))
- **Zig 0.15.1+** (for terminal wrapper)
- **System libraries** (Linux):
  - libxcb, libxkbcommon, libwayland
  - libvulkan, libGL
  - Development headers for the above

### Build Steps

```bash
# Clone the repository
git clone https://github.com/YourUsername/the-editor.git
cd the-editor

# Build Zig wrapper (for terminal support)
cd the-terminal
zig build
cd ..

# Build the editor
cargo build --release --features unicode-lines

# Binary will be at: target/release/the-editor
```

### Nix Users

If you have Nix with flakes enabled:

```bash
# Run directly
nix run github:YourUsername/the-editor

# Enter development shell
nix develop

# Build
nix build
```

The Nix flake handles all dependencies automatically, including Zig and Ghostty.

## Verifying Downloads

Each release includes SHA256 checksums. Verify your download:

```bash
# Download both the tarball and .sha256 file
wget https://github.com/YourUsername/the-editor/releases/download/v0.1.0-alpha.1/the-editor-v0.1.0-alpha.1-linux-x86_64.tar.gz
wget https://github.com/YourUsername/the-editor/releases/download/v0.1.0-alpha.1/the-editor-v0.1.0-alpha.1-linux-x86_64.tar.gz.sha256

# Verify checksum
sha256sum -c the-editor-v0.1.0-alpha.1-linux-x86_64.tar.gz.sha256
```

## Troubleshooting

### Runtime directory not found

If you see errors about missing runtime files:

1. Check that `runtime/` directory exists alongside the binary
2. Set `THE_EDITOR_RUNTIME` environment variable:
   ```bash
   export THE_EDITOR_RUNTIME=/path/to/the-editor/runtime
   ```
3. Or copy `runtime/` to `~/.config/the-editor/runtime/`

### Missing system libraries

On Linux, if the editor fails to start with library errors:

```bash
# Ubuntu/Debian
sudo apt-get install libxcb1 libxkbcommon0 libwayland-client0 libvulkan1 libgl1

# Fedora
sudo dnf install libxcb libxkbcommon wayland vulkan-loader mesa-libGL

# Arch
sudo pacman -S libxcb libxkbcommon wayland vulkan-icd-loader mesa
```

### Terminal not working

The integrated terminal requires the vendored Ghostty VT library (`libghostty-vt.so`). This should be bundled with the release, but if you encounter issues:

1. Make sure you extracted the full tarball
2. Check that `the-terminal/vendored/linux-x86_64/libghostty-vt.so` exists
3. Report an issue with your distribution details

## Getting Help

- **Issues**: https://github.com/YourUsername/the-editor/issues
- **Discussions**: https://github.com/YourUsername/the-editor/discussions

## Alpha Release Notes

This is an **alpha release** intended for early testing and feedback. Expect:

- Bugs and rough edges
- Missing features
- Breaking changes between releases
- Limited platform support (Linux x86_64 only for now)

Your feedback is valuable! Please report issues and suggest improvements.
