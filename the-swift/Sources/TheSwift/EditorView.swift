import SwiftUI
import TheEditorFFIBridge

struct EditorView: View {
    @StateObject private var model: EditorModel

    private struct CursorPickState {
        let remove: Bool
        let currentIndex: Int
    }

    init(filePath: String? = nil) {
        _model = StateObject(wrappedValue: EditorModel(filePath: filePath))
    }

    var body: some View {
        let model = model
        let cellSize = model.cellSize
        let bufferFont = model.bufferFont
        let bufferNSFont = model.bufferNSFont
        let isPaletteOpen = model.uiTree.hasCommandPalettePanel
        let isSearchOpen = model.uiTree.hasSearchPromptPanel
        let isFilePickerOpen = model.filePickerSnapshot?.active ?? false
        let isInputPromptOpen = model.uiTree.hasInputPromptPanel
        let isOverlayOpen = isPaletteOpen || isSearchOpen || isFilePickerOpen || isInputPromptOpen
        let completionSnapshot = model.uiTree.completionSnapshot()
        let hoverSnapshot = model.uiTree.hoverSnapshot()
        let signatureSnapshot = model.uiTree.signatureHelpSnapshot()
        let isCompletionOpen = completionSnapshot != nil
        let isHoverOpen = hoverSnapshot != nil && !isCompletionOpen
        let isSignatureOpen = signatureSnapshot != nil && !isCompletionOpen && !isHoverOpen
        let cursorPickState = cursorPickState(from: model.uiTree.statuslineSnapshot())
        GeometryReader { proxy in
            ZStack {
                Canvas { context, size in
                    drawPlan(
                        in: context,
                        size: size,
                        plan: model.plan,
                        cellSize: cellSize,
                        bufferFont: bufferFont,
                        bufferNSFont: bufferNSFont,
                        cursorPickState: cursorPickState
                    )
                }
                .background(SwiftUI.Color.black)

                UiOverlayHost(
                    tree: model.uiTree,
                    cellSize: cellSize,
                    filePickerSnapshot: model.filePickerSnapshot,
                    filePickerPreviewModel: model.filePickerPreviewModel,
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
                    },
                    onFilePickerSelectionChange: { index in
                        model.filePickerSelectIndex(index)
                    },
                    colorForHighlight: { highlightId in
                        model.colorForHighlight(highlightId)
                    },
                    onInputPromptQueryChange: { query in
                        model.setSearchQuery(query)
                    },
                    onInputPromptClose: {
                        model.closeSearch()
                    },
                    onInputPromptSubmit: {
                        model.submitSearch()
                    }
                )

                if !isOverlayOpen {
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

    private func cursorPickState(from snapshot: StatuslineSnapshot?) -> CursorPickState? {
        guard let snapshot else { return nil }

        let modeToken = snapshot
            .left
            .split(whereSeparator: { $0.isWhitespace })
            .first?
            .uppercased()
        let remove: Bool
        switch modeToken {
        case "COL":
            remove = false
        case "REM":
            remove = true
        default:
            return nil
        }

        let prefix = remove ? "remove " : "collapse "
        let rightSegments = snapshot.rightSegments.map(\.text)
        let pickIndex = rightSegments.compactMap { segment in
            parseCursorPickIndex(segment: segment, prefix: prefix)
        }.first
        guard let pickIndex else { return nil }

        return CursorPickState(remove: remove, currentIndex: pickIndex)
    }

    private func parseCursorPickIndex(segment: String, prefix: String) -> Int? {
        let lower = segment.lowercased()
        guard lower.hasPrefix(prefix) else { return nil }
        let remainder = lower.dropFirst(prefix.count)
        let parts = remainder.split(separator: "/", maxSplits: 1)
        guard
            parts.count == 2,
            let current = Int(parts[0]),
            current >= 1
        else {
            return nil
        }
        return current - 1
    }

    private func drawPlan(
        in context: GraphicsContext,
        size: CGSize,
        plan: RenderPlan,
        cellSize: CGSize,
        bufferFont: Font,
        bufferNSFont: NSFont,
        cursorPickState: CursorPickState?
    ) {
        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        drawGutter(
            in: context,
            size: size,
            plan: plan,
            cellSize: cellSize,
            font: bufferFont,
            contentOffsetX: contentOffsetX
        )
        drawSelections(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        drawDiagnosticUnderlines(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        let textStats = drawText(
            in: context,
            plan: plan,
            cellSize: cellSize,
            font: bufferFont,
            contentOffsetX: contentOffsetX
        )
        let inlineSummary = drawInlineDiagnostics(
            in: context,
            plan: plan,
            cellSize: cellSize,
            nsFont: bufferNSFont,
            contentOffsetX: contentOffsetX
        )
        let eolSummary = drawEolDiagnostics(
            in: context,
            plan: plan,
            cellSize: cellSize,
            nsFont: bufferNSFont,
            contentOffsetX: contentOffsetX
        )
        drawCursors(
            in: context,
            plan: plan,
            cellSize: cellSize,
            contentOffsetX: contentOffsetX,
            cursorPickState: cursorPickState
        )
        debugDrawSnapshot(
            plan: plan,
            textStats: textStats,
            inlineSummary: inlineSummary,
            eolSummary: eolSummary
        )
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

    private struct DrawTextStats {
        let drawnSpans: Int
        let skippedVirtualSpans: Int
    }

    private func drawText(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        font: Font,
        contentOffsetX: CGFloat
    ) -> DrawTextStats {
        let lineCount = Int(plan.line_count())
        guard lineCount > 0 else {
            return DrawTextStats(drawnSpans: 0, skippedVirtualSpans: 0)
        }

        var drawnSpans = 0
        var skippedVirtualSpans = 0

        for lineIndex in 0..<lineCount {
            let line = plan.line_at(UInt(lineIndex))
            let y = CGFloat(line.row()) * cellSize.height
            let spanCount = Int(line.span_count())

            for spanIndex in 0..<spanCount {
                let span = line.span_at(UInt(spanIndex))
                // Inline diagnostics are rendered explicitly in dedicated passes
                // below; skipping virtual spans avoids double-drawing artifacts.
                if span.is_virtual() {
                    skippedVirtualSpans += 1
                    continue
                }
                drawnSpans += 1
                let x = contentOffsetX + CGFloat(span.col()) * cellSize.width
                let color = colorForSpan(span)
                let text = Text(span.text().toString()).font(font).foregroundColor(color)
                context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
            }
        }

        return DrawTextStats(drawnSpans: drawnSpans, skippedVirtualSpans: skippedVirtualSpans)
    }

    private func drawCursors(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        cursorPickState: CursorPickState?
    ) {
        let count = Int(plan.cursor_count())
        guard count > 0 else { return }

        let pickedCursorId: UInt64? = {
            guard let cursorPickState else { return nil }
            guard cursorPickState.currentIndex >= 0 && cursorPickState.currentIndex < count else { return nil }
            return plan.cursor_at(UInt(cursorPickState.currentIndex)).id()
        }()
        let defaultCursorColor = SwiftUI.Color.accentColor.opacity(0.9)
        let pickedCursorColor: SwiftUI.Color = {
            guard let cursorPickState else { return defaultCursorColor }
            return cursorPickState.remove
                ? SwiftUI.Color(nsColor: .systemRed)
                : SwiftUI.Color(nsColor: .systemOrange)
        }()

        for index in 0..<count {
            let cursor = plan.cursor_at(UInt(index))
            let pos = cursor.pos()
            let x = contentOffsetX + CGFloat(pos.col) * cellSize.width
            let y = CGFloat(pos.row) * cellSize.height
            let isPickedCursor = pickedCursorId == cursor.id()
            let strokeColor = isPickedCursor ? pickedCursorColor : defaultCursorColor

            switch cursor.kind() {
            case 1: // bar
                let rect = CGRect(x: x, y: y, width: 2.5, height: cellSize.height)
                context.fill(Path(rect), with: .color(strokeColor))
            case 2: // underline
                let rect = CGRect(x: x, y: y + cellSize.height - 2, width: cellSize.width, height: 2)
                context.fill(Path(rect), with: .color(strokeColor))
            case 3: // hollow
                let rect = CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height)
                context.stroke(Path(rect), with: .color(strokeColor), lineWidth: 1.2)
            case 4: // hidden
                continue
            default: // block
                let rect = CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height)
                context.fill(Path(rect), with: .color(strokeColor.opacity(isPickedCursor ? 0.65 : 0.5)))
            }
        }
    }

    // MARK: - Inline Diagnostics

    private func drawInlineDiagnostics(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        nsFont: NSFont,
        contentOffsetX: CGFloat
    ) -> String {
        let count = Int(plan.inline_diagnostic_line_count())
        guard count > 0 else { return "count=0" }
        var entries: [String] = []

        for i in 0..<count {
            let line = plan.inline_diagnostic_line_at(UInt(i))
            let y = CGFloat(line.row()) * cellSize.height
            let baseX = contentOffsetX + CGFloat(line.col()) * cellSize.width
            let color = diagnosticNSColor(severity: line.severity()).withAlphaComponent(0.6)
            entries.append(
                "\(line.row()):\(line.col()):\(line.severity()):\(debugTruncate(line.text().toString(), limit: 70))"
            )
            drawDiagnosticText(
                in: context,
                text: line.text().toString(),
                at: CGPoint(x: baseX, y: y),
                nsFont: nsFont,
                color: color
            )
        }
        return "count=\(count) [\(entries.joined(separator: " || "))]"
    }

    private func diagnosticColor(severity: UInt8) -> SwiftUI.Color {
        switch severity {
        case 1: return SwiftUI.Color(nsColor: .systemRed)
        case 2: return SwiftUI.Color(nsColor: .systemYellow)
        case 3: return SwiftUI.Color(nsColor: .systemBlue)
        case 4: return SwiftUI.Color(nsColor: .systemGreen)
        default: return SwiftUI.Color.white
        }
    }

    private func diagnosticNSColor(severity: UInt8) -> NSColor {
        switch severity {
        case 1: return .systemRed
        case 2: return .systemYellow
        case 3: return .systemBlue
        case 4: return .systemGreen
        default: return .white
        }
    }

    private func drawDiagnosticText(
        in context: GraphicsContext,
        text: String,
        at point: CGPoint,
        nsFont: NSFont,
        color: NSColor
    ) {
        guard !text.isEmpty else { return }
        let attrs: [NSAttributedString.Key: Any] = [
            .font: nsFont,
            .foregroundColor: color
        ]
        let attributed = NSAttributedString(string: text, attributes: attrs)
        context.withCGContext { cg in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: cg, flipped: true)
            attributed.draw(at: point)
            NSGraphicsContext.restoreGraphicsState()
        }
    }

    // MARK: - End-of-Line Diagnostics

    private func drawEolDiagnostics(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        nsFont: NSFont,
        contentOffsetX: CGFloat
    ) -> String {
        let count = Int(plan.eol_diagnostic_count())
        guard count > 0 else { return "count=0" }
        var entries: [String] = []

        for i in 0..<count {
            let eol = plan.eol_diagnostic_at(UInt(i))
            let y = CGFloat(eol.row()) * cellSize.height
            let baseX = contentOffsetX + CGFloat(eol.col()) * cellSize.width
            let color = diagnosticNSColor(severity: eol.severity()).withAlphaComponent(0.6)
            entries.append(
                "\(eol.row()):\(eol.col()):\(eol.severity()):\(debugTruncate(eol.message().toString(), limit: 70))"
            )
            drawDiagnosticText(
                in: context,
                text: eol.message().toString(),
                at: CGPoint(x: baseX, y: y),
                nsFont: nsFont,
                color: color
            )
        }
        return "count=\(count) [\(entries.joined(separator: " || "))]"
    }

    // MARK: - Diagnostic Underlines

    private func drawDiagnosticUnderlines(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat
    ) {
        let count = Int(plan.diagnostic_underline_count())
        guard count > 0 else { return }

        for i in 0..<count {
            let underline = plan.diagnostic_underline_at(UInt(i))
            let y = CGFloat(underline.row()) * cellSize.height + cellSize.height - 1
            let xStart = contentOffsetX + CGFloat(underline.start_col()) * cellSize.width
            let xEnd = contentOffsetX + CGFloat(underline.end_col()) * cellSize.width
            let color = diagnosticColor(severity: underline.severity()).opacity(0.7)

            // Draw wavy underline
            let width = xEnd - xStart
            guard width > 0 else { continue }
            let waveHeight: CGFloat = 2.0
            let wavelength: CGFloat = 4.0
            var path = Path()
            var x = xStart
            path.move(to: CGPoint(x: x, y: y))
            var up = true
            while x < xEnd {
                let nextX = min(x + wavelength, xEnd)
                let controlY = up ? y - waveHeight : y + waveHeight
                path.addQuadCurve(
                    to: CGPoint(x: nextX, y: y),
                    control: CGPoint(x: (x + nextX) / 2, y: controlY)
                )
                x = nextX
                up.toggle()
            }
            context.stroke(path, with: .color(color), lineWidth: 1.0)
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

    private func debugDrawSnapshot(
        plan: RenderPlan,
        textStats: DrawTextStats,
        inlineSummary: String,
        eolSummary: String
    ) {
        guard DiagnosticsDebugLog.enabled else { return }
        let cursorCount = Int(plan.cursor_count())
        let cursorSummary: String
        if cursorCount > 0 {
            let pos = plan.cursor_at(0).pos()
            cursorSummary = "\(pos.row):\(pos.col)"
        } else {
            cursorSummary = "none"
        }
        DiagnosticsDebugLog.logChanged(
            key: "view.draw",
            value: "cursor=\(cursorSummary) drawn_spans=\(textStats.drawnSpans) skipped_virtual=\(textStats.skippedVirtualSpans) inline=\(inlineSummary) eol=\(eolSummary)"
        )
    }

    private func debugTruncate(_ text: String, limit: Int) -> String {
        let normalized = text
            .replacingOccurrences(of: "\n", with: "\\n")
            .replacingOccurrences(of: "\t", with: "\\t")
        if normalized.count <= limit {
            return normalized
        }
        let idx = normalized.index(normalized.startIndex, offsetBy: limit)
        return "\(normalized[..<idx])..."
    }
}
