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
    effect_time: f32,
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
    @location(8) effect_time: f32,
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
    @location(7) effect_time: f32,
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
    out.effect_time = instance.effect_time;

    return out;
}

// Signed distance function for rounded rectangle
fn sdf_rounded_rect(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let d = abs(pos - half_size) - half_size + radius;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a + (b - a) * t;
}

// 3D simplex noise function for fire effect
fn snoise(uv_input: vec3<f32>, res: f32) -> f32 {
    let s = vec3<f32>(1e0, 1e2, 1e3);
    var uv = uv_input * res;

    let uv0 = floor(uv % res) * s;
    let uv1 = floor((uv + vec3<f32>(1.0)) % res) * s;

    var f = fract(uv);
    f = f * f * (3.0 - 2.0 * f);

    let v = vec4<f32>(
        uv0.x + uv0.y + uv0.z,
        uv1.x + uv0.y + uv0.z,
        uv0.x + uv1.y + uv0.z,
        uv1.x + uv1.y + uv0.z
    );

    var r = fract(sin(v * 1e-1) * 1e3);
    let r0 = mix(mix(r.x, r.y, f.x), mix(r.z, r.w, f.x), f.y);

    r = fract(sin((v + uv1.z - uv0.z) * 1e-1) * 1e3);
    let r1 = mix(mix(r.x, r.y, f.x), mix(r.z, r.w, f.x), f.y);

    return mix(r0, r1, f.z) * 2.0 - 1.0;
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
    } else if (in.effect_kind < 2.5) {
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
    } else if (in.effect_kind < 3.5) {
        // Directional stroke ring with thickness fading from top -> sides -> bottom.
        let thickness_top = in.glow_center.x;
        let thickness_bottom = in.glow_center.y;
        let thickness_side = in.glow_radius;

        let dist_top = in.local_pos.y;
        let dist_side = min(in.local_pos.x, in.rect_size.x - in.local_pos.x);

        let top_range = max(thickness_top * 3.0, 1.0);
        let side_range = max(thickness_side * 3.0, 1.0);

        let top_influence = saturate(1.0 - dist_top / top_range);
        let side_influence = saturate(1.0 - dist_side / side_range);

        let thickness_by_top = lerp(thickness_bottom, thickness_top, top_influence);
        let thickness_by_side = lerp(thickness_bottom, thickness_side, side_influence);
        let thickness = max(max(thickness_by_top, thickness_by_side), 0.5);

        let outer_alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
        let inner_threshold = -thickness;
        let inner_alpha = 1.0 - smoothstep(inner_threshold - 0.5, inner_threshold + 0.5, dist);
        let ring_alpha = clamp(outer_alpha - inner_alpha, 0.0, 1.0);
        return vec4<f32>(in.color.rgb, in.color.a * ring_alpha * shape_alpha);
    } else if (in.effect_kind < 4.5) {
        // Fire explosion effect (effect_kind 4.0)
        // Inspired by https://godotshaders.com/shader/ball-of-fire/
        let center = in.rect_size * 0.5;
        let t = in.effect_time;

        // Convert to normalized coordinates centered at origin
        let p = (in.local_pos - center) / in.glow_radius;

        // Calculate intensity based on distance from center
        let frame_scope = 3.0;
        var color_intensity = 3.0 - (frame_scope * length(2.0 * p));

        // Polar coordinates for noise
        let angle = atan2(p.x, p.y) / 6.2832 + 0.5;
        let radius = length(p) * 0.4;
        let coord = vec3<f32>(angle, radius, 0.5);

        // Layer multiple octaves of noise for turbulent fire effect
        for (var i: i32 = 1; i <= 7; i = i + 1) {
            let power = pow(2.0, f32(i));
            // Animate noise by offsetting coordinates over time
            let time_offset = vec3<f32>(0.0, -t * 0.5, t * 0.1);
            color_intensity += (1.5 / power) * snoise(coord + time_offset, power * 16.0);
        }

        // Create fire color gradient (white -> yellow -> orange -> red -> black)
        if (color_intensity < 0.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        } else {
            // Fire color: red (full), green (squared), blue (cubed)
            let fire_color = vec3<f32>(
                color_intensity,
                pow(max(color_intensity, 0.0), 2.0) * 0.4,
                pow(max(color_intensity, 0.0), 3.0) * 0.15
            );

            // Alpha is the luminance of the fire color
            let alpha = length(fire_color);

            // Fade out over time
            let time_fade = 1.0 - t;

            return vec4<f32>(fire_color * time_fade, alpha * time_fade * shape_alpha);
        }

    } else if (in.effect_kind < 5.5) {
        // Laser effect (effect_kind 5.0)
        // Vertical laser beam with glow
        let center = in.rect_size * 0.5;
        let t = in.effect_time;

        // Horizontal distance from center (for laser beam)
        let horiz_dist = abs(in.local_pos.x - center.x);

        // Core beam
        let beam_width = 1.5;
        let beam_alpha = 1.0 - smoothstep(0.0, beam_width, horiz_dist);

        // Glow around beam
        let glow_width = 6.0 + sin(t * 6.28 * 2.0) * 2.0; // Pulsing glow
        let glow_alpha = (1.0 - smoothstep(beam_width, glow_width, horiz_dist)) * 0.6;

        // Vertical scan line animation
        let scan_y = (t * in.rect_size.y) % in.rect_size.y;
        let scan_dist = abs(in.local_pos.y - scan_y);
        let scan_alpha = (1.0 - smoothstep(0.0, 8.0, scan_dist)) * 0.4;

        // Energy flicker
        let flicker = sin(t * 6.28 * 8.0) * 0.15 + 0.85;

        // Combine effects
        let total_alpha = (beam_alpha + glow_alpha + scan_alpha) * flicker;
        return vec4<f32>(in.color.rgb, in.color.a * total_alpha * shape_alpha);
    } else {
        // Horizontal gradient fade (effect_kind 6.0)
        // Fades from full color on left to transparent on right
        let gradient = 1.0 - (in.local_pos.x / in.rect_size.x);
        return vec4<f32>(in.color.rgb, in.color.a * gradient * shape_alpha);
    }
}
