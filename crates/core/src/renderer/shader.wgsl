// Main PBR/Lambertian Shader with GPU Colored Soft Shadows & Sky Lighting

struct CameraUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye_pos: vec4<f32>,
    viewport: vec4<f32>, // x = width, y = height, z = 1/width, w = 1/height
};

struct LightUniform {
    light_view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,       // xyz = sun direction, w = is_daylight (1.0 or 0.0)
    sun_color: vec4<f32>,     // rgb = sunlight color, a = intensity
    ambient_color: vec4<f32>, // rgb = ambient sky color, a = intensity
    shadow_params: vec4<f32>, // x = world_texel_size, y = uv_texel_size, z = depth_range, w = extent
};

struct ObjectUniform {
    model: mat4x4<f32>,
    color_override: vec4<f32>, // if a > 0.0, use this color
    shadow_color: vec4<f32>,   // custom shadow color for this object
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> light: LightUniform;

@group(1) @binding(0)
var<uniform> object: ObjectUniform;

@group(2) @binding(0)
var t_shadow: texture_depth_2d;
@group(2) @binding(1)
var s_shadow: sampler_comparison;
@group(2) @binding(2)
var t_shadow_color: texture_2d<f32>;
@group(2) @binding(3)
var s_shadow_color: sampler;

struct BasemapUniform {
    opacity: f32,
    brightness: f32,
    grid_mode: f32,
    debug_border: f32,
};

@group(3) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(3) @binding(1)
var s_diffuse: sampler;
@group(3) @binding(2)
var<uniform> basemap_meta: BasemapUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) shadow_pos: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Morph interpolation factor passed via camera.eye_pos.w (0.0 = Planar, 1.0 = Globe)
    let morph = camera.eye_pos.w;
    let is_morph_tile = in.color.a > 0.5 && length(in.color.xyz) > 1000000.0;

    var local_pos = in.position;
    var local_norm = in.normal;
    if (is_morph_tile) {
        local_pos = mix(in.position, in.color.xyz, morph);
        let globe_norm = normalize(in.color.xyz);
        local_norm = normalize(mix(in.normal, globe_norm, morph));
    }

    let world_pos = object.model * vec4<f32>(local_pos, 1.0);
    out.world_pos = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((object.model * vec4<f32>(local_norm, 0.0)).xyz);
    out.uv = in.uv;

    // Color from vertex or uniform override
    if (object.color_override.a > 0.0) {
        out.color = object.color_override;
    } else if (is_morph_tile) {
        out.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    } else {
        out.color = in.color;
    }

    // Shadow coordinates in light projection space
    out.shadow_pos = light.light_view_proj * world_pos;

    return out;
}

// ----------------------------------------------------
// Screen-Space 3D Pixel Line Vertex Shader
// ----------------------------------------------------
@vertex
fn vs_screen_line(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // in.position: world position of this endpoint
    // in.normal: world position of the other endpoint
    // in.uv.x: side offset (-1.0 or +1.0)
    // in.uv.y: line width in screen pixels (e.g. 3.0)

    var clip_curr = camera.view_proj * vec4<f32>(in.position, 1.0);
    var clip_other = camera.view_proj * vec4<f32>(in.normal, 1.0);

    // Near-plane safety clipping to prevent inversion if one endpoint is behind camera
    let near_w = 0.05;
    if (clip_curr.w < near_w && clip_other.w >= near_w) {
        let t = (near_w - clip_curr.w) / (clip_other.w - clip_curr.w);
        clip_curr = mix(clip_curr, clip_other, t);
    } else if (clip_other.w < near_w && clip_curr.w >= near_w) {
        let t = (near_w - clip_other.w) / (clip_curr.w - clip_other.w);
        clip_other = mix(clip_other, clip_curr, t);
    }

    let ndc_curr = clip_curr.xy / max(clip_curr.w, 1e-4);
    let ndc_other = clip_other.xy / max(clip_other.w, 1e-4);

    let vp = max(camera.viewport.xy, vec2<f32>(1.0, 1.0));
    let screen_curr = (ndc_curr * 0.5 + 0.5) * vp;
    let screen_other = (ndc_other * 0.5 + 0.5) * vp;

    let screen_delta = screen_other - screen_curr;
    var screen_dir = normalize(screen_delta);
    if (length(screen_delta) < 1e-3) {
        screen_dir = vec2<f32>(1.0, 0.0);
    }

    // Perpendicular screen normal
    let screen_normal = vec2<f32>(-screen_dir.y, screen_dir.x);

    let width_px = select(3.0, in.uv.y, in.uv.y > 0.0);
    let offset_px = screen_normal * (in.uv.x * width_px * 0.5);

    // Convert pixel offset to clip-space offset
    let offset_clip = (offset_px / vp) * 2.0 * clip_curr.w;

    out.clip_position = vec4<f32>(clip_curr.xy + offset_clip, clip_curr.z, clip_curr.w);
    out.world_pos = in.position;
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
    out.uv = in.uv;
    out.color = in.color;
    out.shadow_pos = vec4<f32>(0.0, 0.0, 0.0, 1.0);

    return out;
}

