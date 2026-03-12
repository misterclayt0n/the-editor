import Foundation

private final class TheSwiftResourceBundleLocator {}

enum TheSwiftResourceBundle {
    static let bundle: Bundle = {
#if SWIFT_PACKAGE
        return .module
#else
        return Bundle(for: TheSwiftResourceBundleLocator.self)
#endif
    }()
}
