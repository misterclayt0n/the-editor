import SwiftUI

struct EditorNotificationOverlay: View {
    let banners: [EditorNotificationBannerSnapshot]

    var body: some View {
        VStack(spacing: 10) {
            ForEach(banners) { banner in
                EditorNotificationBannerView(snapshot: banner)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .padding(.horizontal, 20)
        .padding(.bottom, 28)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
        .allowsHitTesting(false)
        .animation(.spring(response: 0.34, dampingFraction: 0.82), value: banners)
    }
}

private struct EditorNotificationBannerView: View {
    let snapshot: EditorNotificationBannerSnapshot

    private var bodyText: String? {
        let trimmed = snapshot.body?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return trimmed.isEmpty ? nil : trimmed
    }

    private var sourceText: String? {
        let trimmed = snapshot.sourceLabel?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return trimmed.isEmpty ? nil : trimmed
    }

    private var width: CGFloat {
        bodyText == nil ? 560 : 620
    }

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: snapshot.severity.symbolName)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(snapshot.severity.swiftUIColor)
                .frame(width: 20, height: 20)

            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(snapshot.title)
                        .font(FontLoader.uiFont(size: 13).weight(.semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(bodyText == nil ? 2 : 1)

                    if let sourceText {
                        Text(sourceText)
                            .font(FontLoader.uiFont(size: 10).weight(.semibold))
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 3)
                            .background(
                                Capsule(style: .continuous)
                                    .fill(Color.white.opacity(0.08))
                            )
                            .fixedSize()
                    }
                }

                if let bodyText {
                    Text(bodyText)
                        .font(FontLoader.uiFont(size: 12).weight(.medium))
                        .foregroundStyle(.secondary)
                        .lineLimit(3)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, bodyText == nil ? 10 : 12)
        .frame(maxWidth: width, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 15, style: .continuous)
                .fill(.ultraThinMaterial)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 15, style: .continuous)
                .stroke(Color.white.opacity(0.08), lineWidth: 1)
        )
        .shadow(color: Color.black.opacity(0.18), radius: 18, x: 0, y: 8)
        .accessibilityElement(children: .combine)
    }
}
