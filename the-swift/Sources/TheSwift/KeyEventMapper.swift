import AppKit
import TheEditorFFIBridge

enum KeyKind: UInt8 {
    case char = 0
    case enter = 1
    case numpadEnter = 2
    case escape = 3
    case backspace = 4
    case tab = 5
    case delete = 6
    case insert = 7
    case home = 8
    case end = 9
    case pageUp = 10
    case pageDown = 11
    case left = 12
    case right = 13
    case up = 14
    case down = 15
    case f1 = 16
    case f2 = 17
    case f3 = 18
    case f4 = 19
    case f5 = 20
    case f6 = 21
    case f7 = 22
    case f8 = 23
    case f9 = 24
    case f10 = 25
    case f11 = 26
    case f12 = 27
    case other = 255
}

struct KeyEventMapper {
    static func map(event: NSEvent) -> KeyEvent? {
        if event.modifierFlags.contains(.command) {
            return nil
        }

        let modifiers = modifiersBits(from: event)
        if let special = specialKey(for: event.keyCode) {
            return KeyEvent(kind: special.rawValue, codepoint: 0, modifiers: modifiers)
        }

        if let chars = event.charactersIgnoringModifiers,
           let scalar = chars.unicodeScalars.first {
            return KeyEvent(kind: KeyKind.char.rawValue, codepoint: scalar.value, modifiers: modifiers)
        }

        return nil
    }

    private static func modifiersBits(from event: NSEvent) -> UInt8 {
        var bits: UInt8 = 0
        if event.modifierFlags.contains(.control) {
            bits |= 0b0000_0001
        }
        if event.modifierFlags.contains(.option) {
            bits |= 0b0000_0010
        }
        if event.modifierFlags.contains(.shift) {
            bits |= 0b0000_0100
        }
        return bits
    }

    private static func specialKey(for keyCode: UInt16) -> KeyKind? {
        switch keyCode {
        case 36: return .enter
        case 76: return .numpadEnter
        case 48: return .tab
        case 51: return .backspace
        case 117: return .delete
        case 53: return .escape
        case 123: return .left
        case 124: return .right
        case 125: return .down
        case 126: return .up
        case 115: return .home
        case 119: return .end
        case 116: return .pageUp
        case 121: return .pageDown
        case 122: return .f1
        case 120: return .f2
        case 99: return .f3
        case 118: return .f4
        case 96: return .f5
        case 97: return .f6
        case 98: return .f7
        case 100: return .f8
        case 101: return .f9
        case 109: return .f10
        case 103: return .f11
        case 111: return .f12
        default: return nil
        }
    }
}
