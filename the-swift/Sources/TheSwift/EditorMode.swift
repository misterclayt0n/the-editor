import Foundation

enum EditorMode: UInt8 {
    case normal = 0
    case insert = 1
    case select = 2
    case command = 3

    var isTextInput: Bool {
        switch self {
        case .insert, .command:
            return true
        case .normal, .select:
            return false
        }
    }
}
