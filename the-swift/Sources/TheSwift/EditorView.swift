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
                    if !isOverlayOpen {
                        ScrollCaptureView(
                            onScroll: { deltaX, deltaY, precise in
                                model.handleScroll(deltaX: deltaX, deltaY: deltaY, precise: precise)
                            }
                        )
                        .allowsHitTesting(true)
                    }
                }
            )
            .onAppear {
                model.updateViewport(pixelSize: proxy.size, cellSize: cellSize)
            }
            .onChange(of: proxy.size) { newSize in
                model.updateViewport(pixelSize: newSize, cellSize: cellSize)
            }
        }
    }

    private func drawPlan(in context: GraphicsContext, size: CGSize, plan: RenderPlan, cellSize: CGSize, font: Font) {
        drawSelections(in: context, plan: plan, cellSize: cellSize)
        drawText(in: context, plan: plan, cellSize: cellSize, font: font)
        drawCursors(in: context, plan: plan, cellSize: cellSize)
    }

    private func drawSelections(in context: GraphicsContext, plan: RenderPlan, cellSize: CGSize) {
        let count = Int(plan.selection_count())
        guard count > 0 else { return }

        for index in 0..<count {
            let selection = plan.selection_at(UInt(index))
            let rect = selection.rect()
            let x = CGFloat(rect.x) * cellSize.width
            let y = CGFloat(rect.y) * cellSize.height
            let width = CGFloat(rect.width) * cellSize.width
            let height = CGFloat(rect.height) * cellSize.height
            let path = Path(CGRect(x: x, y: y, width: width, height: height))
            context.fill(path, with: .color(SwiftUI.Color.accentColor.opacity(0.25)))
        }
    }

    private func drawText(in context: GraphicsContext, plan: RenderPlan, cellSize: CGSize, font: Font) {
        let lineCount = Int(plan.line_count())
        guard lineCount > 0 else { return }

        for lineIndex in 0..<lineCount {
            let line = plan.line_at(UInt(lineIndex))
            let y = CGFloat(line.row()) * cellSize.height
            let spanCount = Int(line.span_count())

            for spanIndex in 0..<spanCount {
                let span = line.span_at(UInt(spanIndex))
                let x = CGFloat(span.col()) * cellSize.width
                let color = colorForSpan(span)
                let text = Text(span.text().toString()).font(font).foregroundColor(color)
                context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
            }
        }
    }

    private func drawCursors(in context: GraphicsContext, plan: RenderPlan, cellSize: CGSize) {
        let count = Int(plan.cursor_count())
        guard count > 0 else { return }

        for index in 0..<count {
            let cursor = plan.cursor_at(UInt(index))
            let pos = cursor.pos()
            let x = CGFloat(pos.col) * cellSize.width
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
