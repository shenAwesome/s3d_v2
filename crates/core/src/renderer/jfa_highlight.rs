use crate::renderer::mesh::{GpuMesh, Vertex};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

pub const MAX_JFA_WIDTH: u32 = 2560;
pub const MAX_JFA_HEIGHT: u32 = 1440;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaskViewportUniformGpu {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct JfaStepUniformGpu {
    pub step_size: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub _pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct HighlightConfigUniformGpu {
    pub outline_color: [f32; 4],
    pub fill_color: [f32; 4],
    pub outline_width: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub _pad1: f32,
    pub _pad2: f32,
    pub _pad3: f32,
}

#[derive(Debug, Clone)]
pub struct JfaHighlightConfig {
    pub outline_color: [f32; 4],
    pub fill_color: [f32; 4],
    pub outline_width: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
}

impl Default for JfaHighlightConfig {
    fn default() -> Self {
        Self {
            outline_color: [0.0, 0.88, 1.0, 1.0], // Electric Cyan
            fill_color: [0.0, 0.65, 1.0, 0.12],   // Subtle inner translucent tint
            outline_width: 3.5,                   // 3.5 px crisp outline
            glow_radius: 6.0,                     // 6.0 px outer halo
            glow_intensity: 0.50,                 // 50% glow
        }
    }
}

pub struct JfaHighlighter {
    pub config: JfaHighlightConfig,
    pub current_width: u32,
    pub current_height: u32,

    // Textures
    pub mask_texture: wgpu::Texture,
    pub mask_view: wgpu::TextureView,

    pub ping_texture: wgpu::Texture,
    pub ping_view: wgpu::TextureView,

    pub pong_texture: wgpu::Texture,
    pub pong_view: wgpu::TextureView,

    pub linear_sampler: wgpu::Sampler,
    pub nearest_sampler: wgpu::Sampler,

    // Pipelines
    pub mask_pipeline: wgpu::RenderPipeline,
    pub jfa_pipeline: wgpu::RenderPipeline,
    pub composite_pipeline: wgpu::RenderPipeline,

    // Layouts
    pub mask_viewport_bind_group_layout: wgpu::BindGroupLayout,
    pub jfa_step_bind_group_layout: wgpu::BindGroupLayout,
    pub composite_bind_group_layout: wgpu::BindGroupLayout,

    // Buffers & Bind Groups
    pub mask_viewport_buffer: wgpu::Buffer,
    pub mask_viewport_bind_group: wgpu::BindGroup,

    pub config_buffer: wgpu::Buffer,

    // Step uniform buffers for powers of two up to 2048
    pub step_buffers: Vec<(f32, wgpu::Buffer)>,
}

/// Computes the sequence of halving step sizes for Jump Flooding
pub fn compute_jfa_steps(max_dim: f32) -> Vec<f32> {
    let mut step = 1.0;
    while step * 2.0 <= max_dim {
        step *= 2.0;
    }

    let mut pass_steps = Vec::new();
    let mut curr_step = step;
    while curr_step >= 1.0 {
        pass_steps.push(curr_step);
        curr_step /= 2.0;
    }
    // Extra 1-step refinement for 0 JFA boundary errors
    pass_steps.push(1.0);
    pass_steps
}

impl JfaHighlighter {
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let w = width.clamp(64, MAX_JFA_WIDTH);
        let h = height.clamp(64, MAX_JFA_HEIGHT);

        if w == self.current_width && h == self.current_height {
            return;
        }

        self.current_width = w;
        self.current_height = h;

        let jfa_format = wgpu::TextureFormat::Rgba16Float;

        let (mask_texture, mask_view) = Self::create_texture_pair(device, w, h, jfa_format, "JFA Mask Texture");
        let (ping_texture, ping_view) = Self::create_texture_pair(device, w, h, jfa_format, "JFA Ping Texture");
        let (pong_texture, pong_view) = Self::create_texture_pair(device, w, h, jfa_format, "JFA Pong Texture");

        self.mask_texture = mask_texture;
        self.mask_view = mask_view;
        self.ping_texture = ping_texture;
        self.ping_view = ping_view;
        self.pong_texture = pong_texture;
        self.pong_view = pong_view;
    }

    fn create_texture_pair(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: &'static str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn new(
        device: &wgpu::Device,
        global_bind_group_layout: &wgpu::BindGroupLayout,
        object_bind_group_layout: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let jfa_format = wgpu::TextureFormat::Rgba16Float;
        let init_w = 1280;
        let init_h = 720;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("JFA Shaders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("jfa.wgsl").into()),
        });

        // 1. Textures
        let (mask_texture, mask_view) =
            Self::create_texture_pair(device, init_w, init_h, jfa_format, "JFA Mask Texture");
        let (ping_texture, ping_view) =
            Self::create_texture_pair(device, init_w, init_h, jfa_format, "JFA Ping Texture");
        let (pong_texture, pong_view) =
            Self::create_texture_pair(device, init_w, init_h, jfa_format, "JFA Pong Texture");

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("JFA Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("JFA Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 2. Bind Group Layouts
        let mask_viewport_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("JFA Mask Viewport Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let jfa_step_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("JFA Step Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("JFA Composite Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // 3. Render Pipelines
        let mask_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("JFA Mask Pipeline Layout"),
                bind_group_layouts: &[
                    global_bind_group_layout,
                    object_bind_group_layout,
                    &mask_viewport_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let mask_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("JFA Mask Pipeline"),
            layout: Some(&mask_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_mask"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mask"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: jfa_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Render both front and back faces to form a complete silhouette
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let jfa_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("JFA Step Pipeline Layout"),
                bind_group_layouts: &[&jfa_step_bind_group_layout],
                push_constant_ranges: &[],
            });

        let jfa_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("JFA Step Pipeline"),
            layout: Some(&jfa_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_jfa"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: jfa_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("JFA Composite Pipeline Layout"),
                bind_group_layouts: &[&composite_bind_group_layout],
                push_constant_ranges: &[],
            });

        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("JFA Composite Pipeline"),
            layout: Some(&composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_highlight_composite"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 4. Uniform Buffers
        let mask_viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("JFA Mask Viewport Buffer"),
            contents: bytemuck::bytes_of(&MaskViewportUniformGpu {
                viewport_width: 1280.0,
                viewport_height: 720.0,
                _pad1: 0.0,
                _pad2: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mask_viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("JFA Mask Viewport Bind Group"),
            layout: &mask_viewport_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mask_viewport_buffer.as_entire_binding(),
            }],
        });

        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("JFA Highlight Config Buffer"),
            contents: bytemuck::bytes_of(&HighlightConfigUniformGpu {
                outline_color: [0.0, 0.88, 1.0, 1.0],
                fill_color: [0.0, 0.65, 1.0, 0.12],
                outline_width: 3.5,
                glow_radius: 6.0,
                glow_intensity: 0.50,
                viewport_width: 1280.0,
                viewport_height: 720.0,
                _pad1: 0.0,
                _pad2: 0.0,
                _pad3: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Pre-create step uniform buffers for standard steps
        let steps = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0];
        let mut step_buffers = Vec::with_capacity(steps.len());
        for s in steps {
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("JFA Step Buffer {}", s)),
                contents: bytemuck::bytes_of(&JfaStepUniformGpu {
                    step_size: s,
                    viewport_width: 1280.0,
                    viewport_height: 720.0,
                    _pad: 0.0,
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            step_buffers.push((s, buf));
        }

        Self {
            config: JfaHighlightConfig::default(),
            current_width: init_w,
            current_height: init_h,
            mask_texture,
            mask_view,
            ping_texture,
            ping_view,
            pong_texture,
            pong_view,
            linear_sampler,
            nearest_sampler,
            mask_pipeline,
            jfa_pipeline,
            composite_pipeline,
            mask_viewport_bind_group_layout,
            jfa_step_bind_group_layout,
            composite_bind_group_layout,
            mask_viewport_buffer,
            mask_viewport_bind_group,
            config_buffer,
            step_buffers,
        }
    }

    /// Execute the full JFA Selection Highlighting pass
    pub fn render_highlight(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        global_bind_group: &wgpu::BindGroup,
        object_bind_group: &wgpu::BindGroup,
        target_view: &wgpu::TextureView,
        selected_mesh: Option<&GpuMesh>,
        viewport_width: u32,
        viewport_height: u32,
    ) {
        let Some(mesh) = selected_mesh else {
            return; // No selection -> zero work
        };

        // Ensure JFA textures match active viewport dimensions
        self.resize(device, viewport_width, viewport_height);

        let w = self.current_width as f32;
        let h = self.current_height as f32;

        // 1. Update Viewport & Config Uniforms
        queue.write_buffer(
            &self.mask_viewport_buffer,
            0,
            bytemuck::bytes_of(&MaskViewportUniformGpu {
                viewport_width: w,
                viewport_height: h,
                _pad1: 0.0,
                _pad2: 0.0,
            }),
        );

        queue.write_buffer(
            &self.config_buffer,
            0,
            bytemuck::bytes_of(&HighlightConfigUniformGpu {
                outline_color: self.config.outline_color,
                fill_color: self.config.fill_color,
                outline_width: self.config.outline_width,
                glow_radius: self.config.glow_radius,
                glow_intensity: self.config.glow_intensity,
                viewport_width: w,
                viewport_height: h,
                _pad1: 0.0,
                _pad2: 0.0,
                _pad3: 0.0,
            }),
        );

        // Update step buffers with current viewport dimensions
        for (step_val, buf) in &self.step_buffers {
            queue.write_buffer(
                buf,
                0,
                bytemuck::bytes_of(&JfaStepUniformGpu {
                    step_size: *step_val,
                    viewport_width: w,
                    viewport_height: h,
                    _pad: 0.0,
                }),
            );
        }

        // ----------------------------------------------------
        // Pass 1: Render Selected Mesh Silhouette Mask
        // ----------------------------------------------------
        {
            let mut mask_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("JFA Silhouette Mask Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.mask_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: -1.0,
                            g: -1.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            mask_pass.set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
            mask_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);

            mask_pass.set_pipeline(&self.mask_pipeline);
            mask_pass.set_bind_group(0, global_bind_group, &[]);
            mask_pass.set_bind_group(1, object_bind_group, &[]);
            mask_pass.set_bind_group(2, &self.mask_viewport_bind_group, &[]);

            mask_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            mask_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            mask_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }

        // ----------------------------------------------------
        // Pass 2: Jump Flooding Algorithm (Ping-Pong)
        // ----------------------------------------------------
        let max_dim = (viewport_width.max(viewport_height) as f32).max(1.0);
        let pass_steps = compute_jfa_steps(max_dim);

        let mut read_is_ping = false; // Initial pass reads from mask_texture, writes to ping_texture
        let mut final_view = &self.ping_view;

        for (idx, step_val) in pass_steps.iter().enumerate() {
            let (input_view, output_view) = if idx == 0 {
                (&self.mask_view, &self.ping_view)
            } else if read_is_ping {
                (&self.ping_view, &self.pong_view)
            } else {
                (&self.pong_view, &self.ping_view)
            };

            // Find matching step buffer
            let step_buf = self
                .step_buffers
                .iter()
                .find(|(s, _)| (*s - step_val).abs() < 0.1)
                .map(|(_, b)| b)
                .unwrap_or(&self.step_buffers[0].1);

            let step_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("JFA Step {} Bind Group", step_val)),
                layout: &self.jfa_step_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: step_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.nearest_sampler),
                    },
                ],
            });

            {
                let mut jfa_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("JFA Pass Step {}", step_val)),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: -1.0,
                                g: -1.0,
                                b: 0.0,
                                a: 0.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                jfa_pass.set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
                jfa_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
                jfa_pass.set_pipeline(&self.jfa_pipeline);
                jfa_pass.set_bind_group(0, &step_bind_group, &[]);
                jfa_pass.draw(0..3, 0..1);
            }

            final_view = output_view;
            if idx == 0 {
                read_is_ping = true;
            } else {
                read_is_ping = !read_is_ping;
            }
        }

        // ----------------------------------------------------
        // Pass 3: Composite Outline & Glow directly onto Target
        // ----------------------------------------------------
        let composite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("JFA Composite Bind Group"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(final_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.nearest_sampler),
                },
            ],
        });

        {
            let mut composite_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("JFA Outline Composite Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve existing forward pass scene
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            composite_pass.set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
            composite_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
            composite_pass.set_pipeline(&self.composite_pipeline);
            composite_pass.set_bind_group(0, &composite_bind_group, &[]);
            composite_pass.draw(0..3, 0..1);
        }
    }
}

