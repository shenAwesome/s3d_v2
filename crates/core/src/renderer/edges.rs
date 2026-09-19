use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[cfg(target_arch = "wasm32")]
pub const MAX_EDGE_WIDTH: u32 = 2048;
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_EDGE_WIDTH: u32 = 2560;
pub const MAX_EDGE_HEIGHT: u32 = 1440;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct EdgeUniformGpu {
    pub edge_color: [f32; 4],
    pub edge_width: f32,
    pub depth_threshold: f32,
    pub normal_threshold: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub texture_width: f32,
    pub texture_height: f32,
    pub enabled: f32,
    pub _pad1: f32,
    pub _pad2: f32,
    pub _pad3: f32,
    pub _pad4: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeConfig {
    pub enabled: bool,
    pub color: [f32; 4],
    pub width: f32,
    pub depth_threshold: f32,
    pub normal_threshold: f32,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            color: [0.15, 0.16, 0.18, 1.0], // Dark charcoal architectural ink
            width: 0.8,                      // 0.8 px crisp, thin outline
            depth_threshold: 0.025,
            normal_threshold: 0.65,
        }
    }
}

pub struct EdgeRenderer {
    pub config: EdgeConfig,
    pub current_width: u32,
    pub current_height: u32,

    // G-Buffer Normal & Object Mask Texture (allocated at MAX_EDGE_WIDTH x MAX_EDGE_HEIGHT)
    pub normal_texture: wgpu::Texture,
    pub normal_view: wgpu::TextureView,

    // Intermediate color texture for post-processing composite
    pub intermediate_texture: wgpu::Texture,
    pub intermediate_view: wgpu::TextureView,

    pub linear_sampler: wgpu::Sampler,
    pub depth_sampler: wgpu::Sampler,

    pub uniform_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,
}

impl EdgeRenderer {
    pub const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        _width: u32,
        _height: u32,
    ) -> Self {
        let w = MAX_EDGE_WIDTH;
        let h = MAX_EDGE_HEIGHT;

        let normal_texture = Self::create_normal_texture(device, w, h);
        let normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let intermediate_texture = Self::create_color_texture(device, surface_format, w, h);
        let intermediate_view = intermediate_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Edge Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let depth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Edge Depth Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let config = EdgeConfig::default();
        let uniform = EdgeUniformGpu {
            edge_color: config.color,
            edge_width: config.width,
            depth_threshold: config.depth_threshold,
            normal_threshold: config.normal_threshold,
            viewport_width: 1280.0,
            viewport_height: 720.0,
            texture_width: w as f32,
            texture_height: h as f32,
            enabled: if config.enabled { 1.0 } else { 0.0 },
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
            _pad4: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Edge Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Edge Bind Group Layout"),
            entries: &[
                // 0: Color Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 1: Color Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 2: Normal Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 3: Normal Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 4: Depth Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 5: Depth Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // 6: Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Edge Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("edges.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Edge Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Edge Composite Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            config,
            current_width: 1280,
            current_height: 720,
            normal_texture,
            normal_view,
            intermediate_texture,
            intermediate_view,
            linear_sampler,
            depth_sampler,
            uniform_buffer,
            bind_group_layout,
            pipeline,
        }
    }

    fn create_normal_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("G-Buffer Normal & Mask Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::NORMAL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_color_texture(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Edge Intermediate Color Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    pub fn resize(&mut self, _device: &wgpu::Device, _surface_format: wgpu::TextureFormat, width: u32, height: u32) {
        self.current_width = width.clamp(64, MAX_EDGE_WIDTH);
        self.current_height = height.clamp(64, MAX_EDGE_HEIGHT);
    }

    pub fn render_edges(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        _source_color_view: &wgpu::TextureView,
        source_color_texture: &wgpu::Texture,
        depth_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        _target_texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) {
        if !self.config.enabled {
            return;
        }

        let render_w = width.clamp(64, MAX_EDGE_WIDTH);
        let render_h = height.clamp(64, MAX_EDGE_HEIGHT);

        // Copy source lit color to intermediate texture so we can read from it and write to target_view
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: source_color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.intermediate_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: render_w,
                height: render_h,
                depth_or_array_layers: 1,
            },
        );

        // Update uniform buffer
        let uniform = EdgeUniformGpu {
            edge_color: self.config.color,
            edge_width: self.config.width,
            depth_threshold: self.config.depth_threshold,
            normal_threshold: self.config.normal_threshold,
            viewport_width: render_w as f32,
            viewport_height: render_h as f32,
            texture_width: MAX_EDGE_WIDTH as f32,
            texture_height: MAX_EDGE_HEIGHT as f32,
            enabled: if self.config.enabled { 1.0 } else { 0.0 },
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
            _pad4: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        // Create bind group reading from intermediate color, normal view, and depth view
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Edge Pass Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.intermediate_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.depth_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Run edge composite pass writing into target_view
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Architectural Edge Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_viewport(0.0, 0.0, render_w as f32, render_h as f32, 0.0, 1.0);
            pass.set_scissor_rect(0, 0, render_w, render_h);

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1); // Fullscreen triangle
        }
    }
}

