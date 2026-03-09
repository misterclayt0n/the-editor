import Foundation
import TheEditorFFIBridge

/// Editor core runtime backing one Swift editor window/tab instance.
final class SharedEditorRuntime {
    private static let instanceIdLock = NSLock()
    private static var nextInstanceId: UInt64 = 1

    let app: TheEditorFFIBridge.App
    let editorId: EditorId
    let instanceId: UInt64
    private var nativeTabGatewayRefCount: Int = 0

    init() {
        instanceId = SharedEditorRuntime.allocateInstanceId()
        app = TheEditorFFIBridge.App()
        app.set_inline_diagnostic_rendering_enabled(false)
        let viewport = Rect(x: 0, y: 0, width: 80, height: 24)
        let scroll = Position(row: 0, col: 0)
        editorId = app.create_editor("", viewport, scroll)
    }

    @discardableResult
    func openFilePath(_ path: String) -> Bool {
        app.open_file_path(editorId, path)
    }

    @discardableResult
    func openFilePathInNewTab(_ path: String) -> Bool {
        app.open_file_path_in_new_tab(editorId, path)
    }

    @discardableResult
    func openUntitledBufferInNewTab() -> UInt64 {
        app.open_untitled_buffer_in_new_tab(editorId)
    }

    @discardableResult
    func openFilePathSuppressingGateway(_ path: String) -> Bool {
        let shouldRestore = nativeTabGatewayRefCount > 0
        if shouldRestore {
            app.set_native_tab_open_gateway(false)
        }
        let opened = app.open_file_path(editorId, path)
        if shouldRestore {
            app.set_native_tab_open_gateway(true)
        }
        return opened
    }

    func retainNativeTabGateway() {
        nativeTabGatewayRefCount += 1
        if nativeTabGatewayRefCount == 1 {
            app.set_native_tab_open_gateway(true)
        }
    }

    func releaseNativeTabGateway() {
        guard nativeTabGatewayRefCount > 0 else {
            return
        }
        nativeTabGatewayRefCount -= 1
        if nativeTabGatewayRefCount == 0 {
            app.set_native_tab_open_gateway(false)
        }
    }

    func withNativeTabGatewaySuppressed(_ body: () -> Void) {
        let shouldRestore = nativeTabGatewayRefCount > 0
        if shouldRestore {
            app.set_native_tab_open_gateway(false)
        }
        body()
        if shouldRestore {
            app.set_native_tab_open_gateway(true)
        }
    }

    private static func allocateInstanceId() -> UInt64 {
        instanceIdLock.lock()
        defer { instanceIdLock.unlock() }
        let id = nextInstanceId
        let next = nextInstanceId &+ 1
        nextInstanceId = (next == 0) ? 1 : next
        return id
    }
}
