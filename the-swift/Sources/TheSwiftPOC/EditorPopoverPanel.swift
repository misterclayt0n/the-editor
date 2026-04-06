import AppKit
import SwiftUI

struct EditorPopoverPanel<Content: View>: View {
    let frame: CGRect
    let backgroundColor: NSColor
    @ViewBuilder let content: Content

    var body: some View {
        content
            .frame(width: frame.width, height: frame.height)
            .background(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(Color(nsColor: backgroundColor))
            )
            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.10), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.14), radius: 8, y: 3)
            .environment(\.colorScheme, backgroundColor.isLightColor ? .light : .dark)
            .offset(x: frame.minX, y: frame.minY)
    }
}

private extension NSColor {
    var isLightColor: Bool {
        guard let color = usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance > 0.7
    }
}
