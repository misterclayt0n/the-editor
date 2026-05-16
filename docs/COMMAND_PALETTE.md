# Command Palette

This document defines the target command palette and command invocation model.

## Goals

- Commands have stable internal ids.
- Commands have clean human-readable titles.
- Commands can expose short explicit aliases like `pwd`, `open`, or `o`.
- No default command syntax uses `:`.
- Keybindings are separate from command definitions.
- Commands may define first-class argument flows.
- The command palette is generic enough to become the foundation for future pickers, but picker migration is a later pass.

## Command model

A command definition uses one stable id and user-facing metadata:

```ts
editor.commands.register({
  id: commandId("workspace.showCurrentDirectory"),
  title: "Show Current Directory",
  aliases: ["pwd"],
  run(ctx) {
    // ...
  },
});
```

Fields:

```ts
interface Command {
  id: CommandId;
  title: string;
  aliases?: string[];
  category?: string;
  hidden?: boolean;
  args?: CommandArgumentSpec[];
  run(ctx, args): unknown | Promise<unknown>;
}
```

### IDs

Command ids are stable internal identifiers:

```txt
workspace.showCurrentDirectory
file.open
editor.selectNextMatch
```

They are not UI syntax and should not be things like `:pwd`.

### Titles

Titles are what the palette displays:

```txt
Show Current Directory
Open File
Select Next Match
```

The palette fuzzy-matches titles automatically. Do not add aliases like `current directory` only to improve fuzzy search.

### Aliases

Aliases are short explicit command words:

```txt
pwd
open
o
```

Aliases are displayed in the palette row:

```txt
Show Current Directory        pwd        <keybinding>
```

Default aliases should be concise. Users may define their own aliases later.

Aliases are globally unique by default. Duplicate aliases should produce a diagnostic. User configuration may intentionally override aliases later.

## Matching priority

Palette command matching priority:

1. exact alias match
2. exact title match
3. fuzzy title/alias match

Typing:

```txt
pwd
```

should make this the top result:

```txt
Show Current Directory        pwd
```

## Keybindings

Keybindings are separate from commands.

Commands define what can run:

```ts
editor.commands.register({
  id: commandId("file.open"),
  title: "Open File",
  aliases: ["open", "o"],
  run(ctx, args) {},
});
```

Keymaps bind keys to command ids:

```ts
editor.keymaps.bind({
  key: "cmd-o",
  command: commandId("file.open"),
});
```

The palette may join command metadata with active keymap data for display, but keybindings do not live on command definitions.

## Arguments

Commands can define first-class argument flows.

V1 grammar is intentionally simple:

```txt
<command-or-alias> <argument query>
```

Examples:

```txt
open App.tsx
o App.tsx
rename NewName
pwd
```

The first token resolves the command. The rest of the input is passed to that command's active argument provider.

Example:

```ts
editor.commands.register({
  id: commandId("file.open"),
  title: "Open File",
  aliases: ["open", "o"],
  args: [
    {
      name: "file",
      source: "files",
    },
  ],
  run(ctx, args) {
    // args.file
  },
});
```

Typing either of these should switch the palette from command results to file results:

```txt
open 
o 
```

The palette should then fuzzy-match files using the argument query.

## Non-goals for V1

Do not build shell grammar yet.

Out of scope:

```txt
open "some file.ts" --split right --line 20
grep "hello" --case-sensitive | open
```

No quoting rules, pipes, flags, nested subcommands, or shell-style escaping in V1.

## Palette session model

The command palette should be driven by an internal session model, not by the UI library.

Recommended internal concepts:

```ts
interface CommandPaletteSession {
  query: string;
  mode: "commands" | "argument";
  commandId?: CommandId;
  argumentIndex?: number;
  items: CommandPaletteItem[];
  selectedIndex: number;
}

interface CommandPaletteItem {
  id: string;
  title: string;
  subtitle?: string;
  alias?: string;
  keybinding?: string;
  icon?: string;
  score?: number;
  payload?: unknown;
}
```

The UI renders this model. It does not own command resolution.

## UI implementation

The first UI pass should use `cmdk` directly or use the shadcn base Command component as inspiration.

Important rule:

```txt
cmdk is a rendering/input primitive, not the command architecture.
```

Architecture flow:

```txt
CommandRegistry + argument providers
  -> palette session model
  -> cmdk/shadcn-inspired component
```

If this looks bad or fights the existing styling system, consider adopting the full shadcn setup later.

## Picker migration

Do not migrate every picker in the first pass.

Order:

1. command registry metadata: aliases + argument specs
2. command palette resolver/session model
3. new command palette UI
4. file argument provider for `open` / `o`
5. later migrate old pickers onto the same foundation

## Extension model

Extensions should be able to contribute:

- commands
- aliases
- argument providers
- keybindings referencing command ids
- custom item rendering metadata where appropriate

Core invariants:

- command ids are stable
- aliases are explicit and short
- aliases are conflict-checked
- keybindings remain separate
- argument providers are composable
- no default `:` command syntax
