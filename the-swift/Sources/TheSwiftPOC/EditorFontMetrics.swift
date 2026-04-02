import AppKit
import CoreText

struct EditorSurfaceMetrics: Hashable {
    let backingScale: CGFloat
    let cellWidthPx: Int
    let cellHeightPx: Int
    let cellBaselinePx: Int
    let underlinePositionPx: Int
    let underlineThicknessPx: Int
    let cursorThicknessPx: Int

    var cellSizePoints: CGSize {
        CGSize(
            width: CGFloat(cellWidthPx) / max(backingScale, 1),
            height: CGFloat(cellHeightPx) / max(backingScale, 1)
        )
    }

    var baselineFromBottomPoints: CGFloat {
        CGFloat(cellBaselinePx) / max(backingScale, 1)
    }

    var underlinePositionPoints: CGFloat {
        CGFloat(underlinePositionPx) / max(backingScale, 1)
    }

    var underlineThicknessPoints: CGFloat {
        CGFloat(underlineThicknessPx) / max(backingScale, 1)
    }

    var cursorThicknessPoints: CGFloat {
        CGFloat(cursorThicknessPx) / max(backingScale, 1)
    }
}

struct EditorSurfaceConfiguration: Hashable {
    let widthPx: Int
    let heightPx: Int
    let metrics: EditorSurfaceMetrics
}

struct EditorFontMetrics {
    let font: NSFont
    let cellSize: CGSize
    let ascent: CGFloat
    let descent: CGFloat
    let leading: CGFloat
    let baselineFromBottom: CGFloat
    let underlinePositionFromBottom: CGFloat
    let underlineThickness: CGFloat

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

        let rawUnderlinePosition = CTFontGetUnderlinePosition(ctFont)
        let rawUnderlineThickness = max(CTFontGetUnderlineThickness(ctFont), 1)
        let underlinePositionFromBottom = max(baselineFromBottom + rawUnderlinePosition, 0)

        self.cellSize = CGSize(width: cellWidth, height: cellHeight)
        self.ascent = rawAscent
        self.descent = rawDescent
        self.leading = rawLeading
        self.baselineFromBottom = baselineFromBottom
        self.underlinePositionFromBottom = underlinePositionFromBottom
        self.underlineThickness = rawUnderlineThickness
    }

    func surfaceConfiguration(viewSize: CGSize, backingScale: CGFloat) -> EditorSurfaceConfiguration {
        let scale = max(backingScale, 1)
        let widthPx = max(Int(floor(viewSize.width * scale)), 1)
        let heightPx = max(Int(floor(viewSize.height * scale)), 1)
        let metrics = EditorSurfaceMetrics(
            backingScale: scale,
            cellWidthPx: max(Int(round(cellSize.width * scale)), 1),
            cellHeightPx: max(Int(round(cellSize.height * scale)), 1),
            cellBaselinePx: max(Int(round(baselineFromBottom * scale)), 1),
            underlinePositionPx: max(Int(round(underlinePositionFromBottom * scale)), 0),
            underlineThicknessPx: max(Int(round(underlineThickness * scale)), 1),
            cursorThicknessPx: max(Int(round(2 * scale)), 1)
        )
        return EditorSurfaceConfiguration(widthPx: widthPx, heightPx: heightPx, metrics: metrics)
    }
}
