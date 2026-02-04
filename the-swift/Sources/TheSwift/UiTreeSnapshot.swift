import Foundation
import SwiftUI

struct UiTreeSnapshot: Decodable {
    let root: UiNodeSnapshot
    let overlays: [UiNodeSnapshot]
    let focus: UiFocusSnapshot?

    static let empty = UiTreeSnapshot(
        root: .container(.empty),
        overlays: [],
        focus: nil
    )
}

indirect enum UiNodeSnapshot: Decodable {
    case panel(UiPanelSnapshot)
    case container(UiContainerSnapshot)
    case text(UiTextSnapshot)
    case list(UiListSnapshot)
    case input(UiInputSnapshot)
    case divider(UiDividerSnapshot)
    case spacer(UiSpacerSnapshot)
    case tooltip(UiTooltipSnapshot)
    case statusBar(UiStatusBarSnapshot)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = (try? container.decode(String.self, forKey: .type)) ?? ""
        switch type {
        case "panel":
            self = .panel(try container.decode(UiPanelSnapshot.self, forKey: .data))
        case "container":
            self = .container(try container.decode(UiContainerSnapshot.self, forKey: .data))
        case "text":
            self = .text(try container.decode(UiTextSnapshot.self, forKey: .data))
        case "list":
            self = .list(try container.decode(UiListSnapshot.self, forKey: .data))
        case "input":
            self = .input(try container.decode(UiInputSnapshot.self, forKey: .data))
        case "divider":
            self = .divider(try container.decode(UiDividerSnapshot.self, forKey: .data))
        case "spacer":
            self = .spacer(try container.decode(UiSpacerSnapshot.self, forKey: .data))
        case "tooltip":
            self = .tooltip(try container.decode(UiTooltipSnapshot.self, forKey: .data))
        case "status_bar":
            self = .statusBar(try container.decode(UiStatusBarSnapshot.self, forKey: .data))
        default:
            self = .unknown
        }
    }
}

struct UiContainerSnapshot: Decodable {
    let id: String?
    let layout: UiLayoutSnapshot
    let children: [UiNodeSnapshot]
    let style: UiStyleSnapshot
    let constraints: UiConstraintsSnapshot

    static let empty = UiContainerSnapshot(
        id: nil,
        layout: .stack(axis: .vertical, gap: 0),
        children: [],
        style: .fallback,
        constraints: .fallback
    )
}

struct UiPanelSnapshot: Decodable {
    let id: String
    let title: String?
    let intent: LayoutIntentSnapshot
    let style: UiStyleSnapshot
    let constraints: UiConstraintsSnapshot
    let layer: UiLayerSnapshot
    let child: UiNodeSnapshot
}

struct UiTextSnapshot: Decodable {
    let id: String?
    let content: String
    let style: UiStyleSnapshot
    let maxLines: UInt16?
    let clip: Bool

    private enum CodingKeys: String, CodingKey {
        case id
        case content
        case style
        case maxLines
        case clip
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try? container.decode(String.self, forKey: .id)
        content = try container.decode(String.self, forKey: .content)
        style = (try? container.decode(UiStyleSnapshot.self, forKey: .style)) ?? .fallback
        maxLines = try? container.decode(UInt16.self, forKey: .maxLines)
        clip = (try? container.decode(Bool.self, forKey: .clip)) ?? true
    }
}

struct UiListSnapshot: Decodable {
    let id: String
    let items: [UiListItemSnapshot]
    let selected: Int?
    let scroll: Int
    let fillWidth: Bool
    let style: UiStyleSnapshot
    let maxVisible: Int?
    let clip: Bool

    private enum CodingKeys: String, CodingKey {
        case id
        case items
        case selected
        case scroll
        case fillWidth
        case style
        case maxVisible
        case clip
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        items = (try? container.decode([UiListItemSnapshot].self, forKey: .items)) ?? []
        selected = try? container.decode(Int.self, forKey: .selected)
        scroll = (try? container.decode(Int.self, forKey: .scroll)) ?? 0
        fillWidth = (try? container.decode(Bool.self, forKey: .fillWidth)) ?? false
        style = (try? container.decode(UiStyleSnapshot.self, forKey: .style)) ?? .fallback
        maxVisible = try? container.decode(Int.self, forKey: .maxVisible)
        clip = (try? container.decode(Bool.self, forKey: .clip)) ?? true
    }
}

