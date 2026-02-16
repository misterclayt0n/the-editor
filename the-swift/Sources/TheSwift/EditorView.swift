import SwiftUI
import TheEditorFFIBridge

struct EditorView: View {
    @StateObject private var model: EditorModel

    init(filePath: String? = nil) {
        _model = StateObject(wrappedValue: EditorModel(filePath: filePath))
    }

    var body: some View {
        let model = model
        let cellSize = model.cellSize
        let font = model.font
        let isPaletteOpen = model.uiTree.hasCommandPalettePanel
        let isSearchOpen = model.uiTree.hasSearchPromptPanel
        let isFilePickerOpen = model.filePickerSnapshot?.active ?? false
        let isOverlayOpen = isPaletteOpen || isSearchOpen || isFilePickerOpen
        let completionSnapshot = model.uiTree.completionSnapshot()
        let hoverSnapshot = model.uiTree.hoverSnapshot()
        let signatureSnapshot = model.uiTree.signatureHelpSnapshot()
        let isCompletionOpen = completionSnapshot != nil
        let isHoverOpen = hoverSnapshot != nil && !isCompletionOpen
        let isSignatureOpen = signatureSnapshot != nil && !isCompletionOpen && !isHoverOpen
        GeometryReader { proxy in
            ZStack {
                Canvas { context, size in
                    drawPlan(in: context, size: size, plan: model.plan, cellSize: cellSize, font: font)
                }
                .background(SwiftUI.Color.black)

                UiOverlayHost(
                    tree: model.uiTree,
                    cellSize: cellSize,
                    filePickerSnapshot: model.filePickerSnapshot,
                    pendingKeys: model.pendingKeys,
                    onSelectCommand: { index in
                        model.selectCommandPalette(index: index)
                    },
                    onSubmitCommand: { index in
                        model.submitCommandPalette(index: index)
                    },
                    onCloseCommandPalette: {
                        model.closeCommandPalette()
                    },
                    onQueryChange: { query in
                        model.setCommandPaletteQuery(query)
                    },
                    onSearchQueryChange: { query in
                        model.setSearchQuery(query)
                    },
                    onSearchPrev: {
                        model.searchPrev()
                    },
                    onSearchNext: {
                        model.searchNext()
                    },
                    onSearchClose: {
                        model.closeSearch()
                    },
                    onSearchSubmit: {
                        model.submitSearch()
                    },
                    onFilePickerQueryChange: { query in
                        model.setFilePickerQuery(query)
                    },
                    onFilePickerSubmit: { index in
                        model.submitFilePicker(index: index)
                    },
                    onFilePickerClose: {
                        model.closeFilePicker()
                    }
                )

                if let completion = completionSnapshot {
                    CompletionPopupView(
                        snapshot: completion,
                        cursorOrigin: cursorPixelPosition(plan: model.plan, cellSize: cellSize),
                        cellSize: cellSize,
                        containerSize: proxy.size,
                        languageHint: model.completionDocsLanguageHint(),
                        onSelect: { index in
                            model.selectCompletion(index: index)
                        },
                        onSubmit: { index in
                            model.submitCompletion(index: index)
                        }
                    )
                    .allowsHitTesting(true)
                } else if let hover = hoverSnapshot {
                    HoverPopupView(
                        snapshot: hover,
                        cursorOrigin: cursorPixelPosition(plan: model.plan, cellSize: cellSize),
                        cellSize: cellSize,
                        containerSize: proxy.size,
                        languageHint: model.completionDocsLanguageHint()
                    )
                    .allowsHitTesting(true)
                } else if let signature = signatureSnapshot {
                    SignatureHelpPopupView(
                        snapshot: signature,
                        cursorOrigin: cursorPixelPosition(plan: model.plan, cellSize: cellSize),
                        cellSize: cellSize,
                        containerSize: proxy.size,
                        languageHint: model.completionDocsLanguageHint()
                    )
                    .allowsHitTesting(true)
                }
            }
            .background(
                Group {
                    if !isOverlayOpen {
                        KeyCaptureView(
                            onKey: { event in
                                model.handleKeyEvent(event)
                            },
                            onText: { text, modifiers in
                                model.handleText(text, modifiers: modifiers)
                            },
                            onScroll: { _, _, _ in },
                            modeProvider: {
                                model.mode
                            }
                        )
                        .allowsHitTesting(false)
                    }
                }
            )
            .overlay(
                Group {
                    if !isOverlayOpen && !isCompletionOpen && !isHoverOpen && !isSignatureOpen {
                        ScrollCaptureView(
                            onScroll: { deltaX, deltaY, precise in
                                model.handleScroll(deltaX: deltaX, deltaY: deltaY, precise: precise)
                            }
                        )
                        .allowsHitTesting(true)
                    }
                }
            )
            .overlay(
                KeySequenceIndicator(keys: model.pendingKeys, hints: model.pendingKeyHints)
                    .padding(.bottom, 28)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom),
                alignment: .bottom
            )
            .onAppear {
                model.updateViewport(pixelSize: proxy.size, cellSize: cellSize)
            }
            .onChange(of: proxy.size) { newSize in
                model.updateViewport(pixelSize: newSize, cellSize: cellSize)
            }
            .onChange(of: isCompletionOpen) { isOpen in
                guard !isOpen else {
                    return
                }
                DispatchQueue.main.async {
                    KeyCaptureFocusBridge.shared.reclaimActive()
                }
            }
        }
    }

    private func cursorPixelPosition(plan: RenderPlan, cellSize: CGSize) -> CGPoint {
        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        let count = Int(plan.cursor_count())
        guard count > 0 else {
            return CGPoint(x: contentOffsetX, y: 0)
        }
        let pos = plan.cursor_at(0).pos()
        return CGPoint(
            x: contentOffsetX + CGFloat(pos.col) * cellSize.width,
            y: CGFloat(pos.row) * cellSize.height
        )
    }

    private func drawPlan(in context: GraphicsContext, size: CGSize, plan: RenderPlan, cellSize: CGSize, font: Font) {
        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        drawGutter(in: context, size: size, plan: plan, cellSize: cellSize, font: font, contentOffsetX: contentOffsetX)
        drawSelections(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        drawText(in: context, plan: plan, cellSize: cellSize, font: font, contentOffsetX: contentOffsetX)
        drawCursors(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
    }

    // MARK: - Gutter

    private enum GutterSpanKind {
        case lineNumber
        case diagnostic
        case diff
        case other
    }

    private func classifyGutterSpan(_ span: RenderGutterSpan) -> GutterSpanKind {
        let text = span.text().toString()
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty { return .other }
        if trimmed == "\u{25CF}" { return .diagnostic }
        if trimmed == "+" || trimmed == "~" || trimmed == "-" { return .diff }
        if trimmed.allSatisfy({ $0.isNumber }) { return .lineNumber }
        return .other
    }

    private func activeCursorRow(plan: RenderPlan) -> UInt16? {
        let count = Int(plan.cursor_count())
        guard count > 0 else { return nil }
        let pos = plan.cursor_at(0).pos()
        return UInt16(clamping: pos.row)
    }

    private func drawGutter(
        in context: GraphicsContext,
        size: CGSize,
        plan: RenderPlan,
        cellSize: CGSize,
        font: Font,
        contentOffsetX: CGFloat
    ) {
        let lineCount = Int(plan.gutter_line_count())
        guard lineCount > 0, contentOffsetX > 0 else { return }

        let activeRow = activeCursorRow(plan: plan)

        // Layer 1: Gutter background
        let gutterBg = CGRect(x: 0, y: 0, width: contentOffsetX, height: size.height)
        context.fill(Path(gutterBg), with: .color(SwiftUI.Color(white: 0.055)))

        // Layer 2: Active line highlight
        if let activeRow {
            let y = CGFloat(activeRow) * cellSize.height
            let rect = CGRect(x: 0, y: y, width: contentOffsetX, height: cellSize.height)
            context.fill(Path(rect), with: .color(SwiftUI.Color.white.opacity(0.04)))
        }

        // Layer 3: Vertical separator
        let sepPath = Path { p in
            p.move(to: CGPoint(x: contentOffsetX - 0.5, y: 0))
            p.addLine(to: CGPoint(x: contentOffsetX - 0.5, y: size.height))
        }
        context.stroke(sepPath, with: .color(SwiftUI.Color(nsColor: .separatorColor).opacity(0.3)), lineWidth: 0.5)

        // Layer 4: Per-span rendering
        for lineIndex in 0..<lineCount {
            let line = plan.gutter_line_at(UInt(lineIndex))
            let y = CGFloat(line.row()) * cellSize.height
            let isActiveLine = activeRow == line.row()
            let spanCount = Int(line.span_count())

            for spanIndex in 0..<spanCount {
                let span = line.span_at(UInt(spanIndex))
                let x = CGFloat(span.col()) * cellSize.width
                let kind = classifyGutterSpan(span)

                switch kind {
                case .diagnostic:
                    let color = colorForStyle(span.style(), fallback: SwiftUI.Color(nsColor: .systemRed))
                    drawDiagnosticIndicator(in: context, y: y, cellSize: cellSize, gutterWidth: contentOffsetX, color: color)
                case .diff:
                    let fallback: SwiftUI.Color
                    switch span.text().toString().trimmingCharacters(in: .whitespaces) {
                    case "+": fallback = SwiftUI.Color(nsColor: .systemGreen)
                    case "~": fallback = SwiftUI.Color(nsColor: .systemYellow)
                    default:  fallback = SwiftUI.Color(nsColor: .systemRed)
                    }
                    let color = colorForStyle(span.style(), fallback: fallback)
                    drawDiffBar(in: context, y: y, cellSize: cellSize, gutterWidth: contentOffsetX, color: color)
                case .lineNumber:
                    drawLineNumber(in: context, span: span, x: x, y: y, font: font, isActive: isActiveLine)
                case .other:
                    break
                }
            }
        }
    }

    private func drawDiagnosticIndicator(
        in context: GraphicsContext,
        y: CGFloat,
        cellSize: CGSize,
        gutterWidth: CGFloat,
        color: SwiftUI.Color
    ) {
        // Row tint: subtle color wash across the entire gutter row
        let tintRect = CGRect(x: 0, y: y, width: gutterWidth, height: cellSize.height)
        context.fill(Path(tintRect), with: .color(color.opacity(0.06)))

        // Left-edge stripe: flush against the left wall, full cell height
        let stripeWidth: CGFloat = 2.0
        let stripeRect = CGRect(x: 0, y: y, width: stripeWidth, height: cellSize.height)
        context.fill(Path(stripeRect), with: .color(color.opacity(0.7)))
    }

    private func drawDiffBar(
        in context: GraphicsContext,
        y: CGFloat,
        cellSize: CGSize,
        gutterWidth: CGFloat,
        color: SwiftUI.Color
    ) {
        // Thin stripe along the separator edge — consecutive lines merge into
        // one continuous colored strip (no vertical inset).
        let barWidth: CGFloat = 2.0
        let barRect = CGRect(
            x: gutterWidth - barWidth - 0.5,
            y: y,
            width: barWidth,
            height: cellSize.height
        )
        context.fill(Path(barRect), with: .color(color.opacity(0.7)))
    }

    private func drawLineNumber(
        in context: GraphicsContext,
        span: RenderGutterSpan,
        x: CGFloat, y: CGFloat,
        font: Font,
        isActive: Bool
    ) {
        let style = span.style()
        let color: SwiftUI.Color
        if style.has_fg, let themeColor = ColorMapper.color(from: style.fg) {
            color = themeColor
        } else if isActive {
            color = SwiftUI.Color(nsColor: .secondaryLabelColor)
        } else {
            color = SwiftUI.Color(nsColor: .tertiaryLabelColor)
        }
        let text = Text(span.text().toString()).font(font).foregroundColor(color)
        context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
    }

    private func drawSelections(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat
    ) {
        let count = Int(plan.selection_count())
        guard count > 0 else { return }

        for index in 0..<count {
            let selection = plan.selection_at(UInt(index))
            let rect = selection.rect()
            let x = contentOffsetX + CGFloat(rect.x) * cellSize.width
            let y = CGFloat(rect.y) * cellSize.height
            let width = CGFloat(rect.width) * cellSize.width
            let height = CGFloat(rect.height) * cellSize.height
            let path = Path(CGRect(x: x, y: y, width: width, height: height))
            context.fill(path, with: .color(SwiftUI.Color.accentColor.opacity(0.25)))
        }
    }

    private func drawText(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        font: Font,
        contentOffsetX: CGFloat
    ) {
        let lineCount = Int(plan.line_count())
        guard lineCount > 0 else { return }

        for lineIndex in 0..<lineCount {
            let line = plan.line_at(UInt(lineIndex))
            let y = CGFloat(line.row()) * cellSize.height
            let spanCount = Int(line.span_count())

            for spanIndex in 0..<spanCount {
                let span = line.span_at(UInt(spanIndex))
                let x = contentOffsetX + CGFloat(span.col()) * cellSize.width
                let color = colorForSpan(span)
                let text = Text(span.text().toString()).font(font).foregroundColor(color)
                context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
            }
        }
    }

    private func drawCursors(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat
    ) {
        let count = Int(plan.cursor_count())
        guard count > 0 else { return }

        for index in 0..<count {
            let cursor = plan.cursor_at(UInt(index))
            let pos = cursor.pos()
            let x = contentOffsetX + CGFloat(pos.col) * cellSize.width
            let y = CGFloat(pos.row) * cellSize.height
            let cursorColor = SwiftUI.Color.accentColor.opacity(0.8)

            switch cursor.kind() {
            case 1: // bar
                let rect = CGRect(x: x, y: y, width: 2, height: cellSize.height)
                context.fill(Path(rect), with: .color(cursorColor))
            case 2: // underline
                let rect = CGRect(x: x, y: y + cellSize.height - 2, width: cellSize.width, height: 2)
                context.fill(Path(rect), with: .color(cursorColor))
            case 3: // hollow
                let rect = CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height)
                context.stroke(Path(rect), with: .color(cursorColor), lineWidth: 1)
            case 4: // hidden
                continue
            default: // block
                let rect = CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height)
                context.fill(Path(rect), with: .color(cursorColor.opacity(0.5)))
            }
        }
    }

    private func colorForStyle(_ style: Style, fallback: SwiftUI.Color) -> SwiftUI.Color {
        guard style.has_fg, let color = ColorMapper.color(from: style.fg) else {
            return fallback
        }
        return color
    }

    private func colorForSpan(_ span: RenderSpan) -> SwiftUI.Color {
        if span.has_highlight() {
            if let color = model.colorForHighlight(span.highlight()) {
                return color
            }
        }
        if span.is_virtual() {
            return SwiftUI.Color.white.opacity(0.4)
        }
        return SwiftUI.Color.white
    }
}
