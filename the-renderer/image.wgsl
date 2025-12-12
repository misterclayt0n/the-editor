struct ImageUniforms {
    screen_size: vec2<f32>,
    rect_position: vec2<f32>,
    rect_size: vec2<f32>,
    alpha: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: ImageUniforms;

@group(0) @binding(1)
var image_texture: texture_2d<f32>;

@group(0) @binding(2)
var image_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Calculate world position.
    let world_pos = uniforms.rect_position + vertex.position * uniforms.rect_size;
    
    // Convert to normalized device coordinates (-1 to 1).
    let ndc = (world_pos / uniforms.screen_size) * 2.0 - 1.0;
    
    let clip_pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0); // Flip Y for screen coordinates.
    out.clip_position = clip_pos;
    out.tex_coord = vertex.tex_coord;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(image_texture, image_sampler, in.tex_coord);
    return vec4<f32>(color.rgb, color.a * uniforms.alpha);
}