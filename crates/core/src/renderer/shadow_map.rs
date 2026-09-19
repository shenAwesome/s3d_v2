use glam::{Mat4, Vec3};

#[cfg(target_arch = "wasm32")]
pub const SHADOW_MAP_RES: u32 = 2048;
#[cfg(not(target_arch = "wasm32"))]
pub const SHADOW_MAP_RES: u32 = 4096;

pub struct ShadowMap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub color_sampler: wgpu::Sampler,
    pub light_view_proj: Mat4,
    pub extent: f32,
    pub near: f32,
    pub far: f32,
}

impl ShadowMap {
    pub fn new(device: &wgpu::Device) -> Self {
        let size = wgpu::Extent3d {
            width: SHADOW_MAP_RES,
            height: SHADOW_MAP_RES,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Map Depth Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map Color Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let color_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Map Color Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            color_texture,
            color_view,
            color_sampler,
            light_view_proj: Mat4::IDENTITY,
            extent: 500.0,
            near: 1.0,
            far: 5000.0,
        }
    }

    /// Calculate light view-projection matrix covering active focus area and up-sun casters
    pub fn calculate_light_matrix(sun_dir: Vec3, focus_center: Vec3, focus_radius: f32) -> Mat4 {
        let (vp, _, _, _) = Self::calculate_light_matrix_with_bounds(sun_dir, focus_center, focus_radius);
        vp
    }

    /// Calculate light view-projection matrix along with ortho extent, near plane, and far plane distances
    pub fn calculate_light_matrix_with_bounds(sun_dir: Vec3, focus_center: Vec3, focus_radius: f32) -> (Mat4, f32, f32, f32) {
        let radius = focus_radius.max(300.0);
        let light_dir = if sun_dir.y > 0.02 {
            sun_dir.normalize()
        } else {
            // Fake low-angle twilight sun to keep shadows stable
            Vec3::new(sun_dir.x, 0.02, sun_dir.z).normalize()
        };

        // Tight adaptive extent for high-resolution close-up shadows and smooth scaling when zoomed out
        let extent = radius * 1.35 + 40.0;
        let backward_distance = (extent * 3.5 + 400.0).clamp(500.0, 10000.0);
        let light_pos = focus_center + light_dir * backward_distance;
        let up = if light_dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };

        let view = Mat4::look_at_rh(light_pos, focus_center, up);
        let near = 1.0;
        let far = backward_distance + extent * 2.0 + 300.0;
        let proj = Mat4::orthographic_rh(-extent, extent, -extent, extent, near, far);

        let light_vp = proj * view;

        // Texel snapping in light view-projection space to prevent sub-pixel shadow edge crawling/shimmering
        let shadow_origin = light_vp.transform_point3(focus_center);
        let shadow_origin_2d = glam::Vec2::new(shadow_origin.x, shadow_origin.y);
        let rounded_origin_2d = (shadow_origin_2d * (SHADOW_MAP_RES as f32 * 0.5)).round() / (SHADOW_MAP_RES as f32 * 0.5);
        let round_offset = rounded_origin_2d - shadow_origin_2d;

        let snap_matrix = Mat4::from_translation(glam::Vec3::new(round_offset.x, round_offset.y, 0.0));
        (snap_matrix * light_vp, extent, near, far)
    }

    /// Update light view-projection matrix covering active focus area and up-sun casters
    pub fn update_light_matrix(&mut self, sun_dir: Vec3, focus_center: Vec3, focus_radius: f32) {
        let (vp, extent, near, far) = Self::calculate_light_matrix_with_bounds(sun_dir, focus_center, focus_radius);
        self.light_view_proj = vp;
        self.extent = extent;
        self.near = near;
        self.far = far;
    }
}

