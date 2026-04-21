import AppKit
import Foundation
import SwiftUI

@MainActor
final class EditorAgentSidebarCoordinator {
    private static let sidebarAgentItemID: UInt = .max

    let store: EditorAgentPanelStore
    let hostModel: EditorAgentPaneHostModel

    init(controller: EditorSurfaceController) {
        self.store = EditorAgentPanelStore(
            controller: controller,
            agentItemID: Self.sidebarAgentItemID,
            supervisor: controller.agentSessionSupervisor
        )
        self.hostModel = EditorAgentPaneHostModel()
    }

    var agentItemID: UInt {
        Self.sidebarAgentItemID
    }

    func updateAppearance(backgroundColor: NSColor, selectionColor: NSColor, topScrimHeight: CGFloat) {
        if !hostModel.backgroundColor.isEqual(backgroundColor) {
            hostModel.backgroundColor = backgroundColor
        }
        if !hostModel.selectionColor.isEqual(selectionColor) {
            hostModel.selectionColor = selectionColor
        }
        if abs(hostModel.topScrimHeight - topScrimHeight) > 0.5 {
            hostModel.topScrimHeight = topScrimHeight
        }
    }

    func preferredInlineModelSelection() -> (provider: String, modelID: String)? {
        guard let footerInfo = store.footerInfo,
              let provider = footerInfo.modelProvider,
              let modelID = footerInfo.modelID,
              !provider.isEmpty,
              !modelID.isEmpty,
              Self.isAllowedInlineModel(provider: provider, modelID: modelID) else {
            return nil
        }
        return (provider, modelID)
    }

    func availableInlineModels() -> [EditorAgentModel] {
        store.models.filter { Self.isAllowedInlineModel(provider: $0.provider, modelID: $0.id) }
    }

    func sessionPathForInlineModel(provider: String, modelID: String) -> String? {
        guard store.models.contains(where: { $0.provider == provider && $0.id == modelID }) else {
            return nil
        }
        return store.sessionPath
    }

    func storeForSelectionRouting() -> EditorAgentPanelStore {
        store.startIfNeeded()
        return store
    }

    private static func isAllowedInlineModel(provider: String, modelID: String) -> Bool {
        let normalizedProvider = provider.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let normalizedModelID = modelID.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if normalizedProvider == "opencode-go" {
            return normalizedModelID.contains("kimi")
        }
        if normalizedProvider == "cursor" || normalizedProvider == "cursor-agent" {
            return normalizedModelID.contains("composer")
        }
        return false
    }
}
