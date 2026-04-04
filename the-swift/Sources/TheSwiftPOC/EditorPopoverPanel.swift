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
            .offset(x: frame.minX, y: frame.minY)
    }
}
