# The editor
### Motivation
I love neovim, and wanted to create my own!

### Usage
1. Install [rust](https://www.rust-lang.org/)

2. Clone the repository
```zsh
git clone https://github.com/misterclayt0n/the-editor
```

3. Build the-editor
```zsh
cd the-editor
cargo build
```

4. Edit your files
```zsh
./target/debug/editor <FILE>
```

### Roadmap
- [ ] Basic vim motions
- [x] Open files
- [x] Scrolling
- [x] Write to files
- [ ] File manager (of some sort, inspired in [oil.nvim](https://github.com/stevearc/oil.nvim))
- [ ] Support for user configuration, probably something like a file `~/.config/the-editor/config`
- [ ] Use rope data structure for better optimization
