import Foundation

enum FontZoomAction {
    case increase
    case decrease
    case reset

    func adjustedBufferPointSize(from current: CGFloat) -> CGFloat {
        switch self {
        case .increase:
            return FontZoomLimits.clamp(current + FontZoomLimits.stepPoints)
        case .decrease:
            return FontZoomLimits.clamp(current - FontZoomLimits.stepPoints)
        case .reset:
            return FontZoomLimits.defaultBufferPointSize
        }
    }

    var terminalBindingAction: String {
        switch self {
        case .increase:
            return "increase_font_size:\(Int(FontZoomLimits.stepPoints))"
        case .decrease:
            return "decrease_font_size:\(Int(FontZoomLimits.stepPoints))"
        case .reset:
            return "reset_font_size"
        }
    }
}

enum FontZoomLimits {
    // Keep buffer zoom semantics aligned with Ghostty terminal zoom.
    static let minPointSize: CGFloat = 1
    static let maxPointSize: CGFloat = 255
    static let defaultBufferPointSize: CGFloat = 14
    static let stepPoints: CGFloat = 1

    static func clamp(_ value: CGFloat) -> CGFloat {
        min(max(value, minPointSize), maxPointSize)
    }
}