struct UiListItemSnapshot: Decodable {
    let title: String
    let subtitle: String?
    let description: String?
    let shortcut: String?
    let badge: String?
    let leadingIcon: String?
    let leadingColor: UiColorSnapshot?
    let symbols: [String]
    let emphasis: Bool
    let action: String?

    private enum CodingKeys: String, CodingKey {
        case title
        case subtitle
        case description
        case shortcut
        case badge
        case leadingIcon
        case leadingColor
        case symbols
        case emphasis
        case action
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        title = try container.decode(String.self, forKey: .title)
        subtitle = try? container.decode(String.self, forKey: .subtitle)
        description = try? container.decode(String.self, forKey: .description)
        shortcut = try? container.decode(String.self, forKey: .shortcut)
        badge = try? container.decode(String.self, forKey: .badge)
        leadingIcon = try? container.decode(String.self, forKey: .leadingIcon)
        leadingColor = try? container.decode(UiColorSnapshot.self, forKey: .leadingColor)
        symbols = (try? container.decode([String].self, forKey: .symbols)) ?? []
        emphasis = (try? container.decode(Bool.self, forKey: .emphasis)) ?? false
        action = try? container.decode(String.self, forKey: .action)
    }
}

struct UiInputSnapshot: Decodable {
    let id: String
    let value: String
    let placeholder: String?
    let cursor: Int
    let style: UiStyleSnapshot
}

struct UiDividerSnapshot: Decodable {
    let id: String?
}

struct UiSpacerSnapshot: Decodable {
    let id: String?
    let size: UInt16
}

struct UiTooltipSnapshot: Decodable {
    let id: String?
    let target: String?
    let placement: LayoutIntentSnapshot
    let content: String
    let style: UiStyleSnapshot
}

struct UiStatusBarSnapshot: Decodable {
    let id: String?
    let left: String
    let center: String
    let right: String
    let style: UiStyleSnapshot
}

enum UiAxisSnapshot: String, Decodable {
    case horizontal
    case vertical
}

enum UiLayoutSnapshot: Decodable {
    case stack(axis: UiAxisSnapshot, gap: UInt16)
    case split(axis: UiAxisSnapshot, ratios: [UInt16])
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    private struct StackData: Decodable {
        let axis: UiAxisSnapshot
        let gap: UInt16
    }

    private struct SplitData: Decodable {
        let axis: UiAxisSnapshot
        let ratios: [UInt16]
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = (try? container.decode(String.self, forKey: .type)) ?? ""
        switch type {
        case "stack":
            let data = try container.decode(StackData.self, forKey: .data)
            self = .stack(axis: data.axis, gap: data.gap)
        case "split":
            let data = try container.decode(SplitData.self, forKey: .data)
            self = .split(axis: data.axis, ratios: data.ratios)
        default:
            self = .unknown
        }
    }
}

enum LayoutIntentSnapshot: Decodable {
    case floating
    case bottom
    case top
    case sidebarLeft
    case sidebarRight
    case fullscreen
    case custom(String)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = (try? container.decode(String.self, forKey: .type)) ?? ""
        switch type {
        case "floating":
            self = .floating
        case "bottom":
            self = .bottom
        case "top":
            self = .top
        case "sidebar_left":
            self = .sidebarLeft
        case "sidebar_right":
            self = .sidebarRight
        case "fullscreen":
            self = .fullscreen
        case "custom":
            let data = (try? container.decode(String.self, forKey: .data)) ?? ""
            self = .custom(data)
        default:
            self = .unknown
        }
    }
}

enum UiEmphasisSnapshot: String, Decodable {
    case normal
    case muted
    case strong
}

enum UiRadiusSnapshot: String, Decodable {
    case none
    case small
    case medium
    case large
}

enum UiAlignSnapshot: String, Decodable {
    case start
    case center
    case end
    case stretch
}

struct UiAlignPairSnapshot: Decodable {
    let horizontal: UiAlignSnapshot
    let vertical: UiAlignSnapshot
}

struct UiInsetsSnapshot: Decodable {
    let left: UInt16
    let right: UInt16
    let top: UInt16
    let bottom: UInt16

    static let zero = UiInsetsSnapshot(left: 0, right: 0, top: 0, bottom: 0)
}

