# Architecture
the-editor/
├── Cargo.toml            # configure workspaces
├── crates/
│   ├── editor/           # core of the editor
│   ├── renderer/         # terminal renderer
│   ├── events/           # event system
│   ├── text_engine/      # text engine - rope data structure
│   ├── language_support/ # LSP && tree-sitter
│   ├── plugins/          # plugin system
│   ├── file_manager/     # file manager
│   ├── git_client/       # git client
│   ├── commands/         # commands and possibly macros
│   └── utils/            # shared utilities
├── plugins/
│   └── example_plugin/   # plugin example
└── src/
    └── main.rs           # main entrypoint

- editor: global state of the editor, includes buffers, windows and modes.
- renderer: handle efficient rendering in the terminal.
- events: capture and dispatch events from the user.
- text_engine: expand the rope data structure for efficient text manipulation (probably something like `Line`).
- language_support: integrate LSP and tree-sitter to provide language features.
- plugins: manage loading and execution of plugins.
- file_manager: additional features for managing files.
- git_client: git client inspired by Magit..
- commands: interpret and handle commands from the user.
- utils: functions and types shared between crates.
