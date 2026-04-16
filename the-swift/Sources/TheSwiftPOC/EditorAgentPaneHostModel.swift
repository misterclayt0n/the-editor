import AppKit
import SwiftUI

@MainActor
final class EditorAgentPaneHostModel: ObservableObject {
    @Published var backgroundColor: NSColor = .windowBackgroundColor
    @Published var selectionColor: NSColor = .selectedContentBackgroundColor
    @Published var topScrimHeight: CGFloat = 0
}
