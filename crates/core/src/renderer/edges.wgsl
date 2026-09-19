// Architectural Edge Rendering Shader (Esri SolidEdges3D / Screen-Space Depth & Normal Outline)

struct EdgeUniform {
    edge_color: vec4<f32>,
    edge_width: f32,
    depth_threshold: f32,
    normal_threshold: f32,
    viewport_width: f32,
    viewport_height: f32,
    texture_width: f32,
    texture_height: f32,
    enabled: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    _pad4: f32,
};

@group(0) @binding(0)
var t_color: texture_2d<f32>;
@group(0) @binding(1)
var s_color: sampler;

@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;

@group(0) @binding(4)
var t_depth: texture_depth_2d;
@group(0) @binding(5)
var s_depth: sampler;

@group(0) @binding(6)
var<uniform> params: EdgeUniform;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generates a fullscreen triangle covering [-1, 1] without vertex buffers
    let x = f32(i32(in_vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(in_vertex_index) % 2) * 4.0 - 1.0;
    out.clip_pos = vec4<f32>(x, y, 0.0, 1.0);
    let norm_uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    out.uv = norm_uv * vec2<f32>(params.viewport_width / params.texture_width, params.viewport_height / params.texture_height);
    return out;
}

// Linearize WebGPU standard depth [0, 1] to eye-space relative depth
fn linearize_depth(d: f32) -> f32 {
    return 1.0 / max(1.0 - d, 0.00001);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(t_color, s_color, in.uv);

    if (params.enabled < 0.5) {
        return scene_color;
    }

    let texel_size = vec2<f32>(1.0 / params.texture_width, 1.0 / params.texture_height);
    let sample_r = max(params.edge_width * 0.75, 0.4);
    let off_x = vec2<f32>(texel_size.x * sample_r, 0.0);
    let off_y = vec2<f32>(0.0, texel_size.y * sample_r);
    let off_d = vec2<f32>(texel_size.x * sample_r * 0.7071, texel_size.y * sample_r * 0.7071);

    // 8-Tap isotropic sampling locations (4 cardinal + 4 diagonal)
    let uv_c  = in.uv;
    let uv_l  = in.uv - off_x;
    let uv_r  = in.uv + off_x;
    let uv_t  = in.uv - off_y;
    let uv_b  = in.uv + off_y;
    let uv_tl = in.uv - off_d;
    let uv_br = in.uv + off_d;
    let uv_tr = in.uv + vec2<f32>(off_d.x, -off_d.y);
    let uv_bl = in.uv + vec2<f32>(-off_d.x, off_d.y);

    let norm_c  = textureSample(t_normal, s_normal, uv_c);
    let norm_l  = textureSample(t_normal, s_normal, uv_l);
    let norm_r  = textureSample(t_normal, s_normal, uv_r);
    let norm_t  = textureSample(t_normal, s_normal, uv_t);
    let norm_b  = textureSample(t_normal, s_normal, uv_b);
    let norm_tl = textureSample(t_normal, s_normal, uv_tl);
    let norm_br = textureSample(t_normal, s_normal, uv_br);
    let norm_tr = textureSample(t_normal, s_normal, uv_tr);
    let norm_bl = textureSample(t_normal, s_normal, uv_bl);

    // Only draw edges on 3D geometry / buildings (where alpha/w >= 0.5)
    let is_3d_center = norm_c.a > 0.5;
    let sum_3d = norm_l.a + norm_r.a + norm_t.a + norm_b.a + norm_tl.a + norm_br.a + norm_tr.a + norm_bl.a;
    let is_3d_any = sum_3d > 0.5;
    let is_3d_all = norm_c.a > 0.5 && norm_l.a > 0.5 && norm_r.a > 0.5 && norm_t.a > 0.5 && norm_b.a > 0.5
                 && norm_tl.a > 0.5 && norm_br.a > 0.5 && norm_tr.a > 0.5 && norm_bl.a > 0.5;

    // 2. Isotropic Multi-Axis Depth Discontinuity (Plane-Compensated Step Detection)
    let d_c  = linearize_depth(textureSample(t_depth, s_depth, uv_c));
    let d_l  = linearize_depth(textureSample(t_depth, s_depth, uv_l));
    let d_r  = linearize_depth(textureSample(t_depth, s_depth, uv_r));
    let d_t  = linearize_depth(textureSample(t_depth, s_depth, uv_t));
    let d_b  = linearize_depth(textureSample(t_depth, s_depth, uv_b));
    let d_tl = linearize_depth(textureSample(t_depth, s_depth, uv_tl));
    let d_br = linearize_depth(textureSample(t_depth, s_depth, uv_br));
    let d_tr = linearize_depth(textureSample(t_depth, s_depth, uv_tr));
    let d_bl = linearize_depth(textureSample(t_depth, s_depth, uv_bl));

    if (!is_3d_center && !is_3d_any) {
        return scene_color;
    }

    // 1. Isotropic Normal Sobel Discontinuity (Sharp Crease Angles & Facet Edges)
    let n_l  = norm_l.xyz  * 2.0 - 1.0;
    let n_r  = norm_r.xyz  * 2.0 - 1.0;
    let n_t  = norm_t.xyz  * 2.0 - 1.0;
    let n_b  = norm_b.xyz  * 2.0 - 1.0;
    let n_tl = norm_tl.xyz * 2.0 - 1.0;
    let n_br = norm_br.xyz * 2.0 - 1.0;
    let n_tr = norm_tr.xyz * 2.0 - 1.0;
    let n_bl = norm_bl.xyz * 2.0 - 1.0;

    let sobel_n_x = (n_tr + 2.0 * n_r + n_br) - (n_tl + 2.0 * n_l + n_bl);
    let sobel_n_y = (n_bl + 2.0 * n_b + n_br) - (n_tl + 2.0 * n_t + n_tr);
    let norm_diff = length(sobel_n_x) * 0.25 + length(sobel_n_y) * 0.25;

    let norm_thresh = max(params.normal_threshold, 0.60);
    let normal_edge = clamp((norm_diff - norm_thresh) / max(norm_thresh * 0.35, 0.05), 0.0, 1.0);

    // Multi-axis pairwise second derivative (Laplacian along 4 axes cancels continuous planar tilt)
    let diff_x  = abs(d_l + d_r - 2.0 * d_c);
    let diff_y  = abs(d_t + d_b - 2.0 * d_c);
    let diff_d1 = abs(d_tl + d_br - 2.0 * d_c);
    let diff_d2 = abs(d_tr + d_bl - 2.0 * d_c);
    let max_step = max(max(diff_x, diff_y), max(diff_d1, diff_d2));

    // Relative depth step (distance-invariant: height step proportional to viewing distance)
    let rel_depth_diff = max_step / max(d_c, 0.0001);

    // Dynamic depth threshold (allows small rooftop features to achieve 100% solid opacity)
    let depth_thresh = max(params.depth_threshold * 0.35, 0.005);
    let depth_edge = clamp((rel_depth_diff - depth_thresh) / max(depth_thresh * 0.50, 0.002), 0.0, 1.0);

    // 3. Silhouette Boundary Edge (Building silhouette against ground or background)
    var silhouette_edge = 0.0;
    if ((is_3d_center && !is_3d_all) || (!is_3d_center && is_3d_any)) {
        silhouette_edge = 1.0;
    }

    // Combine all edge factors into a uniform, solid architectural outline
    let total_edge = clamp(max(max(normal_edge, depth_edge), silhouette_edge), 0.0, 1.0);

    if (total_edge < 0.01) {
        return scene_color;
    }

    // Blend edge color onto scene color
    let width_scale = clamp(params.edge_width, 0.2, 1.0);
    let final_rgb = mix(scene_color.rgb, params.edge_color.rgb, total_edge * params.edge_color.a * width_scale);
    return vec4<f32>(final_rgb, scene_color.a);
}