// ----------------------------------------------------
// 16-Tap Poisson Disk Sample Kernel for Soft Shadows
// ----------------------------------------------------
const POISSON_SAMPLES = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090160),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790)
);

struct ShadowResult {
    visibility: f32,
    color: vec3<f32>,
};

// High-fidelity PCF soft shadows with Normal Offset Bias, Analytic Receiver Plane Depth Tracking & Poisson Disk Filtering
fn compute_shadow(world_pos: vec3<f32>, normal: vec3<f32>, light_dir: vec3<f32>) -> ShadowResult {
    var res: ShadowResult;
    res.visibility = 1.0;
    res.color = vec3<f32>(0.10, 0.12, 0.18);

    // In Globe mode (morph > 0.5), no shadow mapping on planetary surface
    if (camera.eye_pos.w > 0.5) {
        return res;
    }

    let raw_n_dot_l = dot(normal, light_dir);
    // If surface faces away from the sun, it is completely in self-shadow
    if (raw_n_dot_l <= 0.0) {
        res.visibility = 0.0;
        return res;
    }

    let n_dot_l = raw_n_dot_l;
    let sin_theta = sqrt(clamp(1.0 - n_dot_l * n_dot_l, 0.0, 1.0));
    let tan_theta = sin_theta / max(n_dot_l, 0.05);

    let world_texel_size = light.shadow_params.x;
    let uv_texel_size = light.shadow_params.y;
    let depth_range = max(light.shadow_params.z, 100.0);
    let extent = light.shadow_params.w;

    // Robust normal-offset bias: offsets along surface normal proportional to the true world texel size
    // Lifts sampling point along surface normal to clear discrete depth map texel stepping without peter-panning
    let normal_bias_scale = world_texel_size * (0.85 + 1.65 * sin_theta);
    let biased_world_pos = world_pos + normal * normal_bias_scale;
    let shadow_pos = light.light_view_proj * vec4<f32>(biased_world_pos, 1.0);

    let proj_coords = shadow_pos.xyz / shadow_pos.w;
    let uv = vec2<f32>(proj_coords.x * 0.5 + 0.5, -proj_coords.y * 0.5 + 0.5);
    let current_depth = proj_coords.z;

    // If outside shadow frustum, consider unshadowed
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || current_depth > 1.0 || current_depth < 0.0) {
        return res;
    }

    // Sample the caster building's shadow color inside valid frustum
    let sampled_shadow_col = textureSampleLevel(t_shadow_color, s_shadow_color, uv, 0.0);
    if (sampled_shadow_col.a > 0.01) {
        res.color = sampled_shadow_col.rgb;
    }

    // Slope-scaled depth bias: scales with angle tangent to prevent acne on grazing walls and ground
    let bias = clamp(0.00008 * tan_theta + 0.00004, 0.00003, 0.0006);
    let shadow_depth = current_depth - bias;

    // Analytic Receiver Plane Depth Tracking:
    // Calculates exact (dz/du, dz/dv) in shadow map UV space from the surface normal and light camera orientation.
    // Each Poisson PCF sample tap adjusts its test depth to match the true plane slope of the receiver,
    // completely eliminating shadow acne and moiré ripple patterns on curved, sloped, and flat surfaces.
    let up_ref = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(light_dir.y) > 0.95);
    let light_right = normalize(cross(-light_dir, up_ref));
    let light_up = cross(light_right, -light_dir);

    let n_u = dot(normal, light_right);
    let n_v = dot(normal, light_up);
    let n_dot_l_safe = max(raw_n_dot_l, 0.05);

    let slope_factor = (2.0 * extent) / depth_range;
    let dz_du = slope_factor * (n_u / n_dot_l_safe);
    let dz_dv = -slope_factor * (n_v / n_dot_l_safe);
    let dz_duv = vec2<f32>(dz_du, dz_dv);

    // 16-Tap Poisson Disk Soft Shadow PCF (tight 1.25x filter radius for crisp architectural shadows)
    let filter_radius = 1.25 * uv_texel_size;
    var shadow_accum = 0.0;

    for (var i = 0; i < 16; i++) {
        let offset = POISSON_SAMPLES[i] * filter_radius;
        let delta_z = clamp(dot(dz_duv, offset), -0.005, 0.005);
        let tap_depth = shadow_depth + delta_z;
        shadow_accum += textureSampleCompareLevel(
            t_shadow,
            s_shadow,
            uv + offset,
            tap_depth
        );
    }

    // Smooth boundary fade at the edge of the shadow frustum so no hard angular cutoff appears
    let border_fade = smoothstep(0.0, 0.02, uv.x) * smoothstep(1.0, 0.98, uv.x)
                    * smoothstep(0.0, 0.02, uv.y) * smoothstep(1.0, 0.98, uv.y);

    res.visibility = mix(1.0, shadow_accum / 16.0, border_fade);

    return res;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let N = normalize(in.world_normal);
    let L = normalize(light.sun_dir.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);

    let is_daylight = light.sun_dir.w;

    // Diffuse lighting
    let raw_n_dot_l = dot(N, L);
    let n_dot_l = max(raw_n_dot_l, 0.0);

    // Shadow factor with per-caster shadow color
    var shadow = ShadowResult(1.0, vec3<f32>(0.10, 0.12, 0.18));
    if (is_daylight > 1.5) {
        // Mode 2: Diffuse daylight (No cast shadows)
        shadow.visibility = 1.0;
    } else if (is_daylight > 0.5) {
        // Mode 1: Dynamic Shadows calculated from shadow map
        shadow = compute_shadow(in.world_pos, N, L);
    } else {
        // Mode 0: Night (No direct sun)
        shadow.visibility = 0.0;
    }

    let daylight_mult = clamp(is_daylight, 0.0, 1.0);

    // Direct Sunlight
    let direct_sun = light.sun_color.rgb * light.sun_color.a * n_dot_l * shadow.visibility * daylight_mult;

    // Ambient Skylight tinted by caster's custom shadow color
    let sky_factor = clamp(N.y * 0.5 + 0.5, 0.45, 1.0);
    let shadow_ambient_factor = mix(0.70, 1.0, shadow.visibility);
    let is_custom_shadow = max(abs(shadow.color.r - 0.10), max(abs(shadow.color.g - 0.12), abs(shadow.color.b - 0.18))) > 0.04;
    let neutral_shadow_tint = vec3<f32>(0.85, 0.87, 0.90);
    let ambient_tint = mix(select(neutral_shadow_tint, shadow.color * 4.0, is_custom_shadow), vec3<f32>(1.0, 1.0, 1.0), shadow.visibility);
    let ambient = light.ambient_color.rgb * max(light.ambient_color.a, 0.55) * sky_factor * shadow_ambient_factor * ambient_tint;
    let custom_shadow_tint = shadow.color * (1.0 - shadow.visibility) * select(0.0, 0.45, is_custom_shadow);

    // Specular highlight (Blinn-Phong)
    let H = normalize(L + V);
    let n_dot_h = max(dot(N, H), 0.0);
    let specular = pow(n_dot_h, 32.0) * 0.15 * shadow.visibility * daylight_mult;

    let final_rgb = in.color.rgb * (ambient + direct_sun) + custom_shadow_tint + vec3<f32>(specular);
    let fogged_rgb = compute_horizon_fog(in.world_pos, final_rgb);
    let edge_flag = select(0.0, 1.0, object.shadow_color.w > 0.5 || in.uv.y > 0.5);
    let normal_encoded = vec4<f32>(N * 0.5 + 0.5, edge_flag);
    return FragmentOutput(vec4<f32>(fogged_rgb, in.color.a), normal_encoded);
}

