import AppKit
import SwiftUI

private let docsStyleBold: UInt16 = 1 << 0
private let docsStyleItalic: UInt16 = 1 << 2

struct EditorDocsPanelsView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let scene = controller.scene {
                    if controller.signatureHelp.isOpen {
                        EditorDocsPanelOverlay(
                            title: "Signature Help",
                            panel: controller.signatureHelp,
                            scene: scene,
                            backgroundColor: controller.chrome.backgroundColor
                        )
                        .zIndex(1)
                    }

                    if controller.hoverDocs.isOpen {
                        EditorDocsPanelOverlay(
                            title: "Hover",
                            panel: controller.hoverDocs,
                            scene: scene,
                            backgroundColor: controller.chrome.backgroundColor
                        )
                        .zIndex(2)
                    }
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
        }
        .allowsHitTesting(true)
    }
}

private struct EditorDocsPanelOverlay: View {
    let title: String
    let panel: EditorDocsPanelState
    let scene: EditorRenderScene
    let backgroundColor: NSColor

    var body: some View {
        if panel.isOpen, panel.width > 0, panel.height > 0 {
            EditorSelectableDocsTextView(
                attributedText: attributedText,
                backgroundColor: backgroundColor
            )
            .frame(width: panelSize.width, height: panelSize.height)
            .background(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(Color(nsColor: backgroundColor))
            )
            .overlay {
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.10), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.16), radius: 10, y: 4)
            .offset(x: panelOrigin.x, y: panelOrigin.y)
            .transition(.opacity)
            .accessibilityLabel(title)
        }
    }

    private var panelOrigin: CGPoint {
        let metrics = scene.info.surfaceMetrics
        return CGPoint(
            x: CGFloat(panel.col) * metrics.cellSizePoints.width,
            y: CGFloat(panel.row) * metrics.cellSizePoints.height
        )
    }

    private var panelSize: CGSize {
        let metrics = scene.info.surfaceMetrics
        return CGSize(
            width: CGFloat(panel.width) * metrics.cellSizePoints.width,
            height: CGFloat(panel.height) * metrics.cellSizePoints.height
        )
    }

    private var attributedText: NSAttributedString {
        let storage = NSMutableAttributedString()
        for run in panel.runs {
            storage.append(NSAttributedString(string: run.text, attributes: attributes(for: run)))
        }
        return storage
    }

    private func attributes(for run: EditorDocsRun) -> [NSAttributedString.Key: Any] {
        let font = font(for: run)
        var attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: run.style.foregroundColor
        ]

        if let backgroundColor = run.style.backgroundColor {
            attributes[.backgroundColor] = backgroundColor
        }

        if run.style.underlineStyle != 0 {
            attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue
            attributes[.underlineColor] = run.style.underlineColor?.color ?? run.style.foregroundColor
        }

        if case .link = run.kind {
            attributes[.cursor] = NSCursor.pointingHand
        }

        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineBreakMode = .byWordWrapping
        attributes[.paragraphStyle] = paragraphStyle
        return attributes
    }

    private func font(for run: EditorDocsRun) -> NSFont {
        let isBold = run.style.addModifiers & docsStyleBold != 0
        let isItalic = run.style.addModifiers & docsStyleItalic != 0

        switch run.kind {
        case .heading1:
            return NSFont.systemFont(ofSize: 16, weight: .semibold)
        case .heading2:
            return NSFont.systemFont(ofSize: 15, weight: .semibold)
        case .heading3:
            return NSFont.systemFont(ofSize: 14, weight: .semibold)
        case .heading4, .heading5, .heading6:
            return NSFont.systemFont(ofSize: 13, weight: .semibold)
        case .inlineCode, .code, .activeParameter:
            return monospacedFont(size: 12, weight: isBold ? .semibold : .regular, italic: isItalic)
        default:
            return systemFont(size: 12, weight: isBold ? .semibold : .regular, italic: isItalic)
        }
    }

    private func systemFont(size: CGFloat, weight: NSFont.Weight, italic: Bool) -> NSFont {
        let base = NSFont.systemFont(ofSize: size, weight: weight)
        guard italic else { return base }
        return NSFontManager.shared.convert(base, toHaveTrait: .italicFontMask)
    }

    private func monospacedFont(size: CGFloat, weight: NSFont.Weight, italic: Bool) -> NSFont {
        let base = NSFont.monospacedSystemFont(ofSize: size, weight: weight)
        guard italic else { return base }
        return NSFontManager.shared.convert(base, toHaveTrait: .italicFontMask)
    }
}

private struct EditorSelectableDocsTextView: NSViewRepresentable {
    let attributedText: NSAttributedString
    let backgroundColor: NSColor

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView(frame: .zero)
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.borderType = .noBorder
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.contentInsets = NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)

        let textView = NSTextView(frame: .zero)
        textView.isEditable = false
        textView.isSelectable = true
        textView.isRichText = true
        textView.importsGraphics = false
        textView.drawsBackground = false
        textView.textContainerInset = NSSize(width: 10, height: 8)
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.heightTracksTextView = false
        textView.textContainer?.lineFragmentPadding = 0
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.minSize = .zero
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.allowsUndo = false
        textView.linkTextAttributes = [
            .foregroundColor: NSColor.linkColor,
            .underlineStyle: NSUnderlineStyle.single.rawValue
        ]
        textView.textStorage?.setAttributedString(attributedText)

        scrollView.documentView = textView
        context.coordinator.textView = textView
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = context.coordinator.textView else { return }
        textView.textStorage?.setAttributedString(attributedText)
    }

    final class Coordinator {
        weak var textView: NSTextView?
    }
}
