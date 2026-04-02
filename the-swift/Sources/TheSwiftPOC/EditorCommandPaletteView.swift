import AppKit
import SwiftUI

struct EditorCommandPaletteView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        ZStack {
            if controller.commandPalette.isOpen {
                GeometryReader { geometry in
                    VStack {
                        Spacer().frame(height: geometry.size.height * 0.05)

                        CommandPaletteView(
                            isPresented: Binding(
                                get: { controller.commandPalette.isOpen },
                                set: { presented in
                                    if !presented {
                                        controller.closeCommandPalette()
                                    }
                                }
                            ),
                            query: controller.commandPalette.query,
                            placeholder: controller.commandPalette.placeholder,
                            backgroundColor: Color(nsColor: controller.scene?.backgroundColor ?? .windowBackgroundColor),
                            options: commandOptions,
                            selectedIndex: controller.commandPalette.selectedIndex.map(UInt.init),
                            onQueryChange: {
                                commandPaletteDebugLog("onQueryChange query=\(String(reflecting: $0))")
                                controller.setCommandPaletteQuery($0)
                            },
                            onMove: {
                                commandPaletteDebugLog("onMove direction=\(String(describing: $0))")
                                controller.moveCommandPaletteSelection($0)
                            },
                            onSubmit: {
                                commandPaletteDebugLog("onSubmit query=\(String(reflecting: controller.commandPalette.query)) selected=\(String(describing: controller.commandPalette.selectedIndex))")
                                controller.submitCommandPalette()
                            }
                        )
                        .zIndex(1)

                        Spacer()
                    }
                    .frame(width: geometry.size.width, height: geometry.size.height, alignment: .top)
                }
            }
        }
        .onChange(of: controller.commandPalette.isOpen) { _, isOpen in
            if !isOpen {
                DispatchQueue.main.async {
                    controller.focusEditor()
                }
            }
        }
    }

    private var commandOptions: [CommandOption] {
        controller.commandPalette.items.enumerated().map { index, item in
            CommandOption(
                title: item.title,
                subtitle: item.subtitle,
                description: item.description,
                symbols: nil,
                leadingIcon: item.leadingIcon,
                leadingColor: item.leadingColor.map { Color(nsColor: $0.color) },
                badge: item.badge,
                emphasis: item.emphasis
            ) {
                controller.submitCommandPalette(visibleIndex: index)
            }
        }
    }
}
