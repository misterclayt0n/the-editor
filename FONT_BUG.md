# Bug: Diagnostic text loses monospace font in SwiftUI Canvas

## Symptom

In `the-swift` (macOS SwiftUI frontend), diagnostic overlay text (both inline diagnostics with box-drawing characters and end-of-line diagnostic messages) renders in the correct **monospace Iosevka** font initially, but **switches to a proportional system font** after the user makes edits and new LSP diagnostics arrive.

Regular code text rendered by `drawText()` in the same Canvas **never** loses its font — it stays monospace through all re-renders.

## Architecture

All editor content is drawn in a single SwiftUI `Canvas` view:

```swift
// EditorView.swift, line ~29
Canvas { context, size in
    drawPlan(in: context, size: size, plan: model.plan, cellSize: cellSize, font: font)
}
```

`drawPlan()` calls these in order:
1. `drawGutter()` — line numbers (monospace, works fine)
2. `drawSelections()` — selection highlights
3. `drawDiagnosticUnderlines()` — wavy underlines
4. `drawText()` — code content (monospace, **always works**)
5. `drawInlineDiagnostics()` — box-drawing connector lines + messages (**breaks**)
6. `drawEolDiagnostics()` — end-of-line diagnostic text (**breaks**)
7. `drawCursors()`

## The font

```swift
// FontLoader.swift
let fontInfo = FontLoader.loadIosevka(size: 14)
// Registers Iosevka-Regular.ttc via CTFontManagerRegisterFontsForURL(.process)
// Returns Font.custom(postScriptName, size: 14)
```

The same `font: Font` object is passed to ALL drawing methods. It is created once in `EditorModel.init()` and never changes.

## What works vs what breaks

### Works (always monospace):
```swift
// drawText() — EditorView.swift line ~366
// Draws many SHORT text spans (syntax tokens, typically 1-20 chars each)
let text = Text(span.text().toString()).font(font).foregroundColor(color)
context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
```

### Breaks (switches to proportional after re-renders):
```swift
// drawInlineDiagnostics() — draws char-by-char at grid positions
for (ci, ch) in line.text().toString().enumerated() {
    let cx = baseX + CGFloat(ci) * cellSize.width
    let text = Text(String(ch)).font(font).foregroundColor(color)
    context.draw(text, at: CGPoint(x: cx, y: y), anchor: .topLeading)
}
```

```swift
// drawEolDiagnostics() — also draws char-by-char at grid positions
for (ci, ch) in eol.message().toString().enumerated() {
    let cx = baseX + CGFloat(ci) * cellSize.width
    let text = Text(String(ch)).font(font).foregroundColor(color)
    context.draw(text, at: CGPoint(x: cx, y: y), anchor: .topLeading)
}
```

## What we've tried

1. **Single long `Text` view per diagnostic line** — breaks (font goes proportional)
2. **Character-by-character drawing** at grid positions — still breaks (same proportional font)
3. Both use the exact same `font` variable and `context.draw(Text(...), at:)` pattern as `drawText()`

## The mystery

- The `font` object is **identical** across all methods (same `Font.custom(postScript, size: 14)`)
- The drawing pattern is **identical** (`Text(str).font(font).foregroundColor(color)` → `context.draw(text, at:, anchor:)`)
- `drawText()` draws hundreds of small spans per frame and **never** loses the font
- Diagnostic drawing uses the same pattern but **does** lose the font after re-renders triggered by LSP diagnostic updates

## Possible causes to investigate

1. **Canvas re-render timing**: Diagnostic data changes asynchronously (LSP responses) while code text changes synchronously with keystrokes. Perhaps the Canvas re-render triggered by diagnostic updates has different font resolution behavior.

2. **CTFontManager font registration scope**: The font is registered with `.process` scope via `CTFontManagerRegisterFontsForURL`. Perhaps re-renders on certain threads/queues don't see the registered font.

3. **SwiftUI Text + Font.custom resolution bug**: `Font.custom(name, size:)` might resolve differently in certain Canvas rendering passes. Consider using `Font(nsFont as CTFont)` instead, which creates the Font from a concrete platform font object rather than a name lookup.

4. **GraphicsContext state**: Perhaps something in the draw calls before diagnostics (drawText draws hundreds of items) puts the GraphicsContext into a state where subsequent `Text` rendering falls back.

5. **The color**: Diagnostic text uses `diagnosticColor(severity:).opacity(0.6)` (e.g., `Color(nsColor: .systemRed).opacity(0.6)`). Regular text uses `ColorMapper.color(from:)`. Perhaps the color type or opacity modifier interacts with font rendering.

## Key files

- `the-swift/Sources/TheSwift/EditorView.swift` — all drawing methods
- `the-swift/Sources/TheSwift/EditorModel.swift` — stores `font: Font` and `cellSize: CGSize`
- `the-swift/Sources/TheSwift/FontLoader.swift` — font loading and registration
- `the-swift/Sources/TheSwift/ColorMapper.swift` — color mapping for regular text

## Desired outcome

Diagnostic text (both inline and EOL) should render in the same monospace Iosevka font as code text, consistently across all re-renders.
