import AppKit
import SwiftUI

enum FontLoader {
    static func loadIosevka(size: CGFloat) -> (font: Font, nsFont: NSFont, cellSize: CGSize) {
        if let url = Bundle.module.url(forResource: "Iosevka-Regular", withExtension: "ttc") {
            _ = CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            if let postScript = firstPostScriptName(from: url),
               let nsFont = NSFont(name: postScript, size: size) {
                let cellSize = measureCellSize(font: nsFont)
                return (Font.custom(postScript, size: size), nsFont, cellSize)
            }
        }

        let fallback = NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        return (Font.system(size: size, weight: .regular, design: .monospaced), fallback, measureCellSize(font: fallback))
    }

    private static func firstPostScriptName(from url: URL) -> String? {
        guard let descriptors = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor],
              let first = descriptors.first
        else {
            return nil
        }
        return CTFontDescriptorCopyAttribute(first, kCTFontNameAttribute) as? String
    }

    private static func measureCellSize(font: NSFont) -> CGSize {
        let attributes: [NSAttributedString.Key: Any] = [.font: font]
        let width = ("M" as NSString).size(withAttributes: attributes).width
        let height = font.ascender - font.descender + font.leading
        return CGSize(width: ceil(width), height: ceil(height))
    }
}
