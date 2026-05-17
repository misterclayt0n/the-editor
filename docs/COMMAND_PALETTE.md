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
  palette?: "topLevel" | "command" | "hidden";
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

## Palette visibility

Not every registered command is a top-level palette command. Keymap targets and low-level editing primitives must remain dispatchable without becoming default palette noise.

Visibility levels:

```ts
palette: "topLevel" // shown when the palette opens
palette: "command"  // hidden from the default list, available through `command <id>`
palette: "hidden"   // internal plumbing; not shown in command browsing
```

Default visibility is `"command"` for normal commands and `"hidden"` for legacy `hidden: true` commands.

Top-level commands should be curated actions such as:

```txt
open
write
theme
reload
pwd
cd
vsplit
hsplit
command
```

Raw/keymap commands remain available explicitly:

```txt
command append_mode
command insert_mode
command edit.deleteBackward
```

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

Argument collection is a nested picker flow, not an inline command language. The command palette has one search input for the current frame. Selecting a command that requires an argument pushes a new frame, clears the search query, and shows suggestions from the command's argument provider.

Example flow:

```txt
Command frame
  query: theme
  selected item: Select Theme
  Enter

Argument frame
  title: Select Theme
  query: gruv
  selected item: Gruvbox Dark
  Enter
```

The final dispatch is still a normal command dispatch:

```ts
{ id: commandId("workbench.selectTheme"), args: { theme: "gruvbox_dark" } }
```

Example command definition:

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

Selecting `Open File` enters a `Files` picker. Typing in that frame fuzzy-matches files. Pressing Backspace with an empty argument query, or using the UI back affordance, returns to the previous command frame.

Aliases remain searchable metadata for command discovery, but aliases are not an inline invocation language. Inputs such as `open App.tsx`, `theme gruvbox`, or `cd ~/code/foo` are not parsed specially by the palette.

## Non-goals for V1

Do not build shell grammar.

Out of scope:

```txt
open App.tsx
o App.tsx
open "some file.ts" --split right --line 20
grep "hello" --case-sensitive | open
```

No inline command invocation, quoting rules, pipes, flags, nested subcommands, shell-style escaping, or compatibility shims.

## Global providers and everything palette

The palette is one interaction surface backed by explicit providers. Command browsing is the built-in provider. Other domains, such as themes, files, symbols, diagnostics, tasks, and extension features, opt into global search by registering a command palette provider.

Global provider participation is explicit. Argument providers do not automatically leak into global results. If a domain should appear in the everything palette, register a palette provider for it.

Empty global palette results stay calm and command-oriented. Extra global providers are queried only after the user enters a non-empty query. Scoped entrypoints may choose different initial providers later.

Provider results are normal palette items. They can execute commands directly or enter nested provider frames:

```ts
editor.palette.registerProvider({
  id: "themes",
  title: "Themes",
  priority: -10,
  provide(ctx) {
    return themesMatching(ctx.query).map((theme) => ({
      id: `theme:${theme.id}`,
      title: theme.title,
      group: "Themes",
      icon: { kind: "lucide", name: "palette" },
      command: { id: commandId("theme.select"), args: { id: theme.id } },
    }));
  },
});
```

Nested provider frames use explicit frame metadata:

```ts
{
  id: "project:the-editor",
  title: "the-editor",
  frame: {
    title: "the-editor",
    provider: "project.actions",
    placeholder: "Search project actions...",
    args: { projectId: "the-editor" },
  },
}
```

Provider visibility controls where providers participate. Omitted visibility means `global`. Developer/internal workbench plumbing should use `developer` or `hidden` and must not appear in the normal everything palette.

Provider API is a single context object:

```ts
interface CommandPaletteProviderContext<TArgs = unknown> {
  editor: EditorHost;
  query: string;
  frame?: CommandPaletteFrameTarget<TArgs>;
}

type CommandPaletteProviderVisibility = "global" | "developer" | "hidden";

interface CommandPaletteProvider<TArgs = unknown> {
  id: string;
  title?: string;
  priority?: number;
  visibility?: CommandPaletteProviderVisibility;
  provide(ctx: CommandPaletteProviderContext<TArgs>): CommandPaletteItem[] | Promise<CommandPaletteItem[]>;
}
```

## Palette session model

The command palette should be driven by an internal session model, not by the UI library.

Recommended internal concepts:

```ts
interface CommandPaletteSession {
  query: string;
  mode: "commands" | "argument";
  commandId?: CommandId;
  commandTitle?: string;
  argumentIndex?: number;
  argumentName?: string;
  placeholder?: string;
  items: CommandPaletteItem[];
  canGoBack?: boolean;
  stack: CommandPaletteFrame[];
}

interface CommandPaletteFrame {
  query: string;
  mode: "commands" | "argument";
  commandId?: CommandId;
  commandTitle?: string;
  argumentIndex?: number;
  argumentName?: string;
  placeholder?: string;
  providerFrame?: CommandPaletteFrameTarget;
}

interface CommandPaletteFrameTarget<TArgs = unknown> {
  title: string;
  provider: string;
  placeholder?: string;
  query?: string;
  args?: TArgs;
}

type CommandPaletteIcon =
  | { kind: "lucide"; name: string }
  | { kind: "file"; path: string; entryKind?: "file" | "directory" }
  | { kind: "text"; value: string }
  | { kind: "svg"; source: string; viewBox?: string }
  | { kind: "uri"; src: string };

interface CommandPaletteItem {
  id: string;
  title: string;
  subtitle?: string;
  alias?: string;
  keybinding?: string;
  icon?: CommandPaletteIcon;
  command?: CommandDispatch;
  frame?: CommandPaletteFrameTarget;
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
