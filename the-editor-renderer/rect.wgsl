// Rectangle rendering shader

struct RectUniforms {
    screen_size: vec2<f32>,
}

struct RectInstance {
    position: vec2<f32>,
    size: vec2<f32>,
    color: vec4<f32>,
    corner_radius: f32,
    glow_center: vec2<f32>,
    glow_radius: f32,
    effect_kind: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: RectUniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct InstanceInput {
    @location(1) rect_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) rect_color: vec4<f32>,
    @location(4) corner_radius: f32,
    @location(5) glow_center: vec2<f32>,
    @location(6) glow_radius: f32,
    @location(7) effect_kind: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) glow_center: vec2<f32>,
    @location(5) glow_radius: f32,
    @location(6) effect_kind: f32,
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Calculate world position
    let world_pos = instance.rect_position + vertex.position * instance.rect_size;

    // Convert to normalized device coordinates (-1 to 1)
    let ndc = (world_pos / uniforms.screen_size) * 2.0 - 1.0;
    let clip_pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0); // Flip Y for screen coordinates

    out.clip_position = clip_pos;
    out.color = instance.rect_color;
    out.local_pos = vertex.position * instance.rect_size;
    out.rect_size = instance.rect_size;
    out.corner_radius = instance.corner_radius;
    out.glow_center = instance.glow_center;
    out.glow_radius = instance.glow_radius;
    out.effect_kind = instance.effect_kind;

    return out;
}

// Signed distance function for rounded rectangle
fn sdf_rounded_rect(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let d = abs(pos - half_size) - half_size + radius;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = sdf_rounded_rect(in.local_pos, in.rect_size, in.corner_radius);
    var shape_alpha: f32;
    if (in.corner_radius > 0.0) {
        shape_alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
    } else {
        shape_alpha = 1.0;
    }

    // effect_kind: 0.0 = flat fill, 1.0 = internal glow overlay, 2.0 = stroke ring
    if (in.effect_kind < 0.5) {
        return vec4<f32>(in.color.rgb, in.color.a * shape_alpha);
    } else if (in.effect_kind < 1.5) {
        // Internal glow: radial falloff from glow_center, clipped by rounded-rect mask.
        // Use pixel-space coordinates for smoother feel.
        let g_center = in.glow_center; // already in local px space
        let d = distance(in.local_pos, g_center);
        let radius = max(in.glow_radius, 1.0);
        // Smooth falloff: 1.0 at center -> 0.0 at radius
        let glow = 1.0 - smoothstep(0.0, radius, d);
        // Border accent derived from shape edge (dist ~ 0). Tie it to glow so it only
        // brightens near the cursor rather than the entire outline.
        let edge = 1.0 - smoothstep(-1.5, -0.1, dist);
        // Keep glow simple and gentle; emphasize the edge modestly
        let intensity = clamp(glow * (0.40 + 0.60 * edge), 0.0, 1.0);
        return vec4<f32>(in.color.rgb, in.color.a * intensity * shape_alpha);
    } else {
        // Stroke ring: difference of outer and inner rounded-rect masks.
        // Use glow_radius as stroke thickness in pixels.
        let thickness = max(in.glow_radius, 0.5);
        let inner_size = max(in.rect_size - vec2<f32>(thickness * 2.0), vec2<f32>(1.0));
        let inner_radius = max(in.corner_radius - thickness, 0.0);
        // Shift local position so the inner rect remains centered relative to the outer rect
        let pos_inner = in.local_pos - vec2<f32>(thickness, thickness);
        let dist_inner = sdf_rounded_rect(pos_inner, inner_size, inner_radius);
        let alpha_outer = 1.0 - smoothstep(-0.5, 0.5, dist);
        let alpha_inner = 1.0 - smoothstep(-0.5, 0.5, dist_inner);
        let ring_alpha = clamp(alpha_outer - alpha_inner, 0.0, 1.0);
        return vec4<f32>(in.color.rgb, in.color.a * ring_alpha);
    }
}
