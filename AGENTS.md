# AGENTS.md

## Local setup and validation

This repo uses `pnpm`, TypeScript, Effect, Turbo, and **jj**.

Use **jj**, not git, for version-control operations. Examples:

```txt
jj status --no-pager
jj diff --no-pager
jj log --no-pager
jj commit -m "message" --no-pager
```

Validation commands:

```txt
pnpm install
pnpm -s build
pnpm -s typecheck
pnpm -s test
```

Current intentional package spine:

```txt
@the-editor/kernel
@the-editor/extensions
@the-editor/commands
```

## North star

The Editor is a programmable editor kernel plus packages.

The goal is not a fixed editor with a plugin API. The goal is a small local operating system for editing: a reloadable kernel, package-owned contributions, deep host modules, and an editor whose core behavior can be replaced and extended.

The intended outcome:

- extensible to the core
- extremely fast local editing path
- simple direct ownership
- package-driven product behavior
- runtime package reload
- global/contextual/modal keymaps
- native Tree-sitter syntax eventually
- cross-platform desktop host eventually
- arbitrary programmable tools, panes, workflows, and configuration-as-code

The default product should eventually be just one curated package set. User config and workspace config should also be packages.

## Effect guidance

Before making non-trivial code changes, inspect Effect itself to match the style and structure of a serious Effect codebase.

If `/tmp/effect` does not exist, clone it there:

```txt
test -d /tmp/effect || git clone https://github.com/Effect-TS/effect /tmp/effect
```

Then inspect relevant source/tests before implementing:

```txt
find /tmp/effect/packages -maxdepth 3 -type f | head
```

Use Effect for lifecycle, scoped resources, typed failures, package activation, registries, services, and background work. Do not use Effect as ceremony in tiny synchronous hot-path logic when plain TypeScript is clearer.

## Coding guidance

- Prefer small public interfaces with deep behavior behind them.
- Prefer synchronous direct TypeScript in hot-path modules.
- Use Effect for lifecycle, package activation, background work, scoped resources, typed errors, and services.
- Tests should target module seams, not implementation details.
- Do not introduce a seam unless there are real reasons for variation.
- Keep package contributions ownership-aware so reload can cleanly remove them.
- One kind of truth should have one owner.
- React, when present, should render and dispatch; it should not own editor truth.
- Electron, when present, should provide privileged OS capabilities; it should not be the editor brain.
- Avoid pass-through abstractions, compatibility shims, fake entities, and global mutable bags.

## Hot path rule

Typing, cursor movement, selection, scrolling, and paint should stay local and synchronous.

A keystroke should be roughly:

```txt
keydown -> keymap command -> synchronous transaction -> paint visible rows
```

Background systems may produce updates. They must not gate input.
