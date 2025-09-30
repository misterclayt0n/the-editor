// Gaussian blur shader for background blur effect

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var input_sampler: sampler;

struct BlurUniforms {
    direction: vec2<f32>,  // (1,0) for horizontal, (0,1) for vertical
    resolution: vec2<f32>,
}

@group(0) @binding(2)
var<uniform> uniforms: BlurUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Full-screen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);

    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    // Map 0..2 vertex-space to 0..1 UVs
    out.tex_coord = vec2<f32>(x, y) * 0.5;

    return out;
}

// 9-tap Gaussian blur kernel weights
const WEIGHTS = array<f32, 9>(
    0.0625, 0.125, 0.0625,
    0.125,  0.25,  0.125,
    0.0625, 0.125, 0.0625
);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / uniforms.resolution;
    var result = vec4<f32>(0.0);

    // 9-tap Gaussian blur
    var index = 0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * uniforms.direction * texel_size;
            let sample_coord = in.tex_coord + offset;
            result += textureSample(input_texture, input_sampler, sample_coord) * WEIGHTS[index];
            index++;
        }
    }

    return result;
}