// ----------------------------------------------------
// Unlit & Control Object Fragment Shader (Zero Shadow Reception)
// ----------------------------------------------------
@fragment
fn fs_unlit(in: VertexOutput) -> FragmentOutput {
    let N = normalize(in.world_normal);
    let normal_encoded = vec4<f32>(N * 0.5 + 0.5, 0.0);
    let base_color = select(in.color, object.color_override, object.color_override.a > 0.0);
    return FragmentOutput(base_color, normal_encoded);
}

// ----------------------------------------------------
// Screen-Space Line Fragment Shader with Analytic Anti-Aliasing
// ----------------------------------------------------
@fragment
fn fs_screen_line(in: VertexOutput) -> FragmentOutput {
    let N = vec3<f32>(0.0, 1.0, 0.0);
    let normal_encoded = vec4<f32>(N * 0.5 + 0.5, 0.0);

    // in.uv.x is normalized across the line width [-1.0, 1.0]
    // in.uv.y is line width in screen pixels
    let width_px = max(in.uv.y, 1.0);
    let dist = abs(in.uv.x);

    // Analytic smoothstep falloff for the outer 1 pixel border
    let aa_edge = max(1.0 - (1.0 / width_px), 0.0);
    let edge_aa = 1.0 - smoothstep(aa_edge, 1.0, dist);

    let final_color = vec4<f32>(in.color.rgb, in.color.a * edge_aa);
    return FragmentOutput(final_color, normal_encoded);
}

