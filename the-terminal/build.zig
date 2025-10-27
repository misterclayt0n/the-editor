const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Create wrapper module with PIC enabled
    const wrapper_module = b.createModule(.{
        .root_source_file = b.path("wrapper.zig"),
        .target = target,
        .optimize = optimize,
        .pic = true, // Enable Position Independent Code for static library
    });

    // Import ghostty-vt dependency
    // SIMD disabled for now to avoid C++ linking issues
    // TODO: Re-enable SIMD once C++ linking is properly configured
    const ghostty_dep = b.dependency("ghostty", .{
        .simd = false,
    });
    wrapper_module.addImport("ghostty-vt", ghostty_dep.module("ghostty-vt"));

    // Build as a static library for linking with Rust
    const lib = b.addLibrary(.{
        .name = "ghostty_wrapper",
        .root_module = wrapper_module,
    });

    b.installArtifact(lib);
}
