#!/usr/bin/env nu

let root = ($env.FILE_PWD | path dirname)
let swift_dir = ($root | path join "the-swift")
let swift_bridge_dir = ($swift_dir | path join "Sources" "TheEditorFFIBridge")
let gen_dir = ($root | path join "the-ffi" "generated")
let headers_dir = ($gen_dir | path join "xcframework_headers")
let frameworks_dir = ($swift_dir | path join "Frameworks")
let xcframework = ($frameworks_dir | path join "TheEditorFFI.xcframework")
let lib_path = ($root | path join "target" "release" "libthe_ffi.a")
let xcode_project = ($swift_dir | path join "TheEditor.xcodeproj")
let xcode_scheme = "TheEditor"
let app_bundle_name = "The Editor.app"
let xcode_dir = "/Applications/Xcode.app/Contents/Developer"

if ($xcode_dir | path exists) {
  $env.DEVELOPER_DIR = $xcode_dir
}

let module_cache = ($env.SWIFT_MODULE_CACHE_PATH? | default "/tmp/swift-module-cache")
let clang_cache = ($env.CLANG_MODULE_CACHE_PATH? | default "/tmp/clang-module-cache")
$env.MACOSX_DEPLOYMENT_TARGET = ($env.MACOSX_DEPLOYMENT_TARGET? | default "13.0")
$env.SWIFT_MODULE_CACHE_PATH = $module_cache
$env.CLANG_MODULE_CACHE_PATH = $clang_cache
let launch_mode = ($env.THE_EDITOR_MACOS_LAUNCH_MODE? | default "app")
let derived_data = ($env.THE_EDITOR_MACOS_DERIVED_DATA_PATH? | default "/tmp/the-editor-xcode-derived")
let archive_path = ($env.THE_EDITOR_MACOS_ARCHIVE_PATH? | default "/tmp/the-editor-xcode-archive/TheEditor.xcarchive")
let export_path = ($env.THE_EDITOR_MACOS_EXPORT_PATH? | default "/tmp/the-editor-xcode-export")
let export_options_override = ($env.THE_EDITOR_MACOS_EXPORT_OPTIONS_PLIST? | default "")
let export_method = ($env.THE_EDITOR_MACOS_EXPORT_METHOD? | default "mac-application")
let export_destination = ($env.THE_EDITOR_MACOS_EXPORT_DESTINATION? | default "export")
let export_signing_style = ($env.THE_EDITOR_MACOS_EXPORT_SIGNING_STYLE? | default "")
let export_signing_certificate = ($env.THE_EDITOR_MACOS_EXPORT_SIGNING_CERTIFICATE? | default "")
let export_team_id = ($env.THE_EDITOR_MACOS_EXPORT_TEAM_ID? | default "")
let export_allow_updates = ($env.THE_EDITOR_MACOS_ALLOW_PROVISIONING_UPDATES? | default "0")
let dmg_path = ($env.THE_EDITOR_MACOS_DMG_PATH? | default ($root | path join "dist" "TheEditor.dmg"))
let dmg_stage = ($env.THE_EDITOR_MACOS_DMG_STAGE_PATH? | default "/tmp/the-editor-dmg-root")
let dmg_volume_name = ($env.THE_EDITOR_MACOS_DMG_VOLUME_NAME? | default "The Editor")
let xcode_home = ($env.THE_EDITOR_MACOS_XCODE_HOME? | default "/tmp/the-editor-xcode-home")
let xcode_cache = ($env.THE_EDITOR_MACOS_XCODE_CACHE_DIR? | default "/tmp/the-editor-xcode-cache")
let xcode_arch = ($env.THE_EDITOR_MACOS_XCODE_ARCH? | default (^uname -m | str trim))

def ensure-dir [dir: path] {
  if not ($dir | path exists) {
    mkdir $dir
  }
}

def xcode-configuration [release: bool, archive: bool] {
  if $archive {
    "Release"
  } else if $release {
    "ReleaseLocal"
  } else {
    "Debug"
  }
}

def xcode-app-path [configuration: string] {
  $derived_data | path join "Build" "Products" $configuration $app_bundle_name
}

def archive-app-path [] {
  $archive_path | path join "Products" "Applications" $app_bundle_name
}

def export-app-path [] {
  $export_path | path join $app_bundle_name
}

def plist-string-entry [key: string, value: string] {
  [
    $"  <key>($key)</key>"
    $"  <string>($value)</string>"
  ]
}

def plist-bool-entry [key: string, value: bool] {
  [
    $"  <key>($key)</key>"
    (if $value { "  <true/>" } else { "  <false/>" })
  ]
}

def generated-export-options-plist [] {
  let options_plist = "/tmp/the-editor-export-options.plist"
  let lines = (
    [
      "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
      "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">"
      "<plist version=\"1.0\">"
      "<dict>"
    ]
    ++ (plist-string-entry "method" $export_method)
    ++ (plist-string-entry "destination" $export_destination)
    ++ (plist-bool-entry "stripSwiftSymbols" false)
    ++ (if ($export_signing_style | is-empty) { [] } else { plist-string-entry "signingStyle" $export_signing_style })
    ++ (if ($export_signing_certificate | is-empty) { [] } else { plist-string-entry "signingCertificate" $export_signing_certificate })
    ++ (if ($export_team_id | is-empty) { [] } else { plist-string-entry "teamID" $export_team_id })
    ++ [
      "</dict>"
      "</plist>"
    ]
  )

  $lines | str join "\n" | save -f $options_plist
  $options_plist
}

