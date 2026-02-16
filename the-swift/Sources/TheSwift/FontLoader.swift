import AppKit
import SwiftUI

enum FontLoader {
    /// The PostScript name of the loaded editor font (e.g. Iosevka), or nil if
    /// falling back to the system monospaced font.
    private(set) static var editorFontName: String?

    static func loadIosevka(size: CGFloat) -> (font: Font, nsFont: NSFont, cellSize: CGSize) {
        if let url = Bundle.module.url(forResource: "Iosevka-Regular", withExtension: "ttc") {
            _ = CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            if let postScript = firstPostScriptName(from: url),
               let nsFont = NSFont(name: postScript, size: size) {
                editorFontName = postScript
                let cellSize = measureCellSize(font: nsFont)
                return (Font.custom(postScript, size: size), nsFont, cellSize)
            }
        }

        let fallback = NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        return (Font.system(size: size, weight: .regular, design: .monospaced), fallback, measureCellSize(font: fallback))
    }

    /// Returns the editor font at the given size/weight, falling back to the
    /// system monospaced font if Iosevka is not available.
    static func editorNSFont(size: CGFloat, weight: NSFont.Weight = .regular) -> NSFont {
        if let name = editorFontName, let font = NSFont(name: name, size: size) {
            if weight != .regular {
                return NSFontManager.shared.convert(font, toHaveTrait: weight == .bold || weight == .semibold ? .boldFontMask : [])
            }
            return font
        }
        return NSFont.monospacedSystemFont(ofSize: size, weight: weight)
    }

    /// SwiftUI `Font` version of the editor font.
    static func editorFont(size: CGFloat) -> Font {
        if let name = editorFontName {
            return Font.custom(name, size: size)
        }
        return Font.system(size: size, weight: .regular, design: .monospaced)
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
