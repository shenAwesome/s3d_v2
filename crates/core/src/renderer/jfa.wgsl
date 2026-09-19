// JFA (Jump Flooding Algorithm) Shaders for Selection Highlighting

struct CameraUniform {
    view_proj: mat4x4<f32>,
    eye_pos: vec4<f32>,
};

struct ObjectUniform {
    model: mat4x4<f32>,
    color_override: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var<uniform> object: ObjectUniform;

struct MaskViewportUniform {
    viewport_width: f32,
    viewport_height: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(2) @binding(0)
var<uniform> mask_viewport: MaskViewportUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct MaskVertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_mask(in: VertexInput) -> MaskVertexOutput {
    var out: MaskVertexOutput;
    let world_pos = object.model * vec4<f32>(in.position, 1.0);
    out.position = camera.view_proj * world_pos;
    return out;
}

@fragment
fn fs_mask(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    // Convert screen pixel coordinate to normalized [0, 1] UV
    let uv = frag_pos.xy / vec2<f32>(mask_viewport.viewport_width, mask_viewport.viewport_height);
    return vec4<f32>(uv.x, uv.y, 0.0, 1.0);
}

// ----------------------------------------------------
// Procedural Fullscreen Triangle
// ----------------------------------------------------

struct FullscreenOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) in_vertex_index: u32) -> FullscreenOutput {
    var out: FullscreenOutput;
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.uv = vec2<f32>(x, y);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

// ----------------------------------------------------
// Jump Flooding Step Pass
// ----------------------------------------------------

struct JfaStepUniform {
    step_size: f32,
    viewport_width: f32,
    viewport_height: f32,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> jfa_step: JfaStepUniform;

@group(0) @binding(1)
var t_jfa_input: texture_2d<f32>;

@group(0) @binding(2)
var s_jfa_input: sampler;

@fragment
fn fs_jfa(in: FullscreenOutput) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(jfa_step.viewport_width, jfa_step.viewport_height);
    let texel = vec2<f32>(1.0 / tex_size.x, 1.0 / tex_size.y);
    let step = jfa_step.step_size;

    var best_seed = vec2<f32>(-1.0, -1.0);
    var best_dist_sq = 1e20;

    let current_uv = in.uv;

    // Sample 3x3 kernel around the current pixel at step distance
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let sample_uv = current_uv + vec2<f32>(f32(dx), f32(dy)) * step * texel;

            if (sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0) {
                let seed_coord = textureSampleLevel(t_jfa_input, s_jfa_input, sample_uv, 0.0).xy;

                if (seed_coord.x >= 0.0 && seed_coord.y >= 0.0) {
                    let diff = (current_uv - seed_coord) * tex_size;
                    let dist_sq = dot(diff, diff);

                    if (dist_sq < best_dist_sq) {
                        best_dist_sq = dist_sq;
                        best_seed = seed_coord;
                    }
                }
            }
        }
    }

    return vec4<f32>(best_seed.x, best_seed.y, 0.0, 1.0);
}

// ----------------------------------------------------
// Highlight Composite Pass (Antialiased Outline + Glow)
// ----------------------------------------------------

struct HighlightConfigUniform {
    outline_color: vec4<f32>,
    fill_color: vec4<f32>,
    outline_width: f32,
    glow_radius: f32,
    glow_intensity: f32,
    viewport_width: f32,
    viewport_height: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
};

@group(0) @binding(0)
var<uniform> highlight_config: HighlightConfigUniform;

@group(0) @binding(1)
var t_jfa_result: texture_2d<f32>;

@group(0) @binding(2)
var t_mask_result: texture_2d<f32>;

@group(0) @binding(3)
var s_composite: sampler;

@fragment
fn fs_highlight_composite(in: FullscreenOutput) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(highlight_config.viewport_width, highlight_config.viewport_height);

    let mask_sample = textureSample(t_mask_result, s_composite, in.uv);
    let is_inside = mask_sample.x >= 0.0 && mask_sample.y >= 0.0;

    let nearest_seed = textureSample(t_jfa_result, s_composite, in.uv).xy;

    if (nearest_seed.x < 0.0 || nearest_seed.y < 0.0) {
        if (is_inside) {
            return highlight_config.fill_color;
        }
        discard;
    }

    let diff = (in.uv - nearest_seed) * tex_size;
    let dist_px = length(diff);

    let outline_w = highlight_config.outline_width;
    let glow_r = max(highlight_config.glow_radius, 1.0);

    if (is_inside) {
        // Inside selected object:
        // Translucent fill overlay + subtle inner rim
        let inner_edge = (1.0 - smoothstep(0.0, max(outline_w, 0.1), dist_px)) * 0.35;
        let col = mix(highlight_config.fill_color.rgb, highlight_config.outline_color.rgb, inner_edge);
        let alpha = clamp(highlight_config.fill_color.a + inner_edge * 0.4, 0.0, 1.0);
        return vec4<f32>(col, alpha);
    } else {
        // Outside selected object:
        // Sharp antialiased boundary edge
        let low_edge = max(outline_w - 0.75, 0.0);
        let high_edge = outline_w + 0.75;
        let outline_alpha = (1.0 - smoothstep(low_edge, high_edge, dist_px)) * highlight_config.outline_color.a;
        // Exponential outer halo glow
        let glow_alpha = exp(-dist_px / glow_r) * highlight_config.glow_intensity * highlight_config.outline_color.a;

        let combined_alpha = max(outline_alpha, glow_alpha);
        if (combined_alpha < 0.005) {
            discard;
        }

        let out_rgb = highlight_config.outline_color.rgb;
        return vec4<f32>(out_rgb, combined_alpha);
    }
}
