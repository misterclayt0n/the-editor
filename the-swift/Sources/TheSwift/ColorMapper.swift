import SwiftUI
import TheEditorFFIBridge

enum ColorMapper {
    static func color(from color: TheEditorFFIBridge.Color) -> SwiftUI.Color? {
        switch color.kind {
        case 0:
            return nil
        case 1:
            let palette: [SwiftUI.Color] = [
                .black, .red, .green, .yellow, .blue, .purple, .cyan, .gray,
                .red.opacity(0.8), .green.opacity(0.8), .yellow.opacity(0.8),
                .blue.opacity(0.8), .purple.opacity(0.8), .cyan.opacity(0.8),
                .gray.opacity(0.9), .white
            ]
            let idx = Int(color.value)
            return (idx >= 0 && idx < palette.count) ? palette[idx] : SwiftUI.Color.white
        case 2:
            let r = Double((color.value >> 16) & 0xFF) / 255.0
            let g = Double((color.value >> 8) & 0xFF) / 255.0
            let b = Double(color.value & 0xFF) / 255.0
            return SwiftUI.Color(red: r, green: g, blue: b)
        case 3:
            return xterm256Color(index: Int(color.value))
        default:
            return nil
        }
    }

    private static func xterm256Color(index: Int) -> SwiftUI.Color? {
        if index < 0 {
            return nil
        }

        if index < 16 {
            let palette: [SwiftUI.Color] = [
                .black, .red, .green, .yellow, .blue, .purple, .cyan, .gray,
                .red.opacity(0.8), .green.opacity(0.8), .yellow.opacity(0.8),
                .blue.opacity(0.8), .purple.opacity(0.8), .cyan.opacity(0.8),
                .gray.opacity(0.9), .white
            ]
            return palette[index]
        }

        if index >= 232 {
            let level = Double(index - 232) / 23.0
            return SwiftUI.Color(white: level)
        }

        let idx = index - 16
        let r = idx / 36
        let g = (idx % 36) / 6
        let b = idx % 6
        func component(_ value: Int) -> Double {
            let levels: [Double] = [0.0, 0.37, 0.58, 0.74, 0.87, 1.0]
            return levels[min(max(value, 0), levels.count - 1)]
        }

        return SwiftUI.Color(red: component(r), green: component(g), blue: component(b))
    }
}
