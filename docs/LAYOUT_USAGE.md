# Layout Engine Usage Guide

The layout engine provides a flexible, composable way to position UI elements without hardcoded pixel positions.

## Quick Start

```rust
use crate::core::{
    graphics::Rect,
    layout::{Layout, Constraint, Alignment, center, align},
};

// Split screen into header, body, footer
let area = Rect::new(0, 0, 120, 40);
let chunks = Layout::vertical()
    .constraints(vec![
        Constraint::Length(1),       // Header: 1 line
        Constraint::Fill(1),          // Body: remaining space
        Constraint::Length(1),        // Footer: 1 line
    ])
    .split(area);

let header_area = chunks[0];
let body_area = chunks[1];
let footer_area = chunks[2];
```

## Constraints

### Fixed Size
```rust
Constraint::Length(10)  // Exactly 10 cells (lines/columns)
```

### Percentage
```rust
Constraint::Percentage(50)  // 50% of available space
```

### Fill (Flexible)
```rust
Constraint::Fill(1)  // Take remaining space with weight 1

// Multiple fills with different weights:
vec![
    Constraint::Fill(1),  // Gets 1/3 of remaining space
    Constraint::Fill(2),  // Gets 2/3 of remaining space
]
```

### Ratio
```rust
Constraint::Ratio(1, 3)  // 1/3 of available space
```

### Min/Max
```rust
Constraint::Min(10)  // At least 10 cells, grows if space available
Constraint::Max(50)  // At most 50 cells, shrinks if needed
```

## Layouts

### Horizontal Layout
```rust
let chunks = Layout::horizontal()
    .constraints(vec![
        Constraint::Percentage(30),  // Left sidebar
        Constraint::Fill(1),          // Main content
        Constraint::Percentage(20),  // Right sidebar
    ])
    .spacing(1)  // 1 cell gap between elements
    .split(area);
```

### Vertical Layout
```rust
let chunks = Layout::vertical()
    .constraints(vec![
        Constraint::Length(3),   // Top panel
        Constraint::Fill(1),      // Middle (expandable)
        Constraint::Length(2),   // Bottom panel
    ])
    .split(area);
```

### Nested Layouts
```rust
// Split vertically into header + body
let outer = Layout::vertical()
    .constraints(vec![Constraint::Length(1), Constraint::Fill(1)])
    .split(area);

// Split body horizontally into sidebar + content
let inner = Layout::horizontal()
    .constraints(vec![Constraint::Percentage(20), Constraint::Fill(1)])
    .split(outer[1]);

let header = outer[0];
let sidebar = inner[0];
let content = inner[1];
```

## Helper Functions

### Center a Widget
```rust
use crate::core::layout::center;

let container = Rect::new(0, 0, 100, 50);
let popup = center(container, 60, 20);  // 60x20 popup centered in container
```

### Align a Widget
```rust
use crate::core::layout::{align, Alignment};

// Top-right corner
let button_rect = align(container, 10, 2, Alignment::End);

// Center
let centered = align(container, 40, 10, Alignment::Center);

// Top-left
let label = align(container, 20, 1, Alignment::Start);
```

## Real-World Examples

### Editor Layout (Helix-style)
```rust
// Main editor area with statusline at bottom
let main_layout = Layout::vertical()
    .constraints(vec![
        Constraint::Fill(1),      // Editor view
        Constraint::Length(1),    // Status line
    ])
    .split(screen);

let editor_area = main_layout[0];
let statusline_area = main_layout[1];
```

### Popup Dialog
```rust
// Center a 60x20 popup
let popup_area = center(screen, 60, 20);

// Split popup into title, content, buttons
let popup_layout = Layout::vertical()
    .constraints(vec![
        Constraint::Length(1),    // Title
        Constraint::Fill(1),       // Content
        Constraint::Length(3),    // Buttons
    ])
    .split(popup_area);

let title_area = popup_layout[0];
let content_area = popup_layout[1];
let button_area = popup_layout[2];

// Horizontal button layout
let buttons = Layout::horizontal()
    .constraints(vec![
        Constraint::Fill(1),      // Spacer
        Constraint::Length(10),   // OK button
        Constraint::Length(1),    // Gap
        Constraint::Length(10),   // Cancel button
    ])
    .spacing(1)
    .split(button_area);
```

### Split View
```rust
// 50/50 horizontal split
let splits = Layout::horizontal()
    .constraints(vec![
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(editor_area);

let left_view = splits[0];
let right_view = splits[1];
```

### Picker with Preview
```rust
// Picker on left, preview on right
let picker_layout = Layout::horizontal()
    .constraints(vec![
        Constraint::Percentage(40),  // Picker list
        Constraint::Fill(1),          // Preview pane
    ])
    .spacing(1)
    .split(area);

let list_area = picker_layout[0];
let preview_area = picker_layout[1];
```

## Migration from Hardcoded Positions

### Before (Hardcoded)
```rust
let button = Button::new("Run")
    .with_rect(Rect::new(100, 1, 8, 2))  // ❌ Hardcoded position
    .visible(false);
```

### After (Layout-based)
```rust
// Define button size
let button_width = 8;
let button_height = 2;

// Position in top-right corner using alignment
let button_rect = align(
    screen,
    button_width,
    button_height,
    Alignment::End
);

let button = Button::new("Run")
    .with_rect(button_rect)  // ✅ Calculated position
    .visible(false);
```

Or with a layout:
```rust
// Top bar with buttons on the right
let top_bar = Layout::horizontal()
    .constraints(vec![
        Constraint::Fill(1),      // Empty space
        Constraint::Length(8),    // Button
    ])
    .split(Rect::new(0, 0, screen_width, 2));

let button_rect = top_bar[1];
```

## Benefits

1. **Responsive**: Layouts adapt to screen size changes
2. **Composable**: Nest layouts to create complex UIs
3. **Maintainable**: No magic numbers, clear intent
4. **Consistent**: Spacing and alignment rules enforced
5. **Flexible**: Mix fixed, percentage, and flexible sizing

## Best Practices

1. **Use Fill for main content areas** - They resize with the window
2. **Use Length for fixed-size elements** - Headers, footers, buttons
3. **Use Percentage for proportional splits** - Side-by-side views
4. **Add spacing between elements** - `.spacing(1)` for visual separation
5. **Nest layouts for complex UIs** - Split large areas into smaller chunks
6. **Use helper functions for popups** - `center()` and `align()` for floating elements

## Common Patterns

### Three-Column Layout
```rust
let columns = Layout::horizontal()
    .constraints(vec![
        Constraint::Percentage(25),  // Left sidebar
        Constraint::Fill(1),          // Main content
        Constraint::Percentage(25),  // Right sidebar
    ])
    .spacing(1)
    .split(area);
```

### Header + Body + Footer
```rust
let sections = Layout::vertical()
    .constraints(vec![
        Constraint::Length(2),   // Header
        Constraint::Fill(1),      // Body
        Constraint::Length(1),   // Footer
    ])
    .split(area);
```

### Centered Modal
```rust
// Centered 50% width, 30% height
let container_width = (area.width as u32 * 50 / 100) as u16;
let container_height = (area.height as u32 * 30 / 100) as u16;
let modal = center(area, container_width, container_height);
```
