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
        let rawAscent = CTFontGetAscent(ctFont)
        let rawDescent = CTFontGetDescent(ctFont)
        let rawLeading = max(CTFontGetLeading(ctFont), 0)

        var scalar = UniChar("M".utf16.first ?? 77)
        var glyph = CGGlyph()
        let hasGlyph = CTFontGetGlyphsForCharacters(ctFont, &scalar, &glyph, 1)

        var advance = CGSize.zero
        if hasGlyph {
            CTFontGetAdvancesForGlyphs(ctFont, .horizontal, &glyph, &advance, 1)
        }

        let faceWidth = max(advance.width, font.maximumAdvancement.width, 1)
        let faceHeight = rawAscent + rawDescent + rawLeading
        let cellWidth = max(round(faceWidth), 1)
        let cellHeight = max(round(faceHeight), 1)
        let halfLineGap = rawLeading / 2
        let faceBaselineFromBottom = halfLineGap + rawDescent
        let baselineFromBottom = round(faceBaselineFromBottom - (cellHeight - faceHeight) / 2)

        self.cellSize = CGSize(width: cellWidth, height: cellHeight)
        self.ascent = rawAscent
        self.descent = rawDescent
        self.leading = rawLeading
        self.baselineFromBottom = baselineFromBottom
    }
}
