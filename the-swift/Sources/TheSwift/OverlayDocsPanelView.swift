import SwiftUI

struct OverlayDocsPanelView: View {
    let docs: String
    let width: CGFloat
    let height: CGFloat
    let languageHint: String
    let theme: PopupChromeTheme

    var body: some View {
        CompletionDocsTextView(
            docs: docs,
            width: width,
            height: height,
            languageHint: languageHint,
            theme: theme.docsTheme
        )
        .frame(width: width, height: height)
        .popupBackground(theme: theme, cornerRadius: 8)
    }
}