struct UiConstraintsSnapshot: Decodable {
    let minWidth: UInt16?
    let maxWidth: UInt16?
    let minHeight: UInt16?
    let maxHeight: UInt16?
    let padding: UiInsetsSnapshot
    let align: UiAlignPairSnapshot

    static let fallback = UiConstraintsSnapshot(
        minWidth: nil,
        maxWidth: nil,
        minHeight: nil,
        maxHeight: nil,
        padding: .zero,
        align: UiAlignPairSnapshot(horizontal: .start, vertical: .start)
    )
}

enum UiLayerSnapshot: String, Decodable {
    case background
    case overlay
    case tooltip
}

enum UiColorTokenSnapshot: String, Decodable {
    case text
    case mutedText
    case panelBg
    case panelBorder
    case accent
    case selectedBg
    case selectedText
    case divider
    case placeholder
}

enum UiColorSnapshot: Decodable {
    case token(UiColorTokenSnapshot)
    case value(ColorSnapshot)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = (try? container.decode(String.self, forKey: .type)) ?? ""
        switch type {
        case "token":
            let token = try container.decode(UiColorTokenSnapshot.self, forKey: .data)
            self = .token(token)
        case "value":
            let value = try container.decode(ColorSnapshot.self, forKey: .data)
            self = .value(value)
        default:
            self = .unknown
        }
    }
}

struct UiStyleSnapshot: Decodable {
    let fg: UiColorSnapshot?
    let bg: UiColorSnapshot?
    let border: UiColorSnapshot?
    let accent: UiColorSnapshot?
    let emphasis: UiEmphasisSnapshot
    let radius: UiRadiusSnapshot

    static let fallback = UiStyleSnapshot(
        fg: nil,
        bg: nil,
        border: nil,
        accent: nil,
        emphasis: .normal,
        radius: .none
    )
}

struct UiFocusSnapshot: Decodable {
    let id: String
    let kind: UiFocusKindSnapshot
    let cursor: Int?

    private enum CodingKeys: String, CodingKey {
        case id
        case kind
        case cursor
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        kind = (try? container.decode(UiFocusKindSnapshot.self, forKey: .kind)) ?? .input
        cursor = try? container.decode(Int.self, forKey: .cursor)
    }
}

enum UiFocusKindSnapshot: Decodable {
    case input
    case list
    case panel
    case custom(String)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = (try? container.decode(String.self, forKey: .type)) ?? ""
        switch type {
        case "input":
            self = .input
        case "list":
            self = .list
        case "panel":
            self = .panel
        case "custom":
            let data = (try? container.decode(String.self, forKey: .data)) ?? ""
            self = .custom(data)
        default:
            self = .unknown
        }
    }
}

enum ColorSnapshot: Decodable {
    case reset
    case black
    case red
    case green
    case yellow
    case blue
    case magenta
    case cyan
    case gray
    case lightRed
    case lightGreen
    case lightYellow
    case lightBlue
    case lightMagenta
    case lightCyan
    case lightGray
    case white
    case rgb(UInt8, UInt8, UInt8)
    case indexed(UInt8)
    case unknown

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = (try? container.decode(String.self, forKey: .type)) ?? ""
        switch type {
        case "reset":
            self = .reset
        case "black":
            self = .black
        case "red":
            self = .red
        case "green":
            self = .green
        case "yellow":
            self = .yellow
        case "blue":
            self = .blue
        case "magenta":
            self = .magenta
        case "cyan":
            self = .cyan
        case "gray":
            self = .gray
        case "light_red":
            self = .lightRed
        case "light_green":
            self = .lightGreen
        case "light_yellow":
            self = .lightYellow
        case "light_blue":
            self = .lightBlue
        case "light_magenta":
            self = .lightMagenta
        case "light_cyan":
            self = .lightCyan
        case "light_gray":
            self = .lightGray
        case "white":
            self = .white
        case "rgb":
            let rgb = (try? container.decode([UInt8].self, forKey: .data)) ?? []
            if rgb.count == 3 {
                self = .rgb(rgb[0], rgb[1], rgb[2])
            } else {
                self = .unknown
            }
        case "indexed":
            let value = (try? container.decode(UInt8.self, forKey: .data)) ?? 0
            self = .indexed(value)
        default:
            self = .unknown
        }
    }
}
