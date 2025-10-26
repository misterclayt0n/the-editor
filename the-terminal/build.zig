const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Create wrapper module
    const wrapper_module = b.createModule(.{
        .root_source_file = b.path("wrapper.zig"),
        .target = target,
        .optimize = optimize,
    });

    // Import ghostty-vt dependency
    const ghostty_dep = b.dependency("ghostty", .{
        .simd = true,
    });
    wrapper_module.addImport("ghostty-vt", ghostty_dep.module("ghostty-vt"));

    // Build as a static library for linking with Rust
    const lib = b.addLibrary(.{
        .name = "ghostty_wrapper",
        .root_module = wrapper_module,
    });

    b.installArtifact(lib);
}
