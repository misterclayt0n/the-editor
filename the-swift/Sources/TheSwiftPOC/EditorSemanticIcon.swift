import CoreText
import SwiftUI
import TheEditorFFI

enum EditorIconFont {
    nonisolated(unsafe) fileprivate static var didRegister = false

    /// PostScript name from `fc-scan` on bundled `SymbolsNerdFontMono-Regular.ttf`.
    static let postScriptName = "SymbolsNFM"

    static func registerIfNeeded() {
        guard !didRegister else { return }
        didRegister = true
        guard let url = Bundle.module.url(forResource: "SymbolsNerdFontMono-Regular", withExtension: "ttf") else {
            return
        }
        var error: Unmanaged<CFError>?
        _ = CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error)
    }
}

func editorSemanticIconGlyph(icon: String, isDirectory: Bool) -> String {
    icon.withCString { cIcon in
        guard let ptr = the_editor_icon_glyph(cIcon, isDirectory) else { return " " }
        return String(cString: ptr)
    }
}

/// Mirrors `completion_item_icon_text` in `the-term/render.rs`.
func editorCompletionLeadingText(icon: String) -> String {
    if icon.count == 1 {
        return icon
    }
    return editorSemanticIconGlyph(icon: icon, isDirectory: false)
}

struct EditorSemanticIconView: View {
    var iconName: String
    var isDirectory = false
    var size: CGFloat = 11

    var body: some View {
        Text(editorSemanticIconGlyph(icon: iconName, isDirectory: isDirectory))
            .font(.custom(EditorIconFont.postScriptName, size: size))
            .frame(width: max(size * 1.15, size), alignment: .center)
    }
}
