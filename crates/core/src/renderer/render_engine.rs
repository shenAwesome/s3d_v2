use crate::gis::basemap::{DecodedTile, TileCoord};
use crate::gis::cache::ResourceBudget;
use crate::gis::crs::ProjectOrigin;
use crate::gis::extrusion::RawMeshData;
use crate::renderer::camera::Camera;
use crate::renderer::jfa_highlight::JfaHighlighter;
use crate::renderer::mesh::GpuMesh;
use crate::renderer::pipeline::{
    BasemapUniformGpu, CameraUniformGpu, LightUniformGpu, ObjectUniformGpu, RenderPipelines,
};
use crate::renderer::shadow_map::ShadowMap;
use crate::scene::scene::Scene;
use crate::solar::sun_calc::SolarPosition;
use crate::spatial::measurement::MeasurementEngine;
use bytemuck::Zeroable;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[cfg(target_arch = "wasm32")]
pub const MAX_VIEWPORT_WIDTH: u32 = 2048;
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_VIEWPORT_WIDTH: u32 = 2560;
pub const MAX_VIEWPORT_HEIGHT: u32 = 1440;

pub struct GpuTile {
    pub coord: TileCoord,
    pub has_texture: bool,
    pub width: u32,
    pub height: u32,
    pub mesh: GpuMesh,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub uniform_buffer: wgpu::Buffer,
    pub last_drawn_frame: u64,
}

pub struct BatchedMeshChunk {
    pub mesh: GpuMesh,
    pub aabb_min: glam::Vec3,
    pub aabb_max: glam::Vec3,
    pub is_visible: bool,
    pub cast_shadows: bool,
    pub shadow_color: [f32; 4],
    pub bind_group: wgpu::BindGroup,
}

pub struct I3SGpuTile {
    pub node_id: u32,
    pub mesh: GpuMesh,
    pub edge_mesh: Option<GpuMesh>,
    pub bind_group: wgpu::BindGroup,
    pub texture_bind_group: wgpu::BindGroup,
    pub uniform_buffer: wgpu::Buffer,
    pub last_drawn_frame: u64,
    pub has_vertex_colors: bool,
    pub has_texture: bool,
}

pub struct ThreeDTileSubMeshGpu {
    pub index_offset: u32,
    pub index_count: u32,
    pub bind_group: wgpu::BindGroup,
    pub uniform_buffer: wgpu::Buffer,
    pub texture_bind_group: wgpu::BindGroup,
    pub base_color: [f32; 4],
}

pub struct ThreeDTileGpuMesh {
    pub id: String,
    pub mesh: GpuMesh,
    pub bounding_box_mesh: Option<GpuMesh>,
    pub submeshes: Vec<ThreeDTileSubMeshGpu>,
    pub last_drawn_frame: u64,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

pub struct CustomShadowCaster {
    pub mesh: GpuMesh,
    pub shadow_color: [f32; 4],
    pub bind_group: wgpu::BindGroup,
}

pub struct RenderEngine {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub pipelines: RenderPipelines,
    pub shadow_map: ShadowMap,
    pub jfa_highlighter: JfaHighlighter,
    pub edge_renderer: crate::renderer::edges::EdgeRenderer,

    // Uniform Buffers & Global Bind Groups
    pub camera_buffer: wgpu::Buffer,
    pub light_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
    pub shadow_bind_group: wgpu::BindGroup,

    // Object Bind Groups Cache
    pub default_object_bind_group: wgpu::BindGroup,
    pub selected_object_bind_group: wgpu::BindGroup,
    pub node_object_bind_groups: Vec<(wgpu::Buffer, wgpu::BindGroup)>,

    // Offscreen render target for egui display
    pub target_texture: wgpu::Texture,
    pub target_view: wgpu::TextureView,
    pub sample_texture: wgpu::Texture,
    pub sample_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub current_width: u32,
    pub current_height: u32,
    pub egui_texture_id: egui::TextureId,

    // Basemap GPU Tiles
    pub basemap_sampler: wgpu::Sampler,
    pub gpu_tiles: HashMap<TileCoord, GpuTile>,
    pub active_tiles_to_draw: Vec<TileCoord>,
    pub current_basemap_grid_mode: f32,
    pub current_basemap_opacity: f32,
    pub current_basemap_debug_borders: bool,
    pub budget: ResourceBudget,
    pub frame_count: u64,

    // Esri SceneLayer (I3S) GPU Tiles
    pub i3s_gpu_tiles: HashMap<u32, I3SGpuTile>,
    pub active_i3s_nodes_to_draw: Vec<u32>,
    pub i3s_cast_shadows: bool,

    // OGC 3D Tiles (Cesium 3D Tiles) GPU Meshes
    pub threedtiles_gpu_tiles: HashMap<String, ThreeDTileGpuMesh>,
    pub active_threedtiles_to_draw: Vec<String>,
    pub show_3dtiles_bounding_boxes: bool,
    pub default_threedtile_texture_bind_group: wgpu::BindGroup,

    // Projection Mode (Planar ENU vs 3D Globe ECEF) & Terrain Morphing
    pub projection_mode: crate::gis::crs::ProjectionMode,
    pub morph_progress: f32,
    pub los_visible: bool,
    pub measurement_visible: bool,

    // GPU Meshes & Batched Chunks
    pub batched_chunks: Vec<BatchedMeshChunk>,
    pub meshes: Vec<GpuMesh>,
    pub ground_mesh: GpuMesh,
    pub earth_sphere: GpuMesh,