@fragment
fn fs_threedtile(in: VertexOutput) -> FragmentOutput {
    let N = normalize(in.world_normal);
    let L = normalize(light.sun_dir.xyz);
    let V = normalize(camera.eye_pos.xyz - in.world_pos);

    let is_daylight = light.sun_dir.w;

    // Diffuse lighting
    let raw_n_dot_l = dot(N, L);
    let n_dot_l = max(raw_n_dot_l, 0.0);

    // Shadow factor with per-caster shadow color
    var shadow = ShadowResult(1.0, vec3<f32>(0.10, 0.12, 0.18));
    if (is_daylight > 1.5) {
        shadow.visibility = 1.0;
    } else if (is_daylight > 0.5) {
        shadow = compute_shadow(in.world_pos, N, L);
    } else {
        shadow.visibility = 0.0;
    }

    let daylight_mult = clamp(is_daylight, 0.0, 1.0);

    // Direct Sunlight
    let direct_sun = light.sun_color.rgb * light.sun_color.a * n_dot_l * shadow.visibility * daylight_mult;

    // Ambient Skylight tinted by caster's custom shadow color
    let sky_factor = clamp(N.y * 0.5 + 0.5, 0.45, 1.0);
    let shadow_ambient_factor = mix(0.70, 1.0, shadow.visibility);
    let is_custom_shadow_tile = max(abs(shadow.color.r - 0.10), max(abs(shadow.color.g - 0.12), abs(shadow.color.b - 0.18))) > 0.04;
    let neutral_shadow_tint_tile = vec3<f32>(0.85, 0.87, 0.90);
    let ambient_tint = mix(select(neutral_shadow_tint_tile, shadow.color * 4.0, is_custom_shadow_tile), vec3<f32>(1.0, 1.0, 1.0), shadow.visibility);
    let ambient = light.ambient_color.rgb * max(light.ambient_color.a, 0.55) * sky_factor * shadow_ambient_factor * ambient_tint;
    let custom_shadow_tint = shadow.color * (1.0 - shadow.visibility) * select(0.0, 0.45, is_custom_shadow_tile);

    // 3D Tiles / glTF PBR Diffuse Shading:
    // If replace_texture is enabled (object.shadow_color.x > 0.5), use solid color override.
    // Otherwise, if texture is present (basemap_meta.grid_mode > 0.5), use the original texture as baseColor.
    // Otherwise, use vertex / default base color.
    let replace_texture = object.shadow_color.x > 0.5;
    var base_rgb: vec3<f32>;

    if (replace_texture) {
        base_rgb = object.color_override.rgb;
    } else if (basemap_meta.grid_mode > 0.5) {
        base_rgb = textureSample(t_diffuse, s_diffuse, in.uv).rgb;
    } else {
        base_rgb = in.color.rgb;
    }

    // Specular highlight (Blinn-Phong)
    let H = normalize(L + V);
    let n_dot_h = max(dot(N, H), 0.0);
    let specular = pow(n_dot_h, 32.0) * 0.15 * shadow.visibility * daylight_mult;

    let final_rgb = base_rgb * (ambient + direct_sun) + custom_shadow_tint + vec3<f32>(specular);
    let fogged_rgb = compute_horizon_fog(in.world_pos, final_rgb);
    let normal_encoded = vec4<f32>(N * 0.5 + 0.5, 1.0); // 1.0 = 3D architectural object
    return FragmentOutput(vec4<f32>(fogged_rgb, in.color.a), normal_encoded);
}

