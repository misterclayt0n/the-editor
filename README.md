# 📝 The Editor

### 🚀 Motivation
I love Neovim and wanted to create my own editor that combines the simplicity of Zed, the speed and expandability of Vim, and the rich features of Emacs. Imagine an editor that configures itself based on your project: open a Rust file, and it asks if you want to set up the necessary LSP, Tree-sitter, formatter, and more. The goal is to create a customizable powerhouse where users can code their own editor using a Rust API, offering deep flexibility and advanced navigation, compile commands, and other Emacs-like functionalities.

### ✨ Features
- **Blazing Fast**: Lightning speed similar to Vim.
- **User Configurations**: Coming soon—configure everything, your way.
- **Emacs-Inspired**: Advanced navigation and compile commands.
- **Easy Setup**: Opens files directly, suggests necessary packages, and more.
  
### 🛠️ Usage
1. **Install [Rust](https://www.rust-lang.org/)**: Make sure you have Rust installed on your machine.

2. **Clone the repository**
   ```bash
   git clone https://github.com/misterclayt0n/the-editor
   ```

3. **Build The Editor**
   ```bash
   cd the-editor
   cargo build
   ```

4. **Start Editing**
   ```bash
   ./target/debug/editor <FILE>
   ```

### 📅 Roadmap
- [x] Basic vim motions
- [x] Open files
- [x] Scrolling
- [x] Write to files
- [ ] File manager inspired by [oil.nvim](https://github.com/stevearc/oil.nvim)
- [ ] User configurations (`~/.config/the-editor/config`)
- [x] Optimize with rope data structure (almost completed)
- [ ] Line numbers
- [ ] Buffer changing
- [ ] Window system (going to copy emacs/vim)
- [ ] Auto closing?
- [ ] Multiple cursors 
- [ ] Visual block mode
- [ ] Compile/Recompile commands
- [ ] Working command mode 
- [ ] Tab identation
- [ ] CommandBar mode with basic operations: 
    - [ ] Saving
    - [ ] Subtitution
- [ ] Advanced vim motions: 
    - [x] "I" and "A"
	- [x] "o" and "O"
	- [x] "s" and "x"
    - [x] "C" and "D" 
	- [x] "cc"
    - [ ] Operator + number + direction
    - [ ] Operator + inside/outside
	- [ ] "r" motion
- [ ] Yanking and pasting
- [ ] Very basic syntax highlight - Probably going to use tree-sitter
- [ ] Fuzzy finder inspired by [telescope.nvim](https://github.com/nvim-telescope/telescope.nvim)

### Fixes
- [ ] "w" and "b" motions are not working as vim's
- [ ] Need to create some sort of rendering buffer to evoid flickering
- [ ] Selection is not working well when encountered with emojis

### 📚 Future Plans
- **Integrated Setup**: Automatically configure necessary tools when opening a new file type.
- **Expandable**: Provide a Rust API for users to customize and extend the editor, allowing them to build their own features.
- **Emacs-like Features**: Compile commands, file navigation systems, and more to create a seamless coding environment.

### 📞 Support
If you encounter any issues, feel free to open an [issue](https://github.com/misterclayt0n/the-editor/issues) on GitHub.

### 🌟 Acknowledgments
Inspired by the simplicity of Zed, the speed of Vim, and the versatility of Emacs.
