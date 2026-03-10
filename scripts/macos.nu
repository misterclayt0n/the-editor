#!/usr/bin/env nu

let root = ($env.FILE_PWD | path dirname)
let swift_dir = ($root | path join "the-swift")
let swift_bridge_dir = ($swift_dir | path join "Sources" "TheEditorFFIBridge")
let gen_dir = ($root | path join "the-ffi" "generated")
let headers_dir = ($gen_dir | path join "xcframework_headers")
let frameworks_dir = ($swift_dir | path join "Frameworks")
let xcframework = ($frameworks_dir | path join "TheEditorFFI.xcframework")
let lib_path = ($root | path join "target" "release" "libthe_ffi.a")
let xcode_dir = "/Applications/Xcode.app/Contents/Developer"

if ($xcode_dir | path exists) {
  $env.DEVELOPER_DIR = $xcode_dir
}

let module_cache = ($env.SWIFT_MODULE_CACHE_PATH? | default "/tmp/swift-module-cache")
let clang_cache = ($env.CLANG_MODULE_CACHE_PATH? | default "/tmp/clang-module-cache")
$env.MACOSX_DEPLOYMENT_TARGET = ($env.MACOSX_DEPLOYMENT_TARGET? | default "13.0")
$env.SWIFT_MODULE_CACHE_PATH = $module_cache
$env.CLANG_MODULE_CACHE_PATH = $clang_cache
let launch_mode = ($env.THE_EDITOR_MACOS_LAUNCH_MODE? | default "swift-run")

def launch_debug_app_bundle [swift_dir: path, deployment_target: string] {
  cd $swift_dir
  ^swift build
  let bin_path = (^swift build --show-bin-path | str trim)
  let app_dir = ($bin_path | path join "the-swift.app")
  let contents_dir = ($app_dir | path join "Contents")
  let macos_dir = ($contents_dir | path join "MacOS")
  let resources_dir = ($contents_dir | path join "Resources")
  let plist = ($contents_dir | path join "Info.plist")

  rm -rf $app_dir
  mkdir $macos_dir
  mkdir $resources_dir

  cp -f ($bin_path | path join "the-swift") ($macos_dir | path join "the-swift")

  let resource_bundle = ($bin_path | path join "the-swift_TheSwift.bundle")
  if ($resource_bundle | path exists) {
    cp -r $resource_bundle $resources_dir
  }

  [
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">"
    "<plist version=\"1.0\">"
    "<dict>"
    "  <key>CFBundleDevelopmentRegion</key>"
    "  <string>en</string>"
    "  <key>CFBundleExecutable</key>"
    "  <string>the-swift</string>"
    "  <key>CFBundleIdentifier</key>"
    "  <string>dev.theeditor.theswift</string>"
    "  <key>CFBundleInfoDictionaryVersion</key>"
    "  <string>6.0</string>"
    "  <key>CFBundleName</key>"
    "  <string>the-swift</string>"
    "  <key>CFBundlePackageType</key>"
    "  <string>APPL</string>"
    "  <key>CFBundleShortVersionString</key>"
    "  <string>0.1</string>"
    "  <key>CFBundleVersion</key>"
    "  <string>1</string>"
    "  <key>LSMinimumSystemVersion</key>"
    $"  <string>($deployment_target)</string>"
    "  <key>NSHighResolutionCapable</key>"
    "  <true/>"
    "  <key>NSPrincipalClass</key>"
    "  <string>NSApplication</string>"
    "</dict>"
    "</plist>"
  ] | str join "\n" | save -f $plist

  ^codesign --force --deep --sign - --identifier dev.theeditor.theswift --timestamp=none $app_dir

  print $"launching app bundle: ($app_dir)"
  ^open -n $app_dir
}

if not ($module_cache | path exists) { mkdir $module_cache }
if not ($clang_cache | path exists) { mkdir $clang_cache }
if not ($swift_bridge_dir | path exists) { mkdir $swift_bridge_dir }
if not ($headers_dir | path exists) { mkdir $headers_dir }
if not ($frameworks_dir | path exists) { mkdir $frameworks_dir }

^cargo build -p the-ffi --release

let swift_core = ($swift_bridge_dir | path join "SwiftBridgeCore.swift")
let ffi_swift = ($swift_bridge_dir | path join "the-ffi.swift")
cp -f ($gen_dir | path join "SwiftBridgeCore.swift") $swift_core
cp -f ($gen_dir | path join "the-ffi" "the-ffi.swift") $ffi_swift
cp -f ($gen_dir | path join "SwiftBridgeCore.h") ($headers_dir | path join "SwiftBridgeCore.h")
cp -f ($gen_dir | path join "the-ffi" "the-ffi.h") ($headers_dir | path join "the-ffi.h")

let umbrella = ($headers_dir | path join "TheEditorFFI.h")
[
  "// Umbrella header for the Swift/C bridge."
  "// Keep SwiftBridgeCore before the-ffi to ensure all core types are visible."
  "#include \"SwiftBridgeCore.h\""
  "#include \"the-ffi.h\""
] | str join "\n" | save -f $umbrella

let modulemap = ($headers_dir | path join "module.modulemap")
[
  "module TheEditorFFI {"
  "  umbrella header \"TheEditorFFI.h\""
  "  export *"
  "  module * { export * }"
  "}"
] | str join "\n" | save -f $modulemap

if not (open $swift_core | str contains "import TheEditorFFI") {
  let contents = (open $swift_core)
  ("import TheEditorFFI\n" + $contents) | save -f $swift_core
}

let patched_swift_core = (
  open $swift_core
  | str replace "extension RustStr: Identifiable {" "extension RustStr: @retroactive Identifiable {"
  | str replace "extension RustStr: Equatable {" "extension RustStr: @retroactive Equatable {"
)
$patched_swift_core | save -f $swift_core

if not (open $ffi_swift | str contains "import TheEditorFFI") {
  let contents = (open $ffi_swift)
  ("import Foundation\nimport TheEditorFFI\n\n" + $contents) | save -f $ffi_swift
}

rm -rf $xcframework
^xcodebuild -create-xcframework -library $lib_path -headers $headers_dir -output $xcframework

cd $swift_dir
if $launch_mode == "app" {
  launch_debug_app_bundle $swift_dir $env.MACOSX_DEPLOYMENT_TARGET
} else {
  ^swift run the-swift
}
