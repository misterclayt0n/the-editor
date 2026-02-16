import SwiftUI

struct OverlayDocsPanelView: View {
    let docs: String
    let width: CGFloat
    let height: CGFloat
    let languageHint: String

    var body: some View {
        CompletionDocsTextView(
            docs: docs,
            width: width,
            height: height,
            languageHint: languageHint
        )
        .frame(width: width, height: height)
        .glassBackground(cornerRadius: 8)
    }
}
