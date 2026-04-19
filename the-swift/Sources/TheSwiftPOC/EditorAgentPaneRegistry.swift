import AppKit
import Foundation
import SwiftUI

@MainActor
final class EditorAgentPaneRegistry {
    private struct Record {
        let store: EditorAgentPanelStore
        let hostModel: EditorAgentPaneHostModel
        let hostingView: NSHostingView<EditorAgentSidebarView>
    }

    private struct VisibleEntry {
        let agentItemID: UInt
        let frame: CGRect
    }

    private unowned let controller: EditorSurfaceController
    private var recordsByAgentItemID: [UInt: Record] = [:]

    private(set) var visibleContentRects: [CGRect] = []

    init(controller: EditorSurfaceController) {
        self.controller = controller
    }

    func reconcile(
        scene: EditorRenderScene?,
        openItems: EditorPaneOpenItemsState,
        in containerView: NSView,
        backgroundColor: NSColor,
        selectionColor: NSColor
    ) {
        let openAgentItemIDs = Set(
            openItems.groups
                .flatMap(\.items)
                .filter { $0.kind == .agent }
                .map(\.itemID)
        )

        for agentItemID in openAgentItemIDs where recordsByAgentItemID[agentItemID] == nil {
            recordsByAgentItemID[agentItemID] = makeRecord(
                agentItemID: agentItemID,
                backgroundColor: backgroundColor,
                selectionColor: selectionColor
            )
        }

        let staleAgentItemIDs = Set(recordsByAgentItemID.keys).subtracting(openAgentItemIDs)
        for agentItemID in staleAgentItemIDs {
            recordsByAgentItemID[agentItemID]?.hostingView.removeFromSuperview()
            recordsByAgentItemID.removeValue(forKey: agentItemID)
            controller.agentSessionSupervisor.releaseAgentItem(agentItemID)
        }

        for record in recordsByAgentItemID.values {
            updateHostModel(record.hostModel, backgroundColor: backgroundColor, selectionColor: selectionColor)
        }

        let activeAgentItemsByPaneID: [UInt: EditorPaneOpenItemRow] = Dictionary(
            uniqueKeysWithValues: openItems.groups.compactMap { group -> (UInt, EditorPaneOpenItemRow)? in
                guard let item = group.items.first(where: { $0.isActive && $0.kind == .agent }) else {
                    return nil
                }
                return (group.paneID, item)
            }
        )

        let visibleEntries: [VisibleEntry] = scene?.panes.compactMap { pane in
            guard pane.kind == .agent,
                  let item = activeAgentItemsByPaneID[pane.paneID]
            else {
                return nil
            }
            let frame = scene?.paneContentRect(for: pane).intersection(containerView.bounds).integral ?? .zero
            guard frame.width > 0, frame.height > 0 else { return nil }
            return VisibleEntry(agentItemID: item.itemID, frame: frame)
        } ?? []

        let visibleEntriesByAgentItemID = Dictionary(uniqueKeysWithValues: visibleEntries.map { ($0.agentItemID, $0) })
        for (agentItemID, record) in recordsByAgentItemID {
            let hostingView = record.hostingView
            if hostingView.superview !== containerView {
                containerView.addSubview(hostingView)
            }

            if let entry = visibleEntriesByAgentItemID[agentItemID] {
                if hostingView.frame != entry.frame {
                    hostingView.frame = entry.frame
                }
                if hostingView.isHidden {
                    hostingView.isHidden = false
                }
            } else {
                if !hostingView.isHidden {
                    hostingView.isHidden = true
                }
            }
        }

        visibleContentRects = visibleEntries.map(\.frame)
        containerView.isHidden = visibleEntries.isEmpty
    }

    private func makeRecord(
        agentItemID: UInt,
        backgroundColor: NSColor,
        selectionColor: NSColor
    ) -> Record {
        let store = EditorAgentPanelStore(
            controller: controller,
            agentItemID: agentItemID,
            supervisor: controller.agentSessionSupervisor
        )
        let hostModel = EditorAgentPaneHostModel()
        updateHostModel(hostModel, backgroundColor: backgroundColor, selectionColor: selectionColor)
        let rootView = EditorAgentSidebarView(store: store, hostModel: hostModel)
        let hostingView = NSHostingView(rootView: rootView)
        hostingView.identifier = NSUserInterfaceItemIdentifier("agent-\(agentItemID)")
        hostingView.sizingOptions = []
        hostingView.isHidden = true
        return Record(store: store, hostModel: hostModel, hostingView: hostingView)
    }

    private func updateHostModel(
        _ hostModel: EditorAgentPaneHostModel,
        backgroundColor: NSColor,
        selectionColor: NSColor
    ) {
        if !hostModel.backgroundColor.isEqual(backgroundColor) {
            hostModel.backgroundColor = backgroundColor
        }
        if !hostModel.selectionColor.isEqual(selectionColor) {
            hostModel.selectionColor = selectionColor
        }
        if hostModel.topScrimHeight != 0 {
            hostModel.topScrimHeight = 0
        }
    }
}
