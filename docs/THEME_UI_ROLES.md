# UI Theme Roles

This doc lists semantic `UiStyle.role` values used by core UI components.

## Scope resolution

Roles are expanded into theme scopes like:

- `ui.{role}.{component}.{state}.{prop}`
- `ui.{role}.{component}.{state}`
- `ui.{role}.{component}.{prop}`
- `ui.{role}.{component}`
- `ui.{role}.{state}.{prop}`
- `ui.{role}.{state}`
- `ui.{role}.{prop}`
- `ui.{role}`

Components map to: `panel`, `container`, `text`, `list`, `input`, `divider`, `tooltip`, `status_bar`.
States map to: `focused`, `selected`, `hovered`, `disabled`.
Props map to: `fg`, `bg`, `border`, `accent`.

## Roles in use

- `command_palette`
  Applied to the command palette panel, container, input, and list.
  Example scopes: `ui.command_palette.panel.bg`, `ui.command_palette.input.fg`, `ui.command_palette.list.selected.bg`.

- `command_palette.help`
  Applied to the command palette help panel and help text.
  Example scopes: `ui.command_palette.help.panel.bg`, `ui.command_palette.help.text.fg`.
