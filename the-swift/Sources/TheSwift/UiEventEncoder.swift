import Foundation
import TheEditorFFIBridge

struct UiEventEnvelope: Encodable {
    let target: String?
    let kind: UiEventKindEnvelope
}

enum UiEventKindEnvelope: Encodable {
    case key(UiKeyEventEnvelope)
    case activate
    case dismiss
    case command(String)

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .key(let keyEvent):
            try container.encode("key", forKey: .type)
            try container.encode(keyEvent, forKey: .data)
        case .activate:
            try container.encode("activate", forKey: .type)
        case .dismiss:
            try container.encode("dismiss", forKey: .type)
        case .command(let command):
            try container.encode("command", forKey: .type)
            try container.encode(command, forKey: .data)
        }
    }
}

struct UiKeyEventEnvelope: Encodable {
    let key: UiKeyEnvelope
    let modifiers: UiModifiersEnvelope
}

enum UiKeyEnvelope: Encodable {
    case char(String)
    case enter
    case escape
    case tab
    case backspace
    case delete
    case up
    case down
    case left
    case right
    case home
    case end
    case pageUp
    case pageDown
    case unknown(String)

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .char(let value):
            try container.encode("char", forKey: .type)
            try container.encode(value, forKey: .data)
        case .enter:
            try container.encode("enter", forKey: .type)
        case .escape:
            try container.encode("escape", forKey: .type)
        case .tab:
            try container.encode("tab", forKey: .type)
        case .backspace:
            try container.encode("backspace", forKey: .type)
        case .delete:
            try container.encode("delete", forKey: .type)
        case .up:
            try container.encode("up", forKey: .type)
        case .down:
            try container.encode("down", forKey: .type)
        case .left:
            try container.encode("left", forKey: .type)
        case .right:
            try container.encode("right", forKey: .type)
        case .home:
            try container.encode("home", forKey: .type)
        case .end:
            try container.encode("end", forKey: .type)
        case .pageUp:
            try container.encode("page_up", forKey: .type)
        case .pageDown:
            try container.encode("page_down", forKey: .type)
        case .unknown(let value):
            try container.encode("unknown", forKey: .type)
            try container.encode(value, forKey: .data)
        }
    }
}

struct UiModifiersEnvelope: Encodable {
    let ctrl: Bool
    let alt: Bool
    let shift: Bool
    let meta: Bool
}

enum UiEventEncoder {
    static func encode(_ event: UiEventEnvelope) -> String? {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        guard let data = try? encoder.encode(event) else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    static func uiKey(from keyEvent: KeyEvent) -> UiKeyEnvelope? {
        guard let kind = KeyKind(rawValue: keyEvent.kind) else {
            return nil
        }
        switch kind {
        case .char:
            guard let scalar = UnicodeScalar(keyEvent.codepoint) else {
                return nil
            }
            return .char(String(scalar))
        case .enter, .numpadEnter:
            return .enter
        case .escape:
            return .escape
        case .tab:
            return .tab
        case .backspace:
            return .backspace
        case .delete:
            return .delete
        case .home:
            return .home
        case .end:
            return .end
        case .pageUp:
            return .pageUp
        case .pageDown:
            return .pageDown
        case .left:
            return .left
        case .right:
            return .right
        case .up:
            return .up
        case .down:
            return .down
        case .f1, .f2, .f3, .f4, .f5, .f6, .f7, .f8, .f9, .f10, .f11, .f12, .insert, .other:
            return nil
        }
    }

    static func uiModifiers(from modifiers: UInt8) -> UiModifiersEnvelope {
        UiModifiersEnvelope(
            ctrl: (modifiers & 0b0000_0001) != 0,
            alt: (modifiers & 0b0000_0010) != 0,
            shift: (modifiers & 0b0000_0100) != 0,
            meta: false
        )
    }

    static func keyEventEnvelope(from keyEvent: KeyEvent) -> UiEventEnvelope? {
        guard let key = uiKey(from: keyEvent) else {
            return nil
        }
        return UiEventEnvelope(
            target: nil,
            kind: .key(
                UiKeyEventEnvelope(
                    key: key,
                    modifiers: uiModifiers(from: keyEvent.modifiers)
                )
            )
        )
    }
}
