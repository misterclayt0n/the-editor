import AppKit
import CoreText
import SwiftUI

enum FontLoader {
    /// The PostScript name of the loaded buffer font (e.g. Iosevka), or nil if
    /// falling back to the system monospaced font.
    private(set) static var bufferFontName: String?

    static func loadBufferFont(size: CGFloat) -> (font: Font, nsFont: NSFont, cellSize: CGSize) {
        if let url = Bundle.module.url(forResource: "Iosevka-Regular", withExtension: "ttc") {
            _ = CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            if let postScript = firstPostScriptName(from: url),
               let nsFont = NSFont(name: postScript, size: size) {
                bufferFontName = postScript
                let cellSize = measureCellSize(font: nsFont)
                // Use a concrete CTFont to avoid runtime name lookup fallback in
                // Canvas text rendering paths.
                return (Font(nsFont as CTFont), nsFont, cellSize)
            }
        }

        let fallback = NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        return (Font(fallback as CTFont), fallback, measureCellSize(font: fallback))
    }

    /// Returns the buffer font at the given size/weight, falling back to the
    /// system monospaced font if Iosevka is not available.
    static func bufferNSFont(size: CGFloat, weight: NSFont.Weight = .regular) -> NSFont {
        if let name = bufferFontName, let font = NSFont(name: name, size: size) {
            if weight != .regular {
                return NSFontManager.shared.convert(font, toHaveTrait: weight == .bold || weight == .semibold ? .boldFontMask : [])
            }
            return font
        }
        return NSFont.monospacedSystemFont(ofSize: size, weight: weight)
    }

    /// SwiftUI `Font` version of the buffer font.
    static func bufferFont(size: CGFloat, weight: NSFont.Weight = .regular) -> Font {
        Font(bufferNSFont(size: size, weight: weight) as CTFont)
    }

    /// UI font family for chrome/popups/panels (system proportional).
    static func uiNSFont(size: CGFloat, weight: NSFont.Weight = .regular) -> NSFont {
        NSFont.systemFont(ofSize: size, weight: weight)
    }

    static func uiFont(size: CGFloat, weight: NSFont.Weight = .regular) -> Font {
        Font(uiNSFont(size: size, weight: weight) as CTFont)
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
