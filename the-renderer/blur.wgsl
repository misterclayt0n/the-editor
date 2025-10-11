// Gaussian blur shader for background blur effect

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var input_sampler: sampler;

struct BlurUniforms {
    direction: vec2<f32>,  // (1,0) for horizontal, (0,1) for vertical
    resolution: vec2<f32>,
    opacity: f32,          // 0.0 - 1.0, controls final blur opacity
    _padding: f32,         // Alignment padding
    rect_origin: vec2<f32>,
    rect_size: vec2<f32>,
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

    // Full-screen triangle mapped to the requested rectangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);

    // Normalized 0..1 coordinates for this vertex
    let uv = vec2<f32>(x, y) * 0.5;

    // Convert rect information from pixels to clip space / UVs
    let rect_origin = uniforms.rect_origin;
    let rect_size = uniforms.rect_size;
    let resolution = uniforms.resolution;

    let pixel_pos = rect_origin + uv * rect_size;

    let clip_x = (pixel_pos.x / resolution.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (pixel_pos.y / resolution.y) * 2.0;

    out.position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);

    let uv_origin = rect_origin / resolution;
    let uv_scale = rect_size / resolution;
    out.tex_coord = uv_origin + uv * uv_scale;

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

    // Apply opacity (only on vertical pass, when direction.y != 0)
    // This ensures opacity is applied once at the final output
    if (uniforms.direction.y != 0.0) {
        result.a *= uniforms.opacity;
    }

    return result;
}
