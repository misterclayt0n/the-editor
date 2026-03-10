import AppKit
import Foundation
import SwiftUI
import UserNotifications

enum EditorNotificationPlatformSupport {
    static var supportsUserNotifications: Bool {
        Bundle.main.bundleURL.pathExtension == "app" && Bundle.main.bundleIdentifier != nil
    }
}

enum EditorNotificationSeverity: String, Decodable, Equatable {
    case info
    case warning
    case error

    var swiftUIColor: Color {
        Color(nsColor: nsColor)
    }

    var nsColor: NSColor {
        switch self {
        case .info:
            return .controlAccentColor
        case .warning:
            return .systemOrange
        case .error:
            return .systemRed
        }
    }

    var symbolName: String {
        switch self {
        case .info:
            return "info.circle.fill"
        case .warning:
            return "exclamationmark.triangle.fill"
        case .error:
            return "xmark.octagon.fill"
        }
    }
}

enum EditorNotificationSourceKind: Equatable {
    case editor
    case terminal
}

enum EditorNotificationSurfaceTarget: Equatable {
    case buffer(UInt64)
    case terminal(UInt64)
}

struct EditorNotificationRoute: OptionSet, Equatable {
    let rawValue: Int

    static let banner = EditorNotificationRoute(rawValue: 1 << 0)
    static let tab = EditorNotificationRoute(rawValue: 1 << 1)
    static let system = EditorNotificationRoute(rawValue: 1 << 2)
}

struct EditorNotificationPayload: Equatable {
    let title: String
    let body: String?
    let sourceLabel: String?
    let severity: EditorNotificationSeverity
    let sourceKind: EditorNotificationSourceKind
    let surfaceTarget: EditorNotificationSurfaceTarget?
    let route: EditorNotificationRoute
}

struct EditorNotificationBannerSnapshot: Identifiable, Equatable {
    let id: UUID
    let title: String
    let body: String?
    let sourceLabel: String?
    let severity: EditorNotificationSeverity
    let sourceKind: EditorNotificationSourceKind
    let expiresAt: Date
}

struct EditorNotificationTabState: Equatable {
    let unreadCount: Int
    let highestSeverity: EditorNotificationSeverity?

    static let none = EditorNotificationTabState(unreadCount: 0, highestSeverity: nil)

    var isVisible: Bool {
        unreadCount > 0 && highestSeverity != nil
    }

    func appending(severity: EditorNotificationSeverity) -> EditorNotificationTabState {
        EditorNotificationTabState(
            unreadCount: unreadCount + 1,
            highestSeverity: Self.maxSeverity(highestSeverity, severity),
        )
    }

    private static func maxSeverity(
        _ lhs: EditorNotificationSeverity?,
        _ rhs: EditorNotificationSeverity
    ) -> EditorNotificationSeverity {
        guard let lhs else { return rhs }
        switch (lhs, rhs) {
        case (.error, _), (_, .error):
            return .error
        case (.warning, _), (_, .warning):
            return .warning
        default:
            return .info
        }
    }
}

struct MessageSnapshotPayload: Decodable {
    let active: MessagePayload?
    let oldestSeq: UInt64
    let latestSeq: UInt64
}

struct MessagePayload: Decodable {
    let id: UInt64
    let level: EditorNotificationSeverity
    let source: String?
    let text: String
}

struct MessageEventPayload: Decodable {
    enum Kind: String, Decodable {
        case published
        case dismissed
        case cleared
    }

    let seq: UInt64
    let kind: Kind
    let message: MessagePayload?
    let dismissedId: UInt64?

    private enum CodingKeys: String, CodingKey {
        case seq
        case kind
        case message
        case id
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        seq = try container.decode(UInt64.self, forKey: .seq)
        kind = try container.decode(Kind.self, forKey: .kind)

        switch kind {
        case .published:
            message = try container.decode(MessagePayload.self, forKey: .message)
            dismissedId = nil
        case .dismissed:
            dismissedId = try container.decode(UInt64.self, forKey: .id)
            message = nil
        case .cleared:
            message = nil
            dismissedId = nil
        }
    }
}

