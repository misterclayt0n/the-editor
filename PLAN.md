# Smooth Scrolling Implementation Plan

## Overview

Implement raddebugger-style smooth scrolling using exponential decay animation for all scroll operations (page up/down, half page, mouse wheel, etc.).

## Current State

1. **Mouse wheel scrolling** already uses smooth animation via `pending_scroll_lines/cols` in `App`, animated in `animate_scroll()` using linear interpolation (`step = remaining * lerp_factor`)

2. **Keyboard scroll commands** (`page_up`, `page_down`, `half_page_up`, `half_page_down`) call `scroll()` directly which immediately sets `view_offset` - no animation

3. **ViewPosition** stores discrete values: `anchor` (char index), `horizontal_offset`, `vertical_offset` - all integers

## Raddebugger Approach

```c
// Rate formula (frame-rate independent):
rate = 1 - 2^(-60 * dt)

// Each frame:
view_off += rate * (view_off_target - view_off)

// Snap when close (within 2 pixels):
if abs(view_off - view_off_target) < 2:
    view_off = view_off_target
```

Key concepts:
- **Two values**: `view_off` (current animated) and `view_off_target` (target)
- **Exponential decay**: Same formula as split animations
- **Frame-rate independent**: Uses `dt` to scale animation rate

## Implementation Plan

### Step 1: Add Scroll Animation State to Document

In `document.rs`, add per-view scroll animation tracking:

```rust
// Per-view scroll animation state
pub struct ScrollAnimation {
    pub target_vertical: f32,    // Target vertical offset (in lines, fractional)
    pub current_vertical: f32,   // Current animated vertical offset
    pub target_horizontal: f32,  // Target horizontal offset (in columns)
    pub current_horizontal: f32, // Current animated horizontal offset
}
```

Store in `Document` alongside existing `view_offsets: HashMap<ViewId, ViewPosition>`:
```rust
scroll_animations: HashMap<ViewId, ScrollAnimation>
```

### Step 2: Modify Scroll Commands to Set Target

Change `page_up`, `page_down`, `half_page_up`, `half_page_down` (and similar) in `commands.rs`:

**Before:**
```rust
pub fn page_down(cx: &mut Context) {
    let view = view!(cx.editor);
    let offset = view.inner_height();
    scroll(cx, offset, Direction::Forward, false);  // Immediate scroll
}
```

**After:**
```rust
pub fn page_down(cx: &mut Context) {
    let view = view!(cx.editor);
    let offset = view.inner_height();
    scroll_animated(cx, offset as f32, Direction::Forward, false);  // Animated scroll
}
```

New `scroll_animated()` function sets the target:
```rust
pub fn scroll_animated(cx: &mut Context, offset: f32, direction: Direction, sync_cursor: bool) {
    let (view, doc) = current!(cx.editor);
    let delta = match direction {
        Direction::Forward => offset,
        Direction::Backward => -offset,
    };
    doc.add_scroll_target(view.id, delta, 0.0);  // Add to vertical target

    if sync_cursor {
        // Also move cursor by same amount (immediate)
        // ...existing cursor sync logic...
    }
}
```

### Step 3: Update Animation Loop in Application

In `application.rs`, modify `animate_scroll()` to use exponential decay:

```rust
fn animate_scroll(&mut self, dt: f32) {
    // Exponential decay rate (same as split animations)
    let rate = 1.0 - 2.0_f32.powf(-60.0 * dt);

    for (view_id, _) in self.editor.tree.views() {
        let doc = match self.editor.documents.get_mut(&view.doc) {
            Some(d) => d,
            None => continue,
        };

        if let Some(anim) = doc.scroll_animation_mut(view_id) {
            // Animate vertical
            anim.current_vertical += rate * (anim.target_vertical - anim.current_vertical);
            if (anim.current_vertical - anim.target_vertical).abs() < 0.5 {
                anim.current_vertical = anim.target_vertical;
            }

            // Animate horizontal
            anim.current_horizontal += rate * (anim.target_horizontal - anim.current_horizontal);
            if (anim.current_horizontal - anim.target_horizontal).abs() < 0.5 {
                anim.current_horizontal = anim.target_horizontal;
            }

            // Apply to view offset (convert fractional to discrete + remainder)
            let view_offset = ViewPosition {
                anchor: ...,  // Calculate from current_vertical
                vertical_offset: anim.current_vertical.floor() as usize,
                horizontal_offset: anim.current_horizontal.floor() as usize,
            };
            doc.set_view_offset(view_id, view_offset);
        }
    }
}
```

### Step 4: Integrate with Rendering

The render loop already uses `doc.view_offset(view_id)` which will now return the animated position. For sub-line smoothness, we can use the fractional part of `current_vertical` to offset the rendering by partial pixels.

In `editor_view.rs` render loop:
```rust
// Get scroll animation for sub-pixel offset
let scroll_frac = doc.scroll_animation(view_id)
    .map(|a| a.current_vertical.fract())
    .unwrap_or(0.0);

// Apply sub-pixel offset to base_y
let base_y = view_offset_y + VIEW_PADDING_TOP - (scroll_frac * cell_height);
```

### Step 5: Handle Mouse Wheel

Replace current `pending_scroll_lines/cols` system with the new animation system:

```rust
// In handle_scroll_delta:
let d_lines = -y * config_lines;
doc.add_scroll_target(view_id, d_lines, 0.0);
```

### Step 6: Check for Active Animations

In `wants_redraw()`, check if any scroll animations are active:

```rust
fn wants_redraw(&self) -> bool {
    // ... existing checks ...

    // Check for active scroll animations
    for (view, _) in self.editor.tree.views() {
        if let Some(doc) = self.editor.documents.get(&view.doc) {
            if doc.has_active_scroll_animation(view.id) {
                return true;
            }
        }
    }

    false
}
```

## Files to Modify

1. **`core/document.rs`** - Add `ScrollAnimation` struct and per-view animation state
2. **`core/commands.rs`** - Add `scroll_animated()`, modify page/half-page commands
3. **`application.rs`** - Update `animate_scroll()` with exponential decay, update `wants_redraw()`
4. **`ui/editor_view.rs`** - Use fractional scroll offset for sub-pixel rendering

## Testing

1. Open editor, create vsplit
2. Page up/down should animate smoothly (not jump)
3. Half page up/down should animate
4. Mouse wheel should continue to work smoothly
5. Scroll should be frame-rate independent (same speed at 30fps vs 144fps)
6. Animation should feel snappy but smooth (exponential decay characteristic)

## Optional Enhancements

1. **Configurable speed**: Add `scroll_animation_speed` config (the `60.0` constant)
2. **Disable option**: Add `smooth_scroll_keyboard` config to disable for keyboard commands
3. **Cursor following**: Optionally animate cursor along with scroll for `page_cursor_*` commands
