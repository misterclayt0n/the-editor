#!/usr/bin/env nu
# Build and package a release of the-editor
# Simple approach like Helix: just cargo build + tarball

def main [version?: string] {
  # Get version from Cargo.toml or use provided argument
  let version = if ($version | is-empty) {
    open Cargo.toml | get package.version
  } else {
    $version
  }

  let platform = "linux-x86_64"
  let release_name = $"the-editor-v($version)-($platform)"
  let dist_dir = "dist"

  print $"Building the-editor v($version) for ($platform)..."

  # Build with cargo
  print "Building with cargo..."
  cargo build --release --features unicode-lines

  print "Build completed!"

  # Create release directory
  print "Creating release package..."
  let release_dir = $"($dist_dir)/($release_name)"
  rm -rf $release_dir
  mkdir $release_dir

  # Copy binary
  cp target/release/the-editor $release_dir

  # Copy runtime
  cp -r runtime $release_dir

  # Copy README and LICENSE if they exist
  if ("README.md" | path exists) { cp README.md $release_dir }
  if ("LICENSE" | path exists) { cp LICENSE $release_dir }
  if ("INSTALL.md" | path exists) { cp INSTALL.md $release_dir }

  # Create tarball
  print "Creating tarball..."
  cd $dist_dir
  tar czf $"($release_name).tar.gz" $release_name

  # Generate checksum
  let checksum = (open $"($release_name).tar.gz" | hash sha256)
  $"($checksum)  ($release_name).tar.gz\n" | save -f $"($release_name).tar.gz.sha256"

  cd ..

  print ""
  print "✓ Release created successfully!"
  print ""
  print $"  Tarball: ($dist_dir)/($release_name).tar.gz"
  print $"  Checksum: ($dist_dir)/($release_name).tar.gz.sha256"
  print ""
  print "To create a GitHub release:"
  print $"  1. git tag v($version)"
  print $"  2. git push origin v($version)"
  print $"  3. gh release create v($version) --draft \\"
  print $"       ($dist_dir)/($release_name).tar.gz \\"
  print $"       ($dist_dir)/($release_name).tar.gz.sha256 \\"
  print $"       --title 'the-editor v($version)' \\"
  print "       --notes-file RELEASE_NOTES.md"
  print ""
}
