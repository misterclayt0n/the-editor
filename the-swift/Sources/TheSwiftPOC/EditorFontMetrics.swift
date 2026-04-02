import AppKit
import CoreText

struct EditorFontMetrics {
    let font: NSFont
    let cellSize: CGSize
    let ascent: CGFloat
    let descent: CGFloat
    let leading: CGFloat
    let baselineFromBottom: CGFloat

    init(font: NSFont) {
        self.font = font

        let ctFont = font as CTFont
        let ascent = ceil(CTFontGetAscent(ctFont))
        let descent = ceil(CTFontGetDescent(ctFont))
        let leading = ceil(max(CTFontGetLeading(ctFont), 0))

        var scalar = UniChar("M".utf16.first ?? 77)
        var glyph = CGGlyph()
        let hasGlyph = CTFontGetGlyphsForCharacters(ctFont, &scalar, &glyph, 1)

        var advance = CGSize.zero
        if hasGlyph {
            CTFontGetAdvancesForGlyphs(ctFont, .horizontal, &glyph, &advance, 1)
        }

        let cellWidth = ceil(max(advance.width, font.maximumAdvancement.width, 1))
        let glyphHeight = ascent + descent
        let cellHeight = ceil(max(glyphHeight + leading, font.boundingRectForFont.height, 1))
        let verticalPadding = max((cellHeight - glyphHeight) / 2, 0)

        self.cellSize = CGSize(width: cellWidth, height: cellHeight)
        self.ascent = ascent
        self.descent = descent
        self.leading = leading
        self.baselineFromBottom = floor(verticalPadding + descent)
    }
}
