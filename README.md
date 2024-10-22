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
- [ ] Git client inspired by magit
- [ ] User configurations (`~/.config/the-editor/config`)
- [x] Optimize with rope data structure (almost completed)
- [ ] Line numbers
- [ ] Buffer changing
- [ ] Window system (going to copy emacs/vim)
- [x] Auto closing?
- [ ] Multiple cursors
- [ ] Visual block mode
- [ ] Compile/Recompile commands 
- [ ] Working command mode 
- [x] Tab identation
- [ ] Minmal mouse support (scrolling, selection, moving cursor)
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
- [ ] "J" command
- [ ] Yanking and pasting
- [ ] Very basic syntax highlight - Probably going to use tree-sitter
- [ ] Fuzzy finder inspired by [telescope.nvim](https://github.com/nvim-telescope/telescope.nvim)

### Fixes
- [x] "w" and "b" motions are not working as vim's
- [x] Need to create some sort of rendering buffer to evoid flickering
- [x] Selection is not working well when encountered with emojis
- [x] Deletion at the beginning of the line acts as delete character
- [x] Vertical movement from the end of the line gets the cursor stuck at one character after the end of the line
- [x] Zooming in does not work (probably because of the whole virtual DOM thing), I guess it would be nice to implement some interaction, if possible, with zooming (there is no way for me to control zooming in the terminal)
- [x] The editor is not scrolling horizontally as I type (also probably because of the virtual DOM)
- [x] Not really a bug, but would be nice to render an empty selected character when passing by an empty line in visual mode
- [x] Ctrl-d and Ctrl-u are not really working just like vim.
- [x] Deleting last line with selection and "d" motion crashes, I also can't delete the last line using "dd"
- [x] "ci" motion not working well
- [x] Very specific panic on searching
- [x] Phantom line introduced a bug where if open a file, it keeps adding a new line

### 📚 Future Plans
- **Integrated Setup**: Automatically configure necessary tools when opening a new file type.
- **Expandable**: Provide a Rust API for users to customize and extend the editor, allowing them to build their own features.
- **Emacs-like Features**: Compile commands, file navigation systems, and more to create a seamless coding environment.

### 📞 Support
If you encounter any issues, feel free to open an [issue](https://github.com/misterclayt0n/the-editor/issues) on GitHub.

### 🌟 Acknowledgments
Inspired by the simplicity of Zed, the speed of Vim, and the versatility of Emacs.