private struct QueuedSystemNotification {
    let payload: EditorNotificationPayload
    let windowTitle: String?
}

final class EditorSystemNotificationManager {
    static let shared = EditorSystemNotificationManager()

    private var authorizationRequestInFlight = false
    private var pendingAuthorizationDeliveries: [QueuedSystemNotification] = []
    private var deliveredIdentifiers: Set<String> = []

    private init() {}

    func prepareAuthorizationIfNeeded() {
        guard let center = notificationCenter() else { return }
        center.getNotificationSettings { [weak self] settings in
            DispatchQueue.main.async {
                guard let self else { return }
                if settings.authorizationStatus == .notDetermined {
                    self.requestAuthorizationIfNeeded()
                }
            }
        }
    }

    func deliver(_ payload: EditorNotificationPayload, windowTitle: String?) {
        guard let center = notificationCenter() else { return }
        let queued = QueuedSystemNotification(payload: payload, windowTitle: windowTitle)
        center.getNotificationSettings { [weak self] settings in
            DispatchQueue.main.async {
                self?.handle(settings: settings, queued: queued)
            }
        }
    }

    private func handle(
        settings: UNNotificationSettings,
        queued: QueuedSystemNotification
    ) {
        switch settings.authorizationStatus {
        case .authorized, .provisional, .ephemeral:
            enqueue(queued)
        case .notDetermined:
            pendingAuthorizationDeliveries.append(queued)
            requestAuthorizationIfNeeded()
        default:
            break
        }
    }

    private func requestAuthorizationIfNeeded() {
        guard !authorizationRequestInFlight else { return }
        guard let center = notificationCenter() else { return }
        authorizationRequestInFlight = true
        center.requestAuthorization(options: [.alert, .sound]) { [weak self] granted, _ in
            DispatchQueue.main.async {
                guard let self else { return }
                self.authorizationRequestInFlight = false
                let queued = self.pendingAuthorizationDeliveries
                self.pendingAuthorizationDeliveries.removeAll()
                guard granted else { return }
                queued.forEach(self.enqueue)
            }
        }
    }

    private func enqueue(_ queued: QueuedSystemNotification) {
        guard let center = notificationCenter() else { return }
        let payload = queued.payload
        let trimmedTitle = payload.title.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBody = payload.body?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTitle.isEmpty || !(trimmedBody?.isEmpty ?? true) else {
            return
        }

        let content = UNMutableNotificationContent()
        content.title = trimmedTitle.isEmpty ? (trimmedBody ?? "Notification") : trimmedTitle

        if let body = trimmedBody, !body.isEmpty, body != content.title {
            content.body = body
        }

        if let sourceLabel = payload.sourceLabel?.trimmingCharacters(in: .whitespacesAndNewlines),
           !sourceLabel.isEmpty,
           sourceLabel != content.title {
            content.subtitle = sourceLabel
        } else if let windowTitle = queued.windowTitle?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !windowTitle.isEmpty,
                  windowTitle != content.title {
            content.subtitle = windowTitle
        }

        content.sound = .default

        let identifier = UUID().uuidString
        let request = UNNotificationRequest(
            identifier: identifier,
            content: content,
            trigger: nil
        )

        center.add(request) { [weak self] error in
            guard error == nil else { return }
            DispatchQueue.main.async {
                self?.deliveredIdentifiers.insert(identifier)
            }
        }
    }

    func clearDeliveredNotifications() {
        guard let center = notificationCenter(), !deliveredIdentifiers.isEmpty else { return }
        let identifiers = Array(deliveredIdentifiers)
        deliveredIdentifiers.removeAll()
        center.removeDeliveredNotifications(withIdentifiers: identifiers)
    }

    private func notificationCenter() -> UNUserNotificationCenter? {
        guard EditorNotificationPlatformSupport.supportsUserNotifications else {
            return nil
        }
        return UNUserNotificationCenter.current()
    }
}