// ----------------------------------------------------
// Dynamic Atmospheric Sky & Solar Disc Shaders
// ----------------------------------------------------

struct SkyVertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) in_vertex_index: u32) -> SkyVertexOutput {
    var out: SkyVertexOutput;
    // Generate fullscreen triangle covering NDC [-1, 1] at depth z = 1.0 (far plane)
    let x = f32(i32(in_vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(in_vertex_index) % 2) * 4.0 - 1.0;
    out.clip_pos = vec4<f32>(x, y, 1.0, 1.0);

    // Unproject far-plane NDC point (x, y, 1.0, 1.0) into world space
    let world_pos_unnorm = camera.inv_view_proj * vec4<f32>(x, y, 1.0, 1.0);
    let world_pos = world_pos_unnorm.xyz / world_pos_unnorm.w;
    out.view_dir = normalize(world_pos - camera.eye_pos.xyz);

    return out;
}

@fragment
fn fs_sky(in: SkyVertexOutput) -> FragmentOutput {
    let V = normalize(in.view_dir);
    let L = normalize(light.sun_dir.xyz);
    let is_daylight = light.sun_dir.w;
    let morph = camera.eye_pos.w;

    // In 3D Globe mode, use deep cosmic space background
    if (morph > 0.5) {
        let space_color = vec3<f32>(0.02, 0.03, 0.05);
        return FragmentOutput(vec4<f32>(space_color, 1.0), vec4<f32>(0.0, 0.0, 0.0, 0.0));
    }

    let sun_dot = max(dot(V, L), 0.0);
    let sun_elev = L.y; // Positive = above horizon, Negative = below horizon

    // 1. Rayleigh Atmospheric Sky Gradient (Daylight / Golden Hour / Twilight / Night)
    let alt = V.y; // -1.0 (down) to +1.0 (zenith)
    
    // Daylight sky colors
    let zenith_day = vec3<f32>(0.18, 0.42, 0.82);   // Deep sky blue
    let mid_day    = vec3<f32>(0.42, 0.65, 0.88);   // Azure
    let horizon_day = vec3<f32>(0.76, 0.84, 0.93);  // Warm bright atmospheric haze
    let ground_haze = vec3<f32>(0.18, 0.22, 0.28);  // Earth ground haze

    // Golden Hour / Sunset colors (when sun_elev is low)
    let sunset_horizon = vec3<f32>(0.96, 0.52, 0.22); // Radiant orange/gold
    let sunset_zenith  = vec3<f32>(0.12, 0.20, 0.45); // Twilight indigo

    // Night colors
    let night_sky = vec3<f32>(0.03, 0.04, 0.07);
    let night_horizon = vec3<f32>(0.06, 0.08, 0.12);

    // Day / Sunset / Night transition factors
    let day_factor = smoothstep(-0.05, 0.20, sun_elev) * is_daylight;
    let sunset_factor = (1.0 - smoothstep(0.05, 0.35, sun_elev)) * day_factor;

    var sky_base: vec3<f32>;
    if (alt > 0.0) {
        let t = pow(alt, 0.55);
        let day_col = mix(horizon_day, mix(mid_day, zenith_day, t), t);
        let sunset_col = mix(sunset_horizon, sunset_zenith, t);
        let night_col = mix(night_horizon, night_sky, t);

        let daytime = mix(day_col, sunset_col, sunset_factor);
        sky_base = mix(night_col, daytime, day_factor);
    } else {
        // Below horizon terrestrial haze
        let t = clamp(-alt * 5.0, 0.0, 1.0);
        let day_ground = mix(horizon_day, ground_haze, t);
        let sunset_ground = mix(sunset_horizon, ground_haze * 0.8, t);
        let night_ground = mix(night_horizon, ground_haze * 0.3, t);

        let daytime = mix(day_ground, sunset_ground, sunset_factor);
        sky_base = mix(night_ground, daytime, day_factor);
    }

    // 2. Solar Disc & Atmospheric Mie Corona Halo
    var solar_contrib = vec3<f32>(0.0);
    if (is_daylight > 0.5) {
        // Crisp Sun Disc (0.53 degree diameter)
        let sun_disc = smoothstep(0.9994, 0.9998, sun_dot);
        let sun_color = light.sun_color.rgb * light.sun_color.a * 3.5;

        // Multi-tier Mie scattering corona and ambient solar halo
        let corona = pow(sun_dot, 64.0) * 0.70;
        let halo = pow(sun_dot, 8.0) * 0.25;
        let golden_halo_color = mix(light.sun_color.rgb, vec3<f32>(1.0, 0.75, 0.40), sunset_factor);

        solar_contrib = sun_color * sun_disc + golden_halo_color * (corona + halo) * day_factor;
    }

    let final_sky_rgb = sky_base + solar_contrib;
    return FragmentOutput(vec4<f32>(final_sky_rgb, 1.0), vec4<f32>(0.0, 0.0, 0.0, 0.0));
}

// Atmospheric Distance Fog for smooth horizon blending
fn compute_horizon_fog(world_pos: vec3<f32>, base_color: vec3<f32>) -> vec3<f32> {
    let morph = camera.eye_pos.w;
    if (morph > 0.5) {
        return base_color;
    }

    let dist = length(camera.eye_pos.xyz - world_pos);
    let fog_factor = clamp(1.0 - exp(-pow(dist / 32000.0, 2.2)), 0.0, 1.0);

    let is_daylight = light.sun_dir.w;
    let sun_elev = light.sun_dir.y;
    let day_factor = smoothstep(-0.05, 0.20, sun_elev) * is_daylight;
    let sunset_factor = (1.0 - smoothstep(0.05, 0.35, sun_elev)) * day_factor;

    let horizon_day = vec3<f32>(0.76, 0.84, 0.93);
    let horizon_sunset = vec3<f32>(0.96, 0.52, 0.22);
    let horizon_night = vec3<f32>(0.06, 0.08, 0.12);

    let daytime = mix(horizon_day, horizon_sunset, sunset_factor);
    let horizon_haze = mix(horizon_night, daytime, day_factor);

    return mix(base_color, horizon_haze, fog_factor);
}

// ----------------------------------------------------
// Shadow Multi-Target Pass Shaders (Depth + Shadow Color)
// ----------------------------------------------------

struct ShadowVertexInput {
    @location(0) position: vec3<f32>,
};

@vertex
fn vs_shadow(in: ShadowVertexInput) -> @builtin(position) vec4<f32> {
    let world_pos = object.model * vec4<f32>(in.position, 1.0);
    return light.light_view_proj * world_pos;
}

@fragment
fn fs_shadow() -> @location(0) vec4<f32> {
    return object.shadow_color;
}

// ----------------------------------------------------
// CAD-Style Procedural Anti-Aliased Grid Computation
// ----------------------------------------------------
fn compute_cad_grid(world_pos: vec3<f32>) -> vec4<f32> {
    let p = world_pos.xz;
    
    // Scale 1: Minor 1-meter grid with Nyquist anti-aliasing
    let dudv1 = fwidth(p);
    let nyquist1 = clamp(1.0 - 2.0 * max(dudv1.x, dudv1.y), 0.0, 1.0);
    let grid1 = abs(fract(p - 0.5) - 0.5) / max(dudv1, vec2<f32>(0.0001, 0.0001));
    let line1 = min(grid1.x, grid1.y);
    let minor_c = (1.0 - min(line1, 1.0)) * nyquist1;
    
    // Scale 2: Major 10-meter grid with Nyquist anti-aliasing
    let p10 = p * 0.1;
    let dudv10 = fwidth(p10);
    let nyquist10 = clamp(1.0 - 2.0 * max(dudv10.x, dudv10.y), 0.0, 1.0);
    let grid10 = abs(fract(p10 - 0.5) - 0.5) / max(dudv10, vec2<f32>(0.0001, 0.0001));
    let line10 = min(grid10.x, grid10.y);
    let major_c = (1.0 - min(line10, 1.0)) * nyquist10;

    // Scale 3: Super 100-meter grid with Nyquist anti-aliasing
    let p100 = p * 0.01;
    let dudv100 = fwidth(p100);
    let nyquist100 = clamp(1.0 - 2.0 * max(dudv100.x, dudv100.y), 0.0, 1.0);
    let grid100 = abs(fract(p100 - 0.5) - 0.5) / max(dudv100, vec2<f32>(0.0001, 0.0001));
    let line100 = min(grid100.x, grid100.y);
    let super_c = (1.0 - min(line100, 1.0)) * nyquist100;

    // Origin Coordinate Axes: X = 0 (North/South axis), Z = 0 (East/West axis)
    let axis_x = (1.0 - min(abs(p.x) / max(dudv1.x, 0.0001), 1.0)) * nyquist10;
    let axis_z = (1.0 - min(abs(p.y) / max(dudv1.y, 0.0001), 1.0)) * nyquist10;

    // Distance fade factors so fine lines smoothly dissolve at distant horizons
    let dist = length(camera.eye_pos.xyz - world_pos);
    let fade1 = clamp(1.0 - dist / 350.0, 0.0, 1.0);
    let fade10 = clamp(1.0 - dist / 2500.0, 0.0, 1.0);
    let fade100 = clamp(1.0 - dist / 12000.0, 0.0, 1.0);

    // Architectural CAD Slate Background Surface
    let base_surface = vec3<f32>(0.13, 0.15, 0.18);

    // Grid Line Colors
    let minor_color = vec3<f32>(0.20, 0.23, 0.28);
    let major_color = vec3<f32>(0.30, 0.34, 0.42);
    let super_color = vec3<f32>(0.42, 0.47, 0.56);
    
    // Origin Axes: Red for X-axis (East), Blue for Z-axis (North)
    let axis_x_col = vec3<f32>(0.75, 0.28, 0.28);
    let axis_z_col = vec3<f32>(0.28, 0.55, 0.85);

    var surface_col = base_surface;
    surface_col = mix(surface_col, minor_color, minor_c * fade1 * 0.50);
    surface_col = mix(surface_col, major_color, major_c * fade10 * 0.75);
    surface_col = mix(surface_col, super_color, super_c * fade100 * 0.85);
    surface_col = mix(surface_col, axis_z_col, axis_z * fade100 * 0.90);
    surface_col = mix(surface_col, axis_x_col, axis_x * fade100 * 0.90);

    return vec4<f32>(surface_col, 1.0);
}

// ----------------------------------------------------
// Flat Ground CAD Grid Fragment Shader
// ----------------------------------------------------
@fragment
fn fs_ground(in: VertexOutput) -> FragmentOutput {
    let morph = camera.eye_pos.w;
    if (morph > 0.5) {
        // Uniform clean deep blue ocean on 3D Earth Globe
        let ocean_color = vec3<f32>(0.07, 0.16, 0.28);
        return FragmentOutput(vec4<f32>(ocean_color, 1.0), vec4<f32>(0.0, 0.0, 0.0, 0.0));
    }

    let grid_color = compute_cad_grid(in.world_pos);

    let N = normalize(in.world_normal);
    let L = normalize(light.sun_dir.xyz);
    let is_daylight = light.sun_dir.w;

    let raw_n_dot_l = dot(N, L);
    let n_dot_l = max(raw_n_dot_l, 0.0);

    var shadow = ShadowResult(1.0, vec3<f32>(0.10, 0.12, 0.18));
    if (is_daylight > 1.5) {
        shadow.visibility = 1.0;
    } else if (is_daylight > 0.5) {
        shadow = compute_shadow(in.world_pos, N, L);
    } else {
        shadow.visibility = 0.0;
    }

    let daylight_mult = clamp(is_daylight, 0.0, 1.0);
    let direct_sun = light.sun_color.rgb * light.sun_color.a * n_dot_l * shadow.visibility * daylight_mult;
    let shadow_ambient_factor = mix(0.65, 1.0, shadow.visibility);
    let is_custom_shadow_plane = max(abs(shadow.color.r - 0.10), max(abs(shadow.color.g - 0.12), abs(shadow.color.b - 0.18))) > 0.04;
    let ambient_tint = mix(shadow.color * select(2.8, 4.0, is_custom_shadow_plane), vec3<f32>(1.0, 1.0, 1.0), shadow.visibility);
    let ambient = light.ambient_color.rgb * max(light.ambient_color.a, 0.50) * shadow_ambient_factor * ambient_tint;
    let custom_shadow_tint = shadow.color * (1.0 - shadow.visibility) * select(0.0, 0.45, is_custom_shadow_plane);

    let lit_rgb = grid_color.rgb * (ambient + direct_sun) + custom_shadow_tint;
    let fogged_rgb = compute_horizon_fog(in.world_pos, lit_rgb);
    let normal_encoded = vec4<f32>(N * 0.5 + 0.5, 0.0); // 0.0 = ground plane

    return FragmentOutput(vec4<f32>(fogged_rgb, 1.0), normal_encoded);
}

// ----------------------------------------------------
// Textured Basemap / 3D Terrain Grid Fragment Shader
// ----------------------------------------------------

@fragment
fn fs_basemap(in: VertexOutput) -> FragmentOutput {
    var base_color: vec4<f32>;
    if (basemap_meta.grid_mode > 0.5) {
        let grid_col = compute_cad_grid(in.world_pos);
        if (basemap_meta.grid_mode > 1.5) {
            // Texture + Grid Overlay (Hybrid)
            let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
            base_color = vec4<f32>(mix(tex_color.rgb, grid_col.rgb, 0.35), tex_color.a);
        } else {
            // Pure CAD Grid Ground / 3D Terrain Surface (Basemap OFF)
            base_color = grid_col;
        }
    } else {
        // Pure Textured Basemap (Basemap ON)
        base_color = textureSample(t_diffuse, s_diffuse, in.uv);
    }

    var is_tile_border = false;
    if (basemap_meta.debug_border > 0.5) {
        let fw = fwidth(in.uv);
        let border_px = 2.0;
        let border_u = min(border_px * max(fw.x, 1e-5), 0.05);
        let border_v = min(border_px * max(fw.y, 1e-5), 0.05);
        is_tile_border = in.uv.x < border_u || in.uv.x > (1.0 - border_u)
                      || in.uv.y < border_v || in.uv.y > (1.0 - border_v);
    }

    let morph = camera.eye_pos.w;
    if (morph > 0.5) {
        if (is_tile_border) {
            return FragmentOutput(vec4<f32>(1.0, 0.05, 0.05, 1.0), vec4<f32>(0.0, 0.0, 0.0, 0.0));
        }
        // Unshaded, uniform natural basemap imagery on 3D Globe
        let final_alpha = base_color.a * basemap_meta.opacity;
        return FragmentOutput(vec4<f32>(base_color.rgb, final_alpha), vec4<f32>(0.0, 0.0, 0.0, 0.0));
    }

    let N = normalize(in.world_normal);
    let normal_encoded = vec4<f32>(N * 0.5 + 0.5, 0.0); // 0.0 = basemap ground plane

    if (is_tile_border) {
        return FragmentOutput(vec4<f32>(1.0, 0.05, 0.05, 1.0), normal_encoded);
    }
    let L = normalize(light.sun_dir.xyz);
    let is_daylight = light.sun_dir.w;

    let raw_n_dot_l = dot(N, L);
    let n_dot_l = max(raw_n_dot_l, 0.0);

    var shadow = ShadowResult(1.0, vec3<f32>(0.10, 0.12, 0.18));
    if (is_daylight > 1.5) {
        shadow.visibility = 1.0;
    } else if (is_daylight > 0.5) {
        shadow = compute_shadow(in.world_pos, N, L);
    } else {
        shadow.visibility = 0.0;
    }

    let daylight_mult = clamp(is_daylight, 0.0, 1.0);
    let direct_sun = light.sun_color.rgb * light.sun_color.a * n_dot_l * shadow.visibility * daylight_mult;
    let shadow_ambient_factor = mix(0.55, 1.0, shadow.visibility);
    let is_custom_shadow_base = max(abs(shadow.color.r - 0.10), max(abs(shadow.color.g - 0.12), abs(shadow.color.b - 0.18))) > 0.04;
    let ambient_tint = mix(shadow.color * select(2.8, 4.0, is_custom_shadow_base), vec3<f32>(1.0, 1.0, 1.0), shadow.visibility);
    let ambient = light.ambient_color.rgb * max(light.ambient_color.a, 0.50) * shadow_ambient_factor * ambient_tint;
    let custom_shadow_tint = shadow.color * (1.0 - shadow.visibility) * select(0.0, 0.45, is_custom_shadow_base);

    let lit_rgb = base_color.rgb * (ambient + direct_sun) + custom_shadow_tint;
    let fogged_rgb = compute_horizon_fog(in.world_pos, lit_rgb);
    let final_alpha = base_color.a * basemap_meta.opacity;

    return FragmentOutput(vec4<f32>(fogged_rgb, final_alpha), normal_encoded);
}