    // Dynamic Selected Feature & Custom Shadow Casters
    pub selected_mesh: Option<GpuMesh>,
    pub custom_shadow_casters: HashMap<String, CustomShadowCaster>,
}

impl RenderEngine {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        render_state: &egui_wgpu::RenderState,
    ) -> Self {
        let surface_format = wgpu::TextureFormat::Rgba8Unorm;
        let pipelines = RenderPipelines::new(&device, surface_format);
        let shadow_map = ShadowMap::new(&device);

        let w = MAX_VIEWPORT_WIDTH;
        let h = MAX_VIEWPORT_HEIGHT;

        // 1. Offscreen render target
        let target_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Render Target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 1b. Sampleable destination texture for egui sampling
        let sample_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Sample Target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sample_view = sample_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 2. Main Depth target
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Depth Target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        #[cfg(target_arch = "wasm32")]
        let egui_view = &target_view;
        #[cfg(not(target_arch = "wasm32"))]
        let egui_view = &sample_view;

        // Register texture view with egui renderer once
        let egui_texture_id = render_state.renderer.write().register_native_texture(
            &device,
            egui_view,
            wgpu::FilterMode::Linear,
        );

        // 3. Create Uniform Buffers
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&CameraUniformGpu::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::bytes_of(&LightUniformGpu::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group"),
            layout: &pipelines.global_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: &pipelines.shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_map.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_map.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&shadow_map.color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&shadow_map.color_sampler),
                },
            ],
        });

        // 4. Object Uniform Buffers
        let default_obj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Default Object Uniform Buffer"),
            contents: bytemuck::bytes_of(&ObjectUniformGpu {
                model: Mat4::IDENTITY.to_cols_array_2d(),
                color_override: [0.0, 0.0, 0.0, 0.0],
                shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let default_object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Default Object Bind Group"),
            layout: &pipelines.object_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: default_obj_buffer.as_entire_binding(),
            }],
        });

        let selected_obj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Selected Object Uniform Buffer"),
            contents: bytemuck::bytes_of(&ObjectUniformGpu {
                model: Mat4::IDENTITY.to_cols_array_2d(),
                color_override: [0.25, 0.65, 1.0, 1.0], // Electric blue selection
                shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let selected_object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Selected Object Bind Group"),
            layout: &pipelines.object_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: selected_obj_buffer.as_entire_binding(),
            }],
        });

        let ground_mesh = GpuMesh::create_ground_plane(&device, 100_000.0, 100.0);
        let earth_sphere = GpuMesh::create_earth_sphere(&device, 64, 128);

        let basemap_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Basemap Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let jfa_highlighter = JfaHighlighter::new(
            &device,
            &pipelines.global_bind_group_layout,
            &pipelines.object_bind_group_layout,
            surface_format,
        );

        let edge_renderer = crate::renderer::edges::EdgeRenderer::new(&device, surface_format, w, h);

        let white_3dtile_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default White 3D Tile Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_3dtile_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_3dtile_view = white_3dtile_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let default_3dtile_meta_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Default 3D Tile Meta Buffer"),
            contents: bytemuck::bytes_of(&BasemapUniformGpu {
                opacity: 0.0,
                brightness: 1.0,
                grid_mode: 0.0,
                debug_border: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let default_threedtile_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Default 3D Tile Texture Bind Group"),
            layout: &pipelines.basemap_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_3dtile_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&basemap_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: default_3dtile_meta_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            device,
            queue,
            pipelines,
            shadow_map,
            jfa_highlighter,
            edge_renderer,
            camera_buffer,
            light_buffer,
            global_bind_group,
            shadow_bind_group,
            default_object_bind_group,
            selected_object_bind_group,
            node_object_bind_groups: Vec::new(),
            target_texture,
            target_view,
            sample_texture,
            sample_view,
            depth_texture,
            depth_view,
            current_width: 1280,
            current_height: 720,
            egui_texture_id,
            basemap_sampler,
            gpu_tiles: HashMap::new(),
            active_tiles_to_draw: Vec::new(),
            current_basemap_grid_mode: 0.0,
            current_basemap_opacity: 1.0,
            current_basemap_debug_borders: false,
            budget: ResourceBudget::default(),
            frame_count: 0,
            i3s_gpu_tiles: HashMap::new(),
            active_i3s_nodes_to_draw: Vec::new(),
            i3s_cast_shadows: true,
            threedtiles_gpu_tiles: HashMap::new(),
            active_threedtiles_to_draw: Vec::new(),
            show_3dtiles_bounding_boxes: true,
            default_threedtile_texture_bind_group,
            projection_mode: crate::gis::crs::ProjectionMode::PlanarENU,
            morph_progress: 0.0,
            los_visible: true,
            measurement_visible: true,
            batched_chunks: Vec::new(),
            meshes: Vec::new(),
            ground_mesh,
            earth_sphere,
            selected_mesh: None,
            custom_shadow_casters: HashMap::new(),
        }
    }

    pub fn ensure_node_uniform_capacity(&mut self, count: usize) {
        while self.node_object_bind_groups.len() < count {
            let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Node Object Uniform Buffer {}", self.node_object_bind_groups.len())),
                contents: bytemuck::bytes_of(&ObjectUniformGpu {
                    model: Mat4::IDENTITY.to_cols_array_2d(),
                    color_override: [0.0, 0.0, 0.0, 0.0],
                    shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Node Object Bind Group {}", self.node_object_bind_groups.len())),
                layout: &self.pipelines.object_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            self.node_object_bind_groups.push((buffer, bind_group));
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let new_w = width.clamp(64, MAX_VIEWPORT_WIDTH);
        let new_h = height.clamp(64, MAX_VIEWPORT_HEIGHT);
        if new_w != self.current_width || new_h != self.current_height {
            self.current_width = new_w;
            self.current_height = new_h;
            self.jfa_highlighter.resize(&self.device, self.current_width, self.current_height);
            self.edge_renderer.resize(&self.device, wgpu::TextureFormat::Rgba8Unorm, self.current_width, self.current_height);
        }
    }

    pub fn update_tile_terrain_mesh(
        &mut self,
        coord: TileCoord,
        origin: &ProjectOrigin,
        terrain: Option<&crate::gis::terrain::TerrainManager>,
        terrain_tile: Option<&crate::gis::terrain::DecodedTerrainTile>,
        height_exaggeration: f32,
    ) {
        if let Some(gpu_tile) = self.gpu_tiles.get_mut(&coord) {
            let n_sub = if coord.z <= 3 { 32 } else if coord.z <= 6 { 24 } else { 16 };
            gpu_tile.mesh = GpuMesh::create_terrain_elevation_tile(
                &self.device,
                coord,
                origin,
                terrain,
                terrain_tile,
                height_exaggeration,
                n_sub,
            );
        }
    }

    /// Reloads / recalculates 3D terrain elevation meshes for all loaded GPU basemap tiles
    pub fn reload_all_terrain_meshes(
        &mut self,
        origin: &ProjectOrigin,
        terrain: &crate::gis::terrain::TerrainManager,
    ) {
        let coords: Vec<TileCoord> = self.gpu_tiles.keys().copied().collect();
        for coord in coords {
            let terrain_tile = if terrain.is_enabled {
                terrain.get_terrain_for_tile(coord)
            } else {
                None
            };
            let terrain_opt = if terrain.is_enabled { Some(terrain) } else { None };
            self.update_tile_terrain_mesh(coord, origin, terrain_opt, terrain_tile.as_ref(), terrain.height_exaggeration);
        }
    }

    pub fn add_tile(
        &mut self,
        tile: DecodedTile,
        origin: &ProjectOrigin,
        opacity: f32,
        terrain: Option<&crate::gis::terrain::TerrainManager>,
        terrain_tile: Option<&crate::gis::terrain::DecodedTerrainTile>,
        height_exaggeration: f32,
    ) -> Vec<TileCoord> {
        let mut evicted_coords = Vec::new();
        if self.gpu_tiles.contains_key(&tile.coord) {
            self.gpu_tiles.remove(&tile.coord);
        }

        let n_sub = if tile.coord.z <= 3 { 32 } else if tile.coord.z <= 6 { 24 } else { 16 };
        let mesh = GpuMesh::create_terrain_elevation_tile(
            &self.device,
            tile.coord,
            origin,
            terrain,
            terrain_tile,
            height_exaggeration,
            n_sub,
        );

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Basemap Tile Texture"),
            size: wgpu::Extent3d {
                width: tile.width,
                height: tile.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &tile.rgba_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tile.width),
                rows_per_image: Some(tile.height),
            },
            wgpu::Extent3d {
                width: tile.width,
                height: tile.height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let grid_mode = if !tile.has_texture { 1.0 } else { self.current_basemap_grid_mode };
        let tile_opacity = if !tile.has_texture { opacity } else { self.current_basemap_opacity.max(opacity) };
        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Basemap Tile Uniform Buffer"),
            contents: bytemuck::bytes_of(&BasemapUniformGpu {
                opacity: tile_opacity,
                brightness: 1.0,
                grid_mode,
                debug_border: if self.current_basemap_debug_borders { 1.0 } else { 0.0 },
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Basemap Tile Bind Group"),
            layout: &self.pipelines.basemap_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.basemap_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        #[cfg(target_arch = "wasm32")]
        const BASELINE_GPU_TILES: usize = 48;
        #[cfg(not(target_arch = "wasm32"))]
        const BASELINE_GPU_TILES: usize = 96;

        let max_gpu_tiles = BASELINE_GPU_TILES.max(self.active_tiles_to_draw.len() + 24);
        if self.gpu_tiles.len() >= max_gpu_tiles {
            let current_drawn: std::collections::HashSet<TileCoord> = self.active_tiles_to_draw.iter().copied().collect();
            let mut candidates: Vec<(TileCoord, u64)> = self.gpu_tiles
                .iter()
                .filter(|(coord, _)| coord.z > 2 && !current_drawn.contains(coord)) // never evict Z=0,1,2 global base world tiles or currently drawn tiles
                .map(|(coord, t)| (*coord, t.last_drawn_frame))
                .collect();

            // Sort by last_drawn_frame ascending (least recently drawn first)
            candidates.sort_by_key(|(_, last_frame)| *last_frame);

            let remove_count = (self.gpu_tiles.len() + 1).saturating_sub(max_gpu_tiles).min(candidates.len());
            for (coord, _) in candidates.into_iter().take(remove_count) {
                self.gpu_tiles.remove(&coord);
                evicted_coords.push(coord);
            }
        }

        let has_texture = tile.has_texture;
        self.gpu_tiles.insert(
            tile.coord,
            GpuTile {
                coord: tile.coord,
                has_texture,
                width: tile.width,
                height: tile.height,
                mesh,
                texture,
                view,
                bind_group,
                uniform_buffer,
                last_drawn_frame: self.frame_count,
            },
        );

        evicted_coords
    }

    /// Dynamically overwrite the RGBA texture of an existing GPU tile (e.g. for client-side vector layer re-rasterization)
    pub fn update_tile_texture_rgba(&self, coord: TileCoord, width: u32, height: u32, rgba_bytes: &[u8]) -> bool {
        if let Some(tile) = self.gpu_tiles.get(&coord) {
            // Guard against size mismatch: only write if dimensions exactly match the GPU texture
            if tile.width != width || tile.height != height {
                return false;
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tile.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba_bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            true
        } else {
            false
        }
    }

    pub fn update_basemap_opacity(&mut self, opacity: f32) {
        self.update_basemap_uniforms(opacity, 0.0, self.current_basemap_debug_borders);
    }

    pub fn update_basemap_uniforms(&mut self, opacity: f32, grid_mode: f32, debug_border: bool) {
        self.current_basemap_opacity = opacity;
        self.current_basemap_grid_mode = grid_mode;
        self.current_basemap_debug_borders = debug_border;
        for tile in self.gpu_tiles.values() {
            let actual_grid_mode = if !tile.has_texture {
                1.0
            } else {
                grid_mode
            };
            let uniform = BasemapUniformGpu {
                opacity: if !tile.has_texture { 1.0 } else { opacity },
                brightness: 1.0,
                grid_mode: actual_grid_mode,
                debug_border: if debug_border { 1.0 } else { 0.0 },
            };
            self.queue.write_buffer(&tile.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        }
    }

    pub fn update_basemap_debug_borders(&mut self, show: bool) {
        self.update_basemap_uniforms(
            self.current_basemap_opacity,
            self.current_basemap_grid_mode,
            show,
        );
    }

    /// Resolves tiles to draw using Zero-Flicker Atomic Tile Replacement:
    /// 1. Remove a tile only when its replacement is ready in GPU VRAM.
    /// 2. Removing old and adding new occurs in the exact same frame.
    /// 3. While newly calculated active target tiles are downloading over the network, resident
    ///    ancestors or descendants in GPU VRAM continue to draw to hold the ground, preventing flickering.
    /// 4. Keeps GPU memory tight with two-stage eviction (Idle eviction + Hard-ceiling LRU).
    pub fn prune_unneeded_tiles(&mut self, active_tiles: &[TileCoord]) -> Vec<TileCoord> {
        let mut tiles_to_draw = Vec::with_capacity(active_tiles.len());
        let mut drawn_set = std::collections::HashSet::with_capacity(active_tiles.len() * 2);

        // Stage 0: Atomic Tile Selection (Zero-Flicker Handoff)
        // 1. Direct active matches: if target tile is already resident in GPU VRAM, draw it.
        for &active in active_tiles {
            if self.gpu_tiles.contains_key(&active) {
                if drawn_set.insert(active) {
                    tiles_to_draw.push(active);
                }
            }
        }

        // 2. Pending active tiles: for target tiles still in-flight/downloading,
        // search for resident ancestors or descendants in GPU VRAM to hold the ground until
        // the replacement arrives.
        for &active in active_tiles {
            if drawn_set.contains(&active) {
                continue;
            }

            // A. Check for resident ancestors (closest ancestor in VRAM first)
            let mut found_ancestor = false;
            for delta in 1..=active.z.min(10) {
                let ancestor = active.ancestor(delta);
                if self.gpu_tiles.contains_key(&ancestor) {
                    if drawn_set.insert(ancestor) {
                        tiles_to_draw.push(ancestor);
                    }
                    found_ancestor = true;
                    break;
                }
            }

            // B. If no ancestor, check if zooming out left finer descendant tiles in VRAM
            if !found_ancestor {
                for &resident in self.gpu_tiles.keys() {
                    if resident.is_descendant_of(&active) {
                        if drawn_set.insert(resident) {
                            tiles_to_draw.push(resident);
                        }
                    }
                }
            }
        }

        self.active_tiles_to_draw = tiles_to_draw;

        // ── Budgets ──────────────────────────────────────────────────────────────
        // Hard ceiling: absolute VRAM cap configured via ResourceBudget.
        let baseline_gpu_tiles = self.budget.max_gpu_tiles_count();
        let max_gpu_tiles = baseline_gpu_tiles.max(self.active_tiles_to_draw.len() + 24);

        // Idle timeout: frames before an idle non-active tile is released.
        // ~10 s at 60 fps on Desktop (~5 s on WASM) — long enough to survive
        // zooming in/out or panning around without flushing tiles from VRAM,
        // while Stage 2 LRU hard-ceiling strictly enforces the max_gpu_tiles budget.
        #[cfg(target_arch = "wasm32")]
        const IDLE_EVICT_FRAMES: u64 = 300;
        #[cfg(not(target_arch = "wasm32"))]
        const IDLE_EVICT_FRAMES: u64 = 600;

        let mut evicted = Vec::new();
        let current_drawn: std::collections::HashSet<TileCoord> =
            self.active_tiles_to_draw.iter().copied().collect();

        // ── Stage 1: Idle eviction ────────────────────────────────────────────
        let idle_threshold = self.frame_count.saturating_sub(IDLE_EVICT_FRAMES);
        let idle_coords: Vec<TileCoord> = self.gpu_tiles
            .iter()
            .filter(|(coord, tile)| {
                coord.z > 2
                    && !current_drawn.contains(coord)
                    && tile.last_drawn_frame < idle_threshold
            })
            .map(|(coord, _)| *coord)
            .collect();

        for coord in idle_coords {
            self.gpu_tiles.remove(&coord);
            evicted.push(coord);
        }

        // ── Stage 2: Hard-ceiling LRU ─────────────────────────────────────────
        // Only reached when rapid panning loads many tiles before the idle
        // timeout expires.  Evicts the least-recently-drawn non-active tiles.
        if self.gpu_tiles.len() > max_gpu_tiles {
            let mut candidates: Vec<(TileCoord, u64)> = self.gpu_tiles
                .iter()
                .filter(|(coord, _)| coord.z > 2 && !current_drawn.contains(coord))
                .map(|(coord, t)| (*coord, t.last_drawn_frame))
                .collect();

            // Sort ascending: oldest drawn first → evict those first
            candidates.sort_by_key(|(_, last_frame)| *last_frame);

            let to_remove = (self.gpu_tiles.len() - max_gpu_tiles).min(candidates.len());
            for (coord, _) in candidates.into_iter().take(to_remove) {
                self.gpu_tiles.remove(&coord);
                evicted.push(coord);
            }
        }

        evicted
    }

    pub fn clear_basemap_tiles(&mut self) {
        self.gpu_tiles.clear();
        self.active_tiles_to_draw.clear();
    }

    pub fn add_i3s_tile(
        &mut self,
        decoded: crate::gis::i3s::DecodedI3SNode,
        tint: [f32; 3],
        opacity: f32,
        edge_enabled: bool,
        stroke_color: Option<[f32; 4]>,
        stroke_width: Option<f32>,
    ) {
        if self.i3s_gpu_tiles.contains_key(&decoded.node_id) {
            return;
        }

        let base_col = if decoded.image_rgba.is_some() {
            [1.0, 1.0, 1.0, decoded.base_color[3]]
        } else {
            [
                decoded.base_color[0] * tint[0],
                decoded.base_color[1] * tint[1],
                decoded.base_color[2] * tint[2],
                decoded.base_color[3],
            ]
        };

        let has_vertex_colors = !decoded.raw_mesh.colors.is_empty();
        let has_texture = decoded.image_rgba.is_some();

        let mut mesh_to_upload = decoded.raw_mesh.clone();
        if has_vertex_colors && opacity < 0.999 {
            for c in &mut mesh_to_upload.colors {
                c[3] = (c[3] * opacity).clamp(0.0, 1.0);
            }
        }

        if let Some(gpu_mesh) = GpuMesh::from_raw_mesh(
            &self.device,
            &mesh_to_upload,
            [base_col[0], base_col[1], base_col[2], base_col[3] * opacity],
        ) {
            // When mesh has photo textures, pass [0, 0, 0, 0] so it uses the texture without any colour fill.
            // For non-textured meshes, apply the layer's color fill / tint.
            let col_override = if has_texture {
                [0.0, 0.0, 0.0, 0.0]
            } else if has_vertex_colors && decoded.raw_mesh.colors.iter().any(|c| (c[0] - 1.0).abs() > 0.01 || (c[1] - 1.0).abs() > 0.01 || (c[2] - 1.0).abs() > 0.01) {
                [0.0, 0.0, 0.0, 0.0]
            } else {
                [tint[0] * base_col[0], tint[1] * base_col[1], tint[2] * base_col[2], opacity.clamp(0.05, 1.0) * base_col[3]]
            };
            let edge_val = if edge_enabled { 1.0 } else { 0.0 };
            let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("I3S Object Uniform Buffer"),
                contents: bytemuck::bytes_of(&ObjectUniformGpu {
                    model: Mat4::IDENTITY.to_cols_array_2d(),
                    color_override: col_override,
                    shadow_color: [0.10, 0.12, 0.18, edge_val],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("I3S Object Bind Group"),
                layout: &self.pipelines.object_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

            let texture_bind_group = if let Some(arc_img) = &decoded.image_rgba {
                let (img_w, img_h, rgba_bytes) = &**arc_img;
                let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("I3S Texture {}", decoded.node_id)),
                    size: wgpu::Extent3d {
                        width: *img_w,
                        height: *img_h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    rgba_bytes,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * *img_w),
                        rows_per_image: Some(*img_h),
                    },
                    wgpu::Extent3d {
                        width: *img_w,
                        height: *img_h,
                        depth_or_array_layers: 1,
                    },
                );
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                let meta_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("I3S Texture Meta {}", decoded.node_id)),
                    contents: bytemuck::bytes_of(&BasemapUniformGpu {
                        opacity: 1.0,
                        brightness: 1.0,
                        grid_mode: 1.0, // Texture active
                        debug_border: 0.0,
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("I3S Texture Bind Group {}", decoded.node_id)),
                    layout: &self.pipelines.basemap_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.basemap_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: meta_buffer.as_entire_binding(),
                        },
                    ],
                })
            } else {
                self.default_threedtile_texture_bind_group.clone()
            };

            // Extract sharp geometric crease edges (35 deg threshold) only if edge_enabled is true
            let edge_mesh = if edge_enabled {
                let crease_lines = decoded.raw_mesh.extract_crease_edges(35.0);
                let s_color = stroke_color.unwrap_or_else(|| [
                    self.edge_renderer.config.color[0],
                    self.edge_renderer.config.color[1],
                    self.edge_renderer.config.color[2],
                    (self.edge_renderer.config.color[3] * 0.70).clamp(0.25, 0.85),
                ]);
                let s_width = stroke_width.unwrap_or(self.edge_renderer.config.width.max(1.0));
                GpuMesh::create_screen_lines_batch(&self.device, &crease_lines, s_width, s_color)
            } else {
                None
            };

            self.i3s_gpu_tiles.insert(
                decoded.node_id,
                I3SGpuTile {
                    node_id: decoded.node_id,
                    mesh: gpu_mesh,
                    edge_mesh,
                    bind_group,
                    texture_bind_group,
                    uniform_buffer,
                    last_drawn_frame: self.frame_count,
                    has_vertex_colors,
                    has_texture,
                },
            );
        }
    }

    pub fn update_i3s_visuals(&self, tint: [f32; 3], opacity: f32, edge_enabled: bool) {
        let edge_val = if edge_enabled { 1.0 } else { 0.0 };
        for tile in self.i3s_gpu_tiles.values() {
            let col_override = if tile.has_texture {
                [0.0, 0.0, 0.0, 0.0]
            } else {
                [tint[0], tint[1], tint[2], opacity.clamp(0.05, 1.0)]
            };
            let data = ObjectUniformGpu {
                model: Mat4::IDENTITY.to_cols_array_2d(),
                color_override: col_override,
                shadow_color: [0.0, 0.0, 0.0, edge_val],
            };
            self.queue.write_buffer(&tile.uniform_buffer, 0, bytemuck::bytes_of(&data));
        }
    }

    pub fn prune_unneeded_i3s_tiles(&mut self, active_nodes: &[u32]) -> Vec<u32> {
        self.active_i3s_nodes_to_draw = active_nodes
            .iter()
            .copied()
            .filter(|id| self.i3s_gpu_tiles.contains_key(id))
            .collect();

        let active_set: std::collections::HashSet<u32> =
            self.active_i3s_nodes_to_draw.iter().copied().collect();

        const MAX_I3S_GPU_TILES: usize = 2048;
        let current_frame = self.frame_count;
        let mut evicted = Vec::new();

        let mut candidates: Vec<(u32, u64)> = self
            .i3s_gpu_tiles
            .iter()
            .filter(|(id, _)| !active_set.contains(id))
            .map(|(id, t)| (*id, t.last_drawn_frame))
            .collect();

        candidates.sort_by_key(|(_, last_frame)| *last_frame);

        for (node_id, last_frame) in candidates {
            if self.i3s_gpu_tiles.len() > MAX_I3S_GPU_TILES || current_frame.saturating_sub(last_frame) > 1800 {
                self.i3s_gpu_tiles.remove(&node_id);
                evicted.push(node_id);
            }
        }

        evicted
    }

    pub fn clear_i3s_tiles(&mut self) {
        self.i3s_gpu_tiles.clear();
        self.active_i3s_nodes_to_draw.clear();
    }

    pub fn add_threedtile(&mut self, decoded: crate::gis::threedtiles::DecodedThreeDTileMesh, tint: [f32; 3], opacity: f32, replace_texture: bool) {
        let tint_vec = [tint[0], tint[1], tint[2], opacity.clamp(0.05, 1.0)];
        if let Some(gpu_mesh) = GpuMesh::from_threedtile_mesh(
            &self.device,
            &decoded,
            tint_vec,
        ) {
            let bounding_box_mesh = if decoded.bounds_min != glam::Vec3::ZERO || decoded.bounds_max != glam::Vec3::ZERO {
                let size = decoded.bounds_max - decoded.bounds_min;
                let thickness = (size.length() * 0.005).clamp(0.5, 3.0);
                Some(GpuMesh::create_wireframe_box(
                    &self.device,
                    decoded.bounds_min,
                    decoded.bounds_max,
                    thickness,
                    [0.0, 0.95, 1.0, 1.0],
                ))
            } else {
                None
            };

            let mut submeshes = Vec::with_capacity(decoded.submeshes.len());
            let mut texture_cache: std::collections::HashMap<*const (u32, u32, Vec<u8>), wgpu::BindGroup> = std::collections::HashMap::new();

            for sm in &decoded.submeshes {
                // If submesh has photo texture, its photographic colors are already baked; avoid multiplying by dark default base_color
                let sm_base = if sm.image_rgba.is_some() { [1.0, 1.0, 1.0, sm.base_color[3]] } else { sm.base_color };
                let sm_tint = [
                    tint[0] * sm_base[0],
                    tint[1] * sm_base[1],
                    tint[2] * sm_base[2],
                    opacity.clamp(0.05, 1.0) * sm_base[3],
                ];

                let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("3D Tile Submesh Object Uniform Buffer"),
                    contents: bytemuck::bytes_of(&ObjectUniformGpu {
                        model: Mat4::IDENTITY.to_cols_array_2d(),
                        color_override: sm_tint,
                        shadow_color: [if replace_texture { 1.0 } else { 0.0 }, 1.0, 0.0, 1.0],
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("3D Tile Submesh Object Bind Group"),
                    layout: &self.pipelines.object_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    }],
                });

                let texture_bind_group = if let Some(arc_img) = &sm.image_rgba {
                    let ptr = std::sync::Arc::as_ptr(arc_img);
                    if let Some(cached_bg) = texture_cache.get(&ptr) {
                        cached_bg.clone()
                    } else {
                        let (img_w, img_h, rgba_bytes) = &**arc_img;
                        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                            label: Some(&format!("3D Tile Texture {}", decoded.id)),
                            size: wgpu::Extent3d {
                                width: *img_w,
                                height: *img_h,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        });
                        self.queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            rgba_bytes,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * *img_w),
                                rows_per_image: Some(*img_h),
                            },
                            wgpu::Extent3d {
                                width: *img_w,
                                height: *img_h,
                                depth_or_array_layers: 1,
                            },
                        );
                        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                        let meta_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("3D Tile Texture Meta {}", decoded.id)),
                            contents: bytemuck::bytes_of(&BasemapUniformGpu {
                                opacity: 1.0,
                                brightness: 1.0,
                                grid_mode: 1.0, // Texture active
                                debug_border: 0.0,
                            }),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        });
                        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("3D Tile Texture Bind Group {}", decoded.id)),
                            layout: &self.pipelines.basemap_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(&self.basemap_sampler),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: meta_buffer.as_entire_binding(),
                                },
                            ],
                        });
                        texture_cache.insert(ptr, bg.clone());
                        bg
                    }
                } else {
                    self.default_threedtile_texture_bind_group.clone()
                };

                submeshes.push(ThreeDTileSubMeshGpu {
                    index_offset: sm.index_offset,
                    index_count: sm.index_count,
                    bind_group,
                    uniform_buffer,
                    texture_bind_group,
                    base_color: sm.base_color,
                });
            }

            self.threedtiles_gpu_tiles.insert(
                decoded.id.clone(),
                ThreeDTileGpuMesh {
                    id: decoded.id,
                    mesh: gpu_mesh,
                    bounding_box_mesh,
                    submeshes,
                    last_drawn_frame: self.frame_count,
                    bounds_min: decoded.bounds_min,
                    bounds_max: decoded.bounds_max,
                },
            );
        }
    }

    pub fn update_threedtile_visuals(&self, tint: [f32; 3], opacity: f32, replace_texture: bool) {
        let op = opacity.clamp(0.05, 1.0);
        for tile in self.threedtiles_gpu_tiles.values() {
            for sm in &tile.submeshes {
                let sm_base = if sm.base_color[3] > 0.0 && sm.base_color[0] != 1.0 {
                    sm.base_color
                } else {
                    [1.0, 1.0, 1.0, sm.base_color[3]]
                };
                let col_override = [
                    tint[0] * sm_base[0],
                    tint[1] * sm_base[1],
                    tint[2] * sm_base[2],
                    op * sm_base[3],
                ];
                let data = ObjectUniformGpu {
                    model: Mat4::IDENTITY.to_cols_array_2d(),
                    color_override: col_override,
                    shadow_color: [if replace_texture { 1.0 } else { 0.0 }, 1.0, 0.0, 1.0],
                };
                self.queue.write_buffer(&sm.uniform_buffer, 0, bytemuck::bytes_of(&data));
            }
        }
    }

    pub fn prune_unneeded_threedtiles(&mut self, active_tiles: &[String]) -> Vec<String> {
        self.active_threedtiles_to_draw = active_tiles
            .iter()
            .cloned()
            .filter(|id| self.threedtiles_gpu_tiles.contains_key(id))
            .collect();

        let active_set: std::collections::HashSet<String> =
            self.active_threedtiles_to_draw.iter().cloned().collect();

        const MAX_3D_TILES: usize = 2048;
        let current_frame = self.frame_count;
        let mut evicted = Vec::new();

        let mut candidates: Vec<(String, u64)> = self
            .threedtiles_gpu_tiles
            .iter()
            .filter(|(id, _)| !active_set.contains(*id))
            .map(|(id, t)| (id.clone(), t.last_drawn_frame))
            .collect();

        candidates.sort_by_key(|(_, last_frame)| *last_frame);

        for (tile_id, last_frame) in candidates {
            if self.threedtiles_gpu_tiles.len() > MAX_3D_TILES || current_frame.saturating_sub(last_frame) > 120 {
                self.threedtiles_gpu_tiles.remove(&tile_id);
                evicted.push(tile_id);
            }
        }

        evicted
    }

    pub fn clear_threedtiles(&mut self) {
        self.threedtiles_gpu_tiles.clear();
        self.active_threedtiles_to_draw.clear();
    }

    pub fn load_mesh(&mut self, raw: &RawMeshData, color: [f32; 4]) -> usize {
        if let Some(gpu_mesh) = GpuMesh::from_raw_mesh(&self.device, raw, color) {
            self.meshes.push(gpu_mesh);
            self.meshes.len() - 1
        } else {
            0
        }
    }

    pub fn clear_meshes(&mut self) {
        self.meshes.clear();
        self.batched_chunks.clear();
        self.custom_shadow_casters.clear();
    }

    pub fn clear_batched_chunks(&mut self) {
        self.batched_chunks.clear();
    }

    pub fn load_batched_chunk(
        &mut self,
        meshes_with_colors: &[(&RawMeshData, [f32; 4], bool)],
        shadow_color: [f32; 4],
        cast_shadows: bool,
        aabb_min: glam::Vec3,
        aabb_max: glam::Vec3,
    ) {
        if let Some(gpu_mesh) = GpuMesh::from_batched_raw_meshes(&self.device, meshes_with_colors) {
            let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Batched Chunk Uniform Buffer"),
                contents: bytemuck::bytes_of(&ObjectUniformGpu {
                    model: Mat4::IDENTITY.to_cols_array_2d(),
                    color_override: [0.0, 0.0, 0.0, 0.0],
                    shadow_color,
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Batched Chunk Object Bind Group"),
                layout: &self.pipelines.object_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            self.batched_chunks.push(BatchedMeshChunk {
                mesh: gpu_mesh,
                aabb_min,
                aabb_max,
                is_visible: true,
                cast_shadows,
                shadow_color,
                bind_group,
            });
        }
    }

    pub fn set_selected_mesh(&mut self, mesh_data: Option<&RawMeshData>) {
        self.selected_mesh = mesh_data.and_then(|raw| {
            GpuMesh::from_raw_mesh(&self.device, raw, [1.0, 1.0, 1.0, 1.0])
        });
    }

    pub fn set_custom_shadow_color(&mut self, feature_id: &str, mesh_data: &RawMeshData, shadow_color: [f32; 4]) {
        // If default slate crate::gis::layer::DEFAULT_SHADOW_COLOR, remove custom override
        let is_default = (shadow_color[0] - 0.10).abs() < 0.01
            && (shadow_color[1] - 0.12).abs() < 0.01
            && (shadow_color[2] - 0.18).abs() < 0.01;

        if is_default {
            self.custom_shadow_casters.remove(feature_id);
            return;
        }

        if let Some(gpu_mesh) = GpuMesh::from_raw_mesh(&self.device, mesh_data, [1.0, 1.0, 1.0, 1.0]) {
            let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Custom Shadow Caster {}", feature_id)),
                contents: bytemuck::bytes_of(&ObjectUniformGpu {
                    model: Mat4::IDENTITY.to_cols_array_2d(),
                    color_override: [0.0, 0.0, 0.0, 0.0],
                    shadow_color,
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Custom Shadow Caster BG {}", feature_id)),
                layout: &self.pipelines.object_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            self.custom_shadow_casters.insert(
                feature_id.to_string(),
                CustomShadowCaster {
                    mesh: gpu_mesh,
                    shadow_color,
                    bind_group,
                },
            );
        }
    }

    pub fn clear_custom_shadow_colors(&mut self) {
        self.custom_shadow_casters.clear();
    }

    pub fn render_with_encoder(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        camera: &Camera,
        solar_pos: &SolarPosition,
        measurement: &MeasurementEngine,
        toolbox: &crate::spatial::toolbox::ToolboxEngine,
        basemap_visible: bool,
        _basemap_zoom: u32,
        sun_intensity: f32,
        ambient_intensity: f32,
        sunlight_enabled: bool,
    ) {
        self.frame_count = self.frame_count.wrapping_add(1);
        let aspect = (self.current_width as f32) / (self.current_height as f32);

        // 1. Update Camera Uniforms
        let view_proj = camera.view_proj_matrix(aspect);
        let inv_view_proj = view_proj.inverse();
        let eye_pos = camera.eye_position();
        let camera_uniform = CameraUniformGpu {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            eye_pos: [eye_pos.x, eye_pos.y, eye_pos.z, self.morph_progress],
            viewport: [
                self.current_width as f32,
                self.current_height as f32,
                1.0 / (self.current_width as f32).max(1.0),
                1.0 / (self.current_height as f32).max(1.0),
            ],
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        // 2. Update Shadow Map Light Matrix & Uniforms with Camera-Adaptive Focus
        let (is_daylight_val, sun_dir_vec, base_sun_color, base_ambient_color) = if sunlight_enabled {
            let is_daylight_val = if solar_pos.is_daylight { 1.0 } else { 0.0 };
            let base_sun_color = if solar_pos.elevation_deg > 10.0 {
                [1.0, 0.98, 0.92]
            } else if solar_pos.elevation_deg > 0.0 {
                [1.0, 0.65, 0.35]
            } else {
                [0.1, 0.15, 0.25]
            };
            let base_ambient_color = if solar_pos.is_daylight {
                [0.45, 0.55, 0.70]
            } else {
                [0.08, 0.10, 0.15]
            };
            (is_daylight_val, solar_pos.sun_direction, base_sun_color, base_ambient_color)
        } else {
            // Neutral 12:00 PM overhead daylight with NO cast shadows (mode 2.0)
            let overhead_sun = glam::Vec3::new(0.08, 0.99, 0.08).normalize();
            let base_sun_color = [1.0, 0.99, 0.96];
            let base_ambient_color = [0.65, 0.70, 0.80];
            (2.0, overhead_sun, base_sun_color, base_ambient_color)
        };

        let sin_pitch = camera.pitch.abs().sin().clamp(0.12, 1.0);
        let tilt_boost = (1.0 / sin_pitch).clamp(1.0, 4.0);
        let forward = (camera.target - camera.eye_position()).normalize_or_zero();
        let forward_planar = glam::Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let sun_dir_planar = glam::Vec3::new(sun_dir_vec.x, 0.0, sun_dir_vec.z).normalize_or_zero();
        let focus_radius = (camera.distance * (1.2 * tilt_boost) + 250.0).clamp(300.0, 12_000.0);
        let focus_center = camera.target
            + forward_planar * (focus_radius * 0.20)
            + sun_dir_planar * (focus_radius * 0.25);
        self.shadow_map.update_light_matrix(
            sun_dir_vec,
            focus_center,
            focus_radius,
        );

        let eff_sun_intensity = if sunlight_enabled { sun_intensity } else { 0.80 };
        let eff_ambient_intensity = if sunlight_enabled { ambient_intensity } else { 0.85 };

        let sun_color_val = [
            base_sun_color[0],
            base_sun_color[1],
            base_sun_color[2],
            eff_sun_intensity,
        ];

        let ambient_color_val = [
            base_ambient_color[0],
            base_ambient_color[1],
            base_ambient_color[2],
            eff_ambient_intensity,
        ];

        let world_texel_size = (self.shadow_map.extent * 2.0) / (crate::renderer::shadow_map::SHADOW_MAP_RES as f32);
        let uv_texel_size = 1.0 / (crate::renderer::shadow_map::SHADOW_MAP_RES as f32);
        let depth_range = self.shadow_map.far - self.shadow_map.near;
        let extent = self.shadow_map.extent;

        let light_uniform = LightUniformGpu {
            light_view_proj: self.shadow_map.light_view_proj.to_cols_array_2d(),
            sun_dir: [
                sun_dir_vec.x,
                sun_dir_vec.y,
                sun_dir_vec.z,
                is_daylight_val,
            ],
            sun_color: sun_color_val,
            ambient_color: ambient_color_val,
            shadow_params: [
                world_texel_size,
                uv_texel_size,
                depth_range,
                extent,
            ],
        };
        queue.write_buffer(&self.light_buffer, 0, bytemuck::bytes_of(&light_uniform));

        // Update Per-Node Uniform Buffers only if batched chunks are not active
        if self.batched_chunks.is_empty() {
            self.ensure_node_uniform_capacity(scene.nodes.len());
            for (i, node) in scene.nodes.iter().enumerate() {
                let (buf, _) = &self.node_object_bind_groups[i];
                let obj_gpu = ObjectUniformGpu {
                    model: node.transform.to_cols_array_2d(),
                    color_override: [0.0, 0.0, 0.0, 0.0],
                    shadow_color: node.shadow_color,
                };
                queue.write_buffer(buf, 0, bytemuck::bytes_of(&obj_gpu));
            }
        }

        // ----------------------------------------------------
        // Pass A: GPU Multi-Target Shadow Pass (Depth + Colored Caster)
        // ----------------------------------------------------
        {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Depth & Color Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.shadow_map.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.10, g: 0.12, b: 0.18, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            shadow_pass.set_pipeline(&self.pipelines.shadow_pipeline);
            shadow_pass.set_bind_group(0, &self.global_bind_group, &[]);

            let is_planar = self.projection_mode == crate::gis::crs::ProjectionMode::PlanarENU;
            if is_planar {
                if !self.batched_chunks.is_empty() {
                    for chunk in &self.batched_chunks {
                        if chunk.is_visible && chunk.cast_shadows {
                            shadow_pass.set_bind_group(1, &chunk.bind_group, &[]);
                            shadow_pass.set_vertex_buffer(0, chunk.mesh.vertex_buffer.slice(..));
                            shadow_pass.set_index_buffer(chunk.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            shadow_pass.draw_indexed(0..chunk.mesh.num_indices, 0, 0..1);
                        }
                    }
                } else {
                    for (i, node) in scene.nodes.iter().enumerate() {
                        if node.is_visible && node.mesh_index < self.meshes.len() {
                            let mesh = &self.meshes[node.mesh_index];
                            shadow_pass.set_bind_group(1, &self.node_object_bind_groups[i].1, &[]);
                            shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                            shadow_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            shadow_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                        }
                    }
                }

                // Draw active I3S 3D SceneLayer meshes into shadow depth buffer
                if self.i3s_cast_shadows {
                    for node_id in &self.active_i3s_nodes_to_draw {
                        if let Some(tile) = self.i3s_gpu_tiles.get(node_id) {
                            shadow_pass.set_bind_group(1, &tile.bind_group, &[]);
                            shadow_pass.set_vertex_buffer(0, tile.mesh.vertex_buffer.slice(..));
                            shadow_pass.set_index_buffer(tile.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            shadow_pass.draw_indexed(0..tile.mesh.num_indices, 0, 0..1);
                        }
                    }
                }

                // Draw active OGC 3D Tiles meshes into shadow depth buffer
                for tile_id in &self.active_threedtiles_to_draw {
                    if let Some(tile) = self.threedtiles_gpu_tiles.get(tile_id) {
                        shadow_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                        shadow_pass.set_vertex_buffer(0, tile.mesh.vertex_buffer.slice(..));
                        shadow_pass.set_index_buffer(tile.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        shadow_pass.draw_indexed(0..tile.mesh.num_indices, 0, 0..1);
                    }
                }

                // Draw individual custom colored shadow casters LAST (overwrites depth and colored shadow map)
                for caster in self.custom_shadow_casters.values() {
                    shadow_pass.set_bind_group(1, &caster.bind_group, &[]);
                    shadow_pass.set_vertex_buffer(0, caster.mesh.vertex_buffer.slice(..));
                    shadow_pass.set_index_buffer(caster.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    shadow_pass.draw_indexed(0..caster.mesh.num_indices, 0, 0..1);
                }
            }
        }

        // ----------------------------------------------------
        // Pass B: Forward Pass (Offscreen Target)
        // ----------------------------------------------------
        {
            let sky_clear = if solar_pos.is_daylight {
                wgpu::Color { r: 0.10, g: 0.12, b: 0.16, a: 1.0 }
            } else {
                wgpu::Color { r: 0.03, g: 0.04, b: 0.06, a: 1.0 }
            };

            let mut main_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Forward Pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.target_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(sky_clear),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.edge_renderer.normal_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.5, g: 0.5, b: 0.5, a: 0.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            main_pass.set_viewport(
                0.0,
                0.0,
                self.current_width as f32,
                self.current_height as f32,
                0.0,
                1.0,
            );
            main_pass.set_scissor_rect(0, 0, self.current_width, self.current_height);

            // 0. Draw Dynamic Atmospheric Sky Dome & Solar Disc (Fullscreen Background Pass)
            main_pass.set_pipeline(&self.pipelines.sky_pipeline);
            main_pass.set_bind_group(0, &self.global_bind_group, &[]);
            main_pass.draw(0..3, 0..1);

            // 1. Draw Default Ground Plane (Planar) or Solid Earth Sphere (Globe) with CAD Grid
            // In Globe mode, always draw the Earth sphere backdrop.
            // In Planar mode, always draw the base CAD ground plane as a foundational backdrop
            // underneath basemap and terrain tiles (at Y = 0.0m). This guarantees that missing,
            // pending, or off-extent regions seamlessly show the clean CAD grid instead of exposing
            // the sky dome haze void!
            // When basemap or terrain tiles are present, we use ground_backdrop_pipeline (depth_write_enabled: false)
            // so the CAD grid backdrop never occludes terrain in rivers, bays, or valleys with negative elevation.
            let should_draw_base_mesh = true;

            if should_draw_base_mesh {
                let use_backdrop_pass = basemap_visible && !self.active_tiles_to_draw.is_empty();
                let ground_pipeline = if use_backdrop_pass {
                    &self.pipelines.ground_backdrop_pipeline
                } else {
                    &self.pipelines.ground_pipeline
                };
                main_pass.set_pipeline(ground_pipeline);
                main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                main_pass.set_bind_group(2, &self.shadow_bind_group, &[]);

                let base_bg_mesh = match self.projection_mode {
                    crate::gis::crs::ProjectionMode::PlanarENU => &self.ground_mesh,
                    crate::gis::crs::ProjectionMode::GlobeECEF => &self.earth_sphere,
                };
                main_pass.set_vertex_buffer(0, base_bg_mesh.vertex_buffer.slice(..));
                main_pass.set_index_buffer(base_bg_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                main_pass.draw_indexed(0..base_bg_mesh.num_indices, 0, 0..1);
            }

            // 2. Draw Basemap / 3D Terrain Tiles (Textured Satellite/Street Imagery or 3D CAD Grid Terrain)
            if basemap_visible && !self.active_tiles_to_draw.is_empty() {
                main_pass.set_pipeline(&self.pipelines.basemap_pipeline);
                main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                main_pass.set_bind_group(2, &self.shadow_bind_group, &[]);

                let current_frame = self.frame_count;
                let mut draw_coords = self.active_tiles_to_draw.clone();
                // Draw fallback lower zoom ancestors first, exact leaf tiles on top
                draw_coords.sort_by_key(|c| c.z);

                for coord in draw_coords {
                    if let Some(tile) = self.gpu_tiles.get_mut(&coord) {
                        tile.last_drawn_frame = current_frame;
                        main_pass.set_bind_group(3, &tile.bind_group, &[]);
                        main_pass.set_vertex_buffer(0, tile.mesh.vertex_buffer.slice(..));
                        main_pass.set_index_buffer(tile.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        main_pass.draw_indexed(0..tile.mesh.num_indices, 0, 0..1);
                    }
                }
            }

            let is_planar = self.projection_mode == crate::gis::crs::ProjectionMode::PlanarENU;

            if is_planar {
                // 2. Draw Scene Nodes / Batched Chunks
                main_pass.set_pipeline(&self.pipelines.main_pipeline);
                main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                main_pass.set_bind_group(2, &self.shadow_bind_group, &[]);

                if !self.batched_chunks.is_empty() {
                    for chunk in &self.batched_chunks {
                        if chunk.is_visible {
                            main_pass.set_bind_group(1, &chunk.bind_group, &[]);
                            main_pass.set_vertex_buffer(0, chunk.mesh.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(chunk.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..chunk.mesh.num_indices, 0, 0..1);
                        }
                    }
                } else {
                    for (i, node) in scene.nodes.iter().enumerate() {
                        if !node.is_visible || node.mesh_index >= self.meshes.len() {
                            continue;
                        }

                        let mesh = &self.meshes[node.mesh_index];
                        main_pass.set_bind_group(1, &self.node_object_bind_groups[i].1, &[]);
                        main_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        main_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        main_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                    }
                }

                // 3. Draw active I3S 3D SceneLayer buildings with PBR sunlight and ambient shading
                if !self.active_i3s_nodes_to_draw.is_empty() {
                    let current_frame = self.frame_count;
                    let mut current_pipeline_is_textured: Option<bool> = None;

                    for node_id in &self.active_i3s_nodes_to_draw {
                        if let Some(tile) = self.i3s_gpu_tiles.get_mut(node_id) {
                            tile.last_drawn_frame = current_frame;

                            if tile.has_texture {
                                if current_pipeline_is_textured != Some(true) {
                                    main_pass.set_pipeline(&self.pipelines.threedtile_pipeline);
                                    main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                                    main_pass.set_bind_group(2, &self.shadow_bind_group, &[]);
                                    current_pipeline_is_textured = Some(true);
                                }
                                main_pass.set_bind_group(1, &tile.bind_group, &[]);
                                main_pass.set_bind_group(3, &tile.texture_bind_group, &[]);
                            } else {
                                if current_pipeline_is_textured != Some(false) {
                                    main_pass.set_pipeline(&self.pipelines.main_pipeline);
                                    main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                                    main_pass.set_bind_group(2, &self.shadow_bind_group, &[]);
                                    current_pipeline_is_textured = Some(false);
                                }
                                main_pass.set_bind_group(1, &tile.bind_group, &[]);
                            }

                            main_pass.set_vertex_buffer(0, tile.mesh.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(tile.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..tile.mesh.num_indices, 0, 0..1);
                        }
                    }

                    // Draw I3S geometric feature crease edges (anti-aliased screen-space quads with depth test & polygon offset)
                    let mut lines_pipeline_bound = false;
                    for node_id in &self.active_i3s_nodes_to_draw {
                        if let Some(tile) = self.i3s_gpu_tiles.get(node_id) {
                            if let Some(edge_mesh) = &tile.edge_mesh {
                                if !lines_pipeline_bound {
                                    main_pass.set_pipeline(&self.pipelines.screen_line_pipeline);
                                    main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                                    main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                                    lines_pipeline_bound = true;
                                }
                                main_pass.set_vertex_buffer(0, edge_mesh.vertex_buffer.slice(..));
                                main_pass.set_index_buffer(edge_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                                main_pass.draw_indexed(0..edge_mesh.num_indices, 0, 0..1);
                            }
                        }
                    }
                }

                // 4. Draw active OGC 3D Tiles buildings with PBR sunlight, photo textures, and ambient shading
                if !self.active_threedtiles_to_draw.is_empty() {
                    main_pass.set_pipeline(&self.pipelines.threedtile_pipeline);
                    main_pass.set_bind_group(0, &self.global_bind_group, &[]);
                    main_pass.set_bind_group(2, &self.shadow_bind_group, &[]);

                    let current_frame = self.frame_count;
                    for tile_id in &self.active_threedtiles_to_draw {
                        if let Some(tile) = self.threedtiles_gpu_tiles.get_mut(tile_id) {
                            tile.last_drawn_frame = current_frame;
                            main_pass.set_vertex_buffer(0, tile.mesh.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(tile.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                            for sm in &tile.submeshes {
                                main_pass.set_bind_group(1, &sm.bind_group, &[]);
                                main_pass.set_bind_group(3, &sm.texture_bind_group, &[]);
                                main_pass.draw_indexed(sm.index_offset..(sm.index_offset + sm.index_count), 0, 0..1);
                            }

                            // Draw tile bounding box wireframe if enabled
                            if self.show_3dtiles_bounding_boxes {
                                if let Some(bbox) = &tile.bounding_box_mesh {
                                    main_pass.set_pipeline(&self.pipelines.unlit_pipeline);
                                    main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                                    main_pass.set_vertex_buffer(0, bbox.vertex_buffer.slice(..));
                                    main_pass.set_index_buffer(bbox.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                                    main_pass.draw_indexed(0..bbox.num_indices, 0, 0..1);
                                    main_pass.set_pipeline(&self.pipelines.threedtile_pipeline);
                                }
                            }
                        }
                    }
                }

                // 5. Draw Measurement overlays (Screen-space 3D lines and unlit markers)
                if self.measurement_visible && !measurement.points.is_empty() {
                    main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);

                    for (idx, pt) in measurement.points.iter().enumerate() {
                        let marker_color = if idx == 0 {
                            [0.2, 0.9, 0.3, 1.0]
                        } else {
                            [1.0, 0.75, 0.1, 1.0]
                        };
                        let marker_mesh = GpuMesh::create_marker_box(&self.device, *pt, 1.2, marker_color);
                        main_pass.set_pipeline(&self.pipelines.unlit_pipeline);
                        main_pass.set_vertex_buffer(0, marker_mesh.vertex_buffer.slice(..));
                        main_pass.set_index_buffer(marker_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        main_pass.draw_indexed(0..marker_mesh.num_indices, 0, 0..1);
                    }

                    if measurement.points.len() >= 2 {
                        main_pass.set_pipeline(&self.pipelines.screen_line_pipeline);
                        for i in 0..measurement.points.len() - 1 {
                            let p1 = measurement.points[i];
                            let p2 = measurement.points[i + 1];
                            let line_mesh = GpuMesh::create_screen_line(
                                &self.device,
                                p1,
                                p2,
                                3.0,
                                [1.0, 0.8, 0.1, 1.0],
                            );
                            main_pass.set_vertex_buffer(0, line_mesh.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(line_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..line_mesh.num_indices, 0, 0..1);
                        }
                    }
                }

                // 6. Draw Line of Sight (LOS) 3D Beams & X-Ray Occluded Sightlines
                // Screen-Space 3D Pixel Lines: size is 3px across all zoom levels
                // Control lines and markers use unlit shaders: never receive shadow, never cast shadow
                if self.los_visible {
                    for analysis in &toolbox.los.analyses {
                        let obs_eye = analysis.observer_eye_pt();
                        let tgt_focal = analysis.target_focal_pt();
                        let normal_green = [0.10, 0.85, 0.30, 1.0];
                        let xray_green = [0.40, 0.95, 0.50, 0.45];
                        let normal_red = [1.0, 0.20, 0.20, 1.0];
                        let xray_red = [1.0, 0.60, 0.65, 0.45];
                        let width_px = 3.0;

                        if analysis.is_visible {
                            let vis_line = GpuMesh::create_screen_line(
                                &self.device,
                                obs_eye,
                                tgt_focal,
                                width_px,
                                normal_green,
                            );
                            main_pass.set_pipeline(&self.pipelines.screen_line_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, vis_line.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(vis_line.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..vis_line.num_indices, 0, 0..1);

                            let xray_line = GpuMesh::create_screen_line(
                                &self.device,
                                obs_eye,
                                tgt_focal,
                                width_px,
                                xray_green,
                            );
                            main_pass.set_pipeline(&self.pipelines.screen_line_xray_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, xray_line.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(xray_line.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..xray_line.num_indices, 0, 0..1);
                        } else if let Some(hit_pt) = analysis.obstruction_pt {
                            let vis_green = GpuMesh::create_screen_line(
                                &self.device,
                                obs_eye,
                                hit_pt,
                                width_px,
                                normal_green,
                            );
                            main_pass.set_pipeline(&self.pipelines.screen_line_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, vis_green.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(vis_green.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..vis_green.num_indices, 0, 0..1);

                            let xray_green_line = GpuMesh::create_screen_line(
                                &self.device,
                                obs_eye,
                                hit_pt,
                                width_px,
                                xray_green,
                            );
                            main_pass.set_pipeline(&self.pipelines.screen_line_xray_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, xray_green_line.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(xray_green_line.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..xray_green_line.num_indices, 0, 0..1);

                            let hit_marker = GpuMesh::create_marker_box(
                                &self.device,
                                hit_pt,
                                0.7,
                                normal_red,
                            );
                            main_pass.set_pipeline(&self.pipelines.unlit_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, hit_marker.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(hit_marker.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..hit_marker.num_indices, 0, 0..1);

                            let vis_red = GpuMesh::create_screen_line(
                                &self.device,
                                hit_pt,
                                tgt_focal,
                                width_px,
                                normal_red,
                            );
                            main_pass.set_pipeline(&self.pipelines.screen_line_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, vis_red.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(vis_red.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..vis_red.num_indices, 0, 0..1);

                            let xray_red_line = GpuMesh::create_screen_line(
                                &self.device,
                                hit_pt,
                                tgt_focal,
                                width_px,
                                xray_red,
                            );
                            main_pass.set_pipeline(&self.pipelines.screen_line_xray_pipeline);
                            main_pass.set_bind_group(1, &self.default_object_bind_group, &[]);
                            main_pass.set_vertex_buffer(0, xray_red_line.vertex_buffer.slice(..));
                            main_pass.set_index_buffer(xray_red_line.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            main_pass.draw_indexed(0..xray_red_line.num_indices, 0, 0..1);
                        }
                    }
                }
            }
        }

        // ----------------------------------------------------
        // Pass C: Architectural Edge Outlines (Esri SolidEdges3D)
        // ----------------------------------------------------
        let is_planar = self.projection_mode == crate::gis::crs::ProjectionMode::PlanarENU;
        #[cfg(not(target_arch = "wasm32"))]
        if is_planar {
            self.edge_renderer.render_edges(
                _device,
                queue,
                encoder,
                &self.target_view,
                &self.target_texture,
                &self.depth_view,
                &self.target_view,
                &self.target_texture,
                self.current_width,
                self.current_height,
            );
        }

        // ----------------------------------------------------
        // Pass D: JFA Selection Outline & Glow
        // ----------------------------------------------------
        if is_planar {
            self.jfa_highlighter.render_highlight(
                _device,
                queue,
                encoder,
                &self.global_bind_group,
                &self.default_object_bind_group,
                &self.target_view,
                self.selected_mesh.as_ref(),
                self.current_width,
                self.current_height,
            );
        }

        // Copy rendered output to sampleable texture (transitions D3D12/Vulkan states cleanly)
        #[cfg(not(target_arch = "wasm32"))]
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.sample_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.current_width,
                height: self.current_height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        solar_pos: &SolarPosition,
        measurement: &MeasurementEngine,
        toolbox: &crate::spatial::toolbox::ToolboxEngine,
        basemap_visible: bool,
        basemap_zoom: u32,
        sun_intensity: f32,
        ambient_intensity: f32,
        sunlight_enabled: bool,
    ) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        self.render_with_encoder(
            &self.device.clone(),
            &self.queue.clone(),
            &mut encoder,
            scene,
            camera,
            solar_pos,
            measurement,
            toolbox,
            basemap_visible,
            basemap_zoom,
            sun_intensity,
            ambient_intensity,
            sunlight_enabled,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        if self.frame_count % 120 == 1 {
            log::trace!(
                "[Renderer] Frame {}: gpu_tiles={}, to_draw={}, viewport={}x{}",
                self.frame_count, self.gpu_tiles.len(), self.active_tiles_to_draw.len(), self.current_width, self.current_height
            );
        }
    }
}