def run-export-archive [] {
  if not ($archive_path | path exists) {
    error make {
      msg: $"archive not found at ($archive_path)"
    }
  }

  let options_plist = (
    if ($export_options_override | is-empty) {
      generated-export-options-plist
    } else {
      $export_options_override
    }
  )

  let export_parent = ($export_path | path dirname)
  ensure-dir $export_parent
  if ($export_path | path exists) {
    rm -rf $export_path
  }

  with-env {
    HOME: $xcode_home
    XDG_CACHE_HOME: $xcode_cache
    SWIFT_MODULE_CACHE_PATH: $module_cache
    CLANG_MODULE_CACHE_PATH: $clang_cache
    DEVELOPER_DIR: $env.DEVELOPER_DIR
  } {
    let maybe_allow_updates = (
      if $export_allow_updates == "1" or $export_allow_updates == "true" or $export_allow_updates == "yes" {
        ["-allowProvisioningUpdates"]
      } else {
        []
      }
    )

    ^/usr/bin/xcodebuild ...$maybe_allow_updates -exportArchive -archivePath $archive_path -exportPath $export_path -exportOptionsPlist $options_plist
  }

  export-app-path
}

def build-dmg [app_path: path] {
  if not ($app_path | path exists) {
    error make {
      msg: $"app bundle not found at ($app_path)"
    }
  }

  let dmg_parent = ($dmg_path | path dirname)
  ensure-dir $dmg_parent

  if ($dmg_stage | path exists) {
    rm -rf $dmg_stage
  }
  ensure-dir $dmg_stage

  let app_name = ($app_path | path basename)
  let staged_app = ($dmg_stage | path join $app_name)
  ^/usr/bin/ditto $app_path $staged_app
  ^ln -s /Applications ($dmg_stage | path join "Applications")

  if ($dmg_path | path exists) {
    rm -f $dmg_path
  }

  ^/usr/bin/hdiutil create -volname $dmg_volume_name -srcfolder $dmg_stage -ov -format UDZO $dmg_path
  $dmg_path
}

def run-xcodebuild [configuration: string, archive: bool] {
  ensure-dir $derived_data
  ensure-dir $xcode_home
  ensure-dir $xcode_cache

  with-env {
    HOME: $xcode_home
    XDG_CACHE_HOME: $xcode_cache
    SWIFT_MODULE_CACHE_PATH: $module_cache
    CLANG_MODULE_CACHE_PATH: $clang_cache
    DEVELOPER_DIR: $env.DEVELOPER_DIR
  } {
    if $archive {
      let archive_dir = ($archive_path | path dirname)
      ensure-dir $archive_dir
      ^/usr/bin/xcodebuild -project $xcode_project -scheme $xcode_scheme -configuration $configuration -derivedDataPath $derived_data -arch $xcode_arch archive -archivePath $archive_path
    } else {
      ^/usr/bin/xcodebuild -project $xcode_project -scheme $xcode_scheme -configuration $configuration -derivedDataPath $derived_data -arch $xcode_arch build
    }
  }
}

def main [
  --release
  --archive
  --export
  --dmg
  --build-only
] {
  ensure-dir $module_cache
  ensure-dir $clang_cache
  ensure-dir $swift_bridge_dir
  ensure-dir $headers_dir
  ensure-dir $frameworks_dir

  let archive_export_flow = ($archive or $export)
  let local_dmg_flow = ($dmg and not $archive_export_flow)
  let distribution_flow = ($archive_export_flow or $local_dmg_flow)
  let configuration = (
    if $local_dmg_flow {
      "ReleaseLocal"
    } else {
      xcode-configuration $release $archive_export_flow
    }
  )

  if $launch_mode == "swift-run" and $distribution_flow {
    error make {
      msg: "swift-run mode only supports local executable launches; use the default app mode for archive/export/dmg"
    }
  }

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

  if $launch_mode == "swift-run" {
    cd $swift_dir
    if $release {
      ^swift run -c release the-swift
    } else {
      ^swift run the-swift
    }
    return
  }

  if $archive_export_flow {
    if $archive or not ($archive_path | path exists) {
      run-xcodebuild $configuration true
    } else {
      print $"using existing archive: ($archive_path)"
    }

    if $archive and not $export and not $dmg {
      print $"archived app bundle: ($archive_path)"
      return
    }

    let packaged_app = (run-export-archive)
    print $"exported app bundle: ($packaged_app)"

    if $dmg {
      let built_dmg = (build-dmg $packaged_app)
      print $"built dmg: ($built_dmg)"
    }
    return
  }

  if $local_dmg_flow {
    run-xcodebuild $configuration false
    let app_path = (xcode-app-path $configuration)
    print $"built app bundle: ($app_path)"
    let built_dmg = (build-dmg $app_path)
    print $"built dmg: ($built_dmg)"
    return
  }

  run-xcodebuild $configuration false

  let app_path = (xcode-app-path $configuration)
  print $"built app bundle: ($app_path)"

  if $build_only or $launch_mode == "build" {
    return
  }

  if $launch_mode == "app" {
    ^open -n $app_path
  } else {
    error make {
      msg: $"unsupported THE_EDITOR_MACOS_LAUNCH_MODE: ($launch_mode)"
    }
  }
}
