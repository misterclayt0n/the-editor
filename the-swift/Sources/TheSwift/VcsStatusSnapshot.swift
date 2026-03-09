import AppKit
import SwiftUI

enum VcsStatusSnapshot: UInt8, Decodable, Equatable {
    case none = 0
    case modified = 1
    case untracked = 2
    case conflict = 3
    case deleted = 4
    case renamed = 5

    var token: String? {
        switch self {
        case .none:
            return nil
        case .modified:
            return "M"
        case .untracked:
            return "?"
        case .conflict:
            return "U"
        case .deleted:
            return "D"
        case .renamed:
            return "R"
        }
    }

    var appKitColor: NSColor {
        switch self {
        case .none:
            return .tertiaryLabelColor
        case .modified:
            return .systemBlue
        case .untracked:
            return .systemGreen
        case .conflict:
            return .systemOrange
        case .deleted:
            return .systemRed
        case .renamed:
            return .systemPurple
        }
    }

    var swiftUIColor: Color {
        Color(nsColor: appKitColor)
    }

    func appKitTextColor(emphasized: Bool) -> NSColor {
        if emphasized {
            return NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.9)
        }
        return appKitColor
    }

    static func neutralCountColor(emphasized: Bool) -> NSColor {
        if emphasized {
            return NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.75)
        }
        return .tertiaryLabelColor
    }
}
