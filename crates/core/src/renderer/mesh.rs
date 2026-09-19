use crate::gis::basemap::TileCoord;
use crate::gis::extrusion::RawMeshData;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl GpuMesh {
    pub fn from_raw_mesh(device: &wgpu::Device, raw: &RawMeshData, color: [f32; 4]) -> Option<Self> {
        if raw.positions.is_empty() || raw.indices.is_empty() {
            return None;
        }

        let mut vertices = Vec::with_capacity(raw.positions.len());
        for i in 0..raw.positions.len() {
            let pos = raw.positions[i];
            let norm = if i < raw.normals.len() { raw.normals[i] } else { [0.0, 1.0, 0.0] };
            let uv = if i < raw.uvs.len() { raw.uvs[i] } else { [0.0, 0.0] };
            let vert_color = if i < raw.colors.len() { raw.colors[i] } else { color };

            vertices.push(Vertex {
                position: pos,
                normal: norm,
                uv,
                color: vert_color,
            });
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&raw.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            num_indices: raw.indices.len() as u32,
        })
    }

    pub fn from_threedtile_mesh(
        device: &wgpu::Device,
        mesh: &crate::gis::threedtiles::DecodedThreeDTileMesh,
        tint: [f32; 4],
    ) -> Option<Self> {
        if mesh.positions.is_empty() || mesh.indices.is_empty() {
            return None;
        }

        let mut vertices = Vec::with_capacity(mesh.positions.len());
        for i in 0..mesh.positions.len() {
            let p = mesh.positions[i];
            let n = mesh.normals.get(i).copied().unwrap_or(Vec3::Y);
            let uv = mesh.uvs.get(i).copied().unwrap_or(Vec2::ZERO);
            let ecef = mesh.ecef_positions.get(i).copied().unwrap_or(p);

            // Storing ECEF position in vertex.color.xyz allows real-time Planar <-> Globe morphing in vertex shader
            vertices.push(Vertex {
                position: [p.x, p.y, p.z],
                normal: [n.x, n.y, n.z],
                uv: [uv.x, uv.y],
                color: [ecef.x, ecef.y, ecef.z, tint[3]],
            });
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("3D Tile Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("3D Tile Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            num_indices: mesh.indices.len() as u32,
        })
    }

    /// Combine thousands of raw meshes into a single unified GPU vertex and index buffer
    pub fn from_batched_raw_meshes(
        device: &wgpu::Device,
        meshes_with_colors: &[(&RawMeshData, [f32; 4], bool)],
    ) -> Option<Self> {
        let total_verts: usize = meshes_with_colors.iter().map(|(m, _, _)| m.positions.len()).sum();
        let total_indices: usize = meshes_with_colors.iter().map(|(m, _, _)| m.indices.len()).sum();

        if total_verts == 0 || total_indices == 0 {
            return None;
        }

        let mut vertices = Vec::with_capacity(total_verts);
        let mut indices = Vec::with_capacity(total_indices);

        for (mesh, color, edge_enabled) in meshes_with_colors {
            let base_vertex = vertices.len() as u32;
            let edge_val = if *edge_enabled { 1.0 } else { 0.0 };
            for i in 0..mesh.positions.len() {
                let pos = mesh.positions[i];
                let norm = if i < mesh.normals.len() { mesh.normals[i] } else { [0.0, 1.0, 0.0] };
                let uv = [0.0, edge_val];
                let vert_color = if i < mesh.colors.len() { mesh.colors[i] } else { *color };
                vertices.push(Vertex {
                    position: pos,
                    normal: norm,
                    uv,
                    color: vert_color,
                });
            }
            for idx in &mesh.indices {
                indices.push(base_vertex + *idx);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batched Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batched Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        })
    }

    /// Ground grid mesh with tessellated subdivisions to guarantee high floating-point depth buffer precision
    pub fn create_ground_plane(device: &wgpu::Device, size: f32, grid_step: f32) -> Self {
        let half = size * 0.5;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let n_sub = 64; // 64x64 grid of small triangles eliminates depth interpolation precision loss
        let ground_color = [0.13, 0.15, 0.18, 1.0];

        for j in 0..=n_sub {
            let v = j as f32 / n_sub as f32;
            let z = -half + v * size;
            let uv_y = v * (size / grid_step);

            for i in 0..=n_sub {
                let u = i as f32 / n_sub as f32;
                let x = -half + u * size;
                let uv_x = u * (size / grid_step);

                vertices.push(Vertex {
                    position: [x, 0.0, z],
                    normal: [0.0, 1.0, 0.0],
                    uv: [uv_x, uv_y],
                    color: ground_color,
                });
            }
        }

        let stride = (n_sub + 1) as u32;
        for j in 0..n_sub as u32 {
            for i in 0..n_sub as u32 {
                let v00 = j * stride + i;
                let v10 = j * stride + (i + 1);
                let v01 = (j + 1) * stride + i;
                let v11 = (j + 1) * stride + (i + 1);

                indices.push(v00);
                indices.push(v11);
                indices.push(v10);

                indices.push(v00);
                indices.push(v01);
                indices.push(v11);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Creates a screen-space constant pixel-width 3D line ribbon segment
    pub fn create_screen_line(
        device: &wgpu::Device,
        p1: Vec3,
        p2: Vec3,
        width_px: f32,
        color: [f32; 4],
    ) -> Self {
        let p2 = if (p2 - p1).length_squared() < 1e-6 {
            p1 + Vec3::new(0.001, 0.0, 0.0)
        } else {
            p2
        };

        let vertices = [
            Vertex { position: [p1.x, p1.y, p1.z], normal: [p2.x, p2.y, p2.z], uv: [-1.0, width_px], color },
            Vertex { position: [p1.x, p1.y, p1.z], normal: [p2.x, p2.y, p2.z], uv: [ 1.0, width_px], color },
            Vertex { position: [p2.x, p2.y, p2.z], normal: [p1.x, p1.y, p1.z], uv: [ 1.0, width_px], color },
            Vertex { position: [p2.x, p2.y, p2.z], normal: [p1.x, p1.y, p1.z], uv: [-1.0, width_px], color },
        ];
        let indices: [u32; 6] = [0, 1, 2, 1, 3, 2];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen Line Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen Line Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: 6,
        }
    }

    /// Creates a batch of screen-space constant pixel-width 3D line ribbon segments
    pub fn create_screen_lines_batch(
        device: &wgpu::Device,
        lines: &[(Vec3, Vec3)],
        width_px: f32,
        color: [f32; 4],
    ) -> Option<Self> {
        if lines.is_empty() {
            return None;
        }

        let mut vertices = Vec::with_capacity(lines.len() * 4);
        let mut indices = Vec::with_capacity(lines.len() * 6);

        for &(p1, p2) in lines {
            let p2 = if (p2 - p1).length_squared() < 1e-6 {
                p1 + Vec3::new(0.001, 0.0, 0.0)
            } else {
                p2
            };

            let base = vertices.len() as u32;
            vertices.push(Vertex { position: [p1.x, p1.y, p1.z], normal: [p2.x, p2.y, p2.z], uv: [-1.0, width_px], color });
            vertices.push(Vertex { position: [p1.x, p1.y, p1.z], normal: [p2.x, p2.y, p2.z], uv: [ 1.0, width_px], color });
            vertices.push(Vertex { position: [p2.x, p2.y, p2.z], normal: [p1.x, p1.y, p1.z], uv: [ 1.0, width_px], color });
            vertices.push(Vertex { position: [p2.x, p2.y, p2.z], normal: [p1.x, p1.y, p1.z], uv: [-1.0, width_px], color });

            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen Lines Batch Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen Lines Batch Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        })
    }

    /// Measurement line segment (represented as a 3D thin prism)
    pub fn create_line_segment(device: &wgpu::Device, p1: Vec3, p2: Vec3, thickness: f32, color: [f32; 4]) -> Self {
        let dir = p2 - p1;
        let len = dir.length();
        if len < 1e-4 {
            return Self::create_marker_box(device, p1, thickness, color);
        }

        let dir_norm = dir / len;
        let up = if dir_norm.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
        let right = dir_norm.cross(up).normalize() * (thickness * 0.5);
        let perp_up = right.cross(dir_norm).normalize() * (thickness * 0.5);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // 8 box vertices around line
        let v = [
            p1 - right - perp_up,
            p1 + right - perp_up,
            p1 + right + perp_up,
            p1 - right + perp_up,
            p2 - right - perp_up,
            p2 + right - perp_up,
            p2 + right + perp_up,
            p2 - right + perp_up,
        ];

        for pos in &v {
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color,
            });
        }

        // 12 triangles (6 box faces)
        let box_indices = [
            0, 1, 2, 0, 2, 3, // start cap
            4, 6, 5, 4, 7, 6, // end cap
            0, 4, 5, 0, 5, 1, // bottom
            2, 6, 7, 2, 7, 3, // top
            0, 3, 7, 0, 7, 4, // left
            1, 5, 6, 1, 6, 2, // right
        ];
        indices.extend_from_slice(&box_indices);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Line Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Marker box (e.g. for picked point or measurement vertex)
    pub fn create_marker_box(device: &wgpu::Device, center: Vec3, half_size: f32, color: [f32; 4]) -> Self {
        let mut vertices = Vec::new();
        let s = half_size;

        let corners = [
            center + Vec3::new(-s, -s, -s),
            center + Vec3::new(s, -s, -s),
            center + Vec3::new(s, s, -s),
            center + Vec3::new(-s, s, -s),
            center + Vec3::new(-s, -s, s),
            center + Vec3::new(s, -s, s),
            center + Vec3::new(s, s, s),
            center + Vec3::new(-s, s, s),
        ];

        for c in &corners {
            vertices.push(Vertex {
                position: [c.x, c.y, c.z],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color,
            });
        }

        let indices: [u32; 36] = [
            0, 2, 1, 0, 3, 2, // front
            4, 5, 6, 4, 6, 7, // back
            0, 1, 5, 0, 5, 4, // bottom
            2, 3, 7, 2, 7, 6, // top
            0, 4, 7, 0, 7, 3, // left
            1, 2, 6, 1, 6, 5, // right
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Marker Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Marker Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: 36,
        }
    }

    /// Creates a 12-edge 3D wireframe bounding box with tubular/box edges
    pub fn create_wireframe_box(
        device: &wgpu::Device,
        min: Vec3,
        max: Vec3,
        thickness: f32,
        color: [f32; 4],
    ) -> Self {
        let c = [
            Vec3::new(min.x, min.y, min.z), // 0
            Vec3::new(max.x, min.y, min.z), // 1
            Vec3::new(max.x, min.y, max.z), // 2
            Vec3::new(min.x, min.y, max.z), // 3
            Vec3::new(min.x, max.y, min.z), // 4
            Vec3::new(max.x, max.y, min.z), // 5
            Vec3::new(max.x, max.y, max.z), // 6
            Vec3::new(min.x, max.y, max.z), // 7
        ];

        let edges = [
            (0, 1), (1, 2), (2, 3), (3, 0), // Bottom
            (4, 5), (5, 6), (6, 7), (7, 4), // Top
            (0, 4), (1, 5), (2, 6), (3, 7), // Vertical
        ];

        let mut total_vertices = Vec::with_capacity(12 * 8);
        let mut total_indices = Vec::with_capacity(12 * 36);

        for (p1_idx, p2_idx) in edges {
            let p1 = c[p1_idx];
            let p2 = c[p2_idx];
            let dir = p2 - p1;
            let len = dir.length();
            if len < 1e-4 {
                continue;
            }

            let dir_norm = dir / len;
            let up = if dir_norm.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
            let right = dir_norm.cross(up).normalize() * (thickness * 0.5);
            let perp_up = right.cross(dir_norm).normalize() * (thickness * 0.5);

            let v_offset = total_vertices.len() as u32;
            let v = [
                p1 - right - perp_up,
                p1 + right - perp_up,
                p1 + right + perp_up,
                p1 - right + perp_up,
                p2 - right - perp_up,
                p2 + right - perp_up,
                p2 + right + perp_up,
                p2 - right + perp_up,
            ];

            for pos in &v {
                total_vertices.push(Vertex {
                    position: [pos.x, pos.y, pos.z],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    color,
                });
            }

            let box_indices = [
                0, 1, 2, 0, 2, 3, // start cap
                4, 6, 5, 4, 7, 6, // end cap
                0, 4, 5, 0, 5, 1, // bottom
                2, 6, 7, 2, 7, 3, // top
                0, 3, 7, 0, 7, 4, // left
                1, 5, 6, 1, 6, 2, // right
            ];
            for idx in box_indices {
                total_indices.push(v_offset + idx);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bounding Box Wireframe Vertex Buffer"),
            contents: bytemuck::cast_slice(&total_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bounding Box Wireframe Index Buffer"),
            contents: bytemuck::cast_slice(&total_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: total_indices.len() as u32,
        }
    }

    /// Single textured quad (e.g. for a Slippy Map basemap tile in Planar ENU Mode)
    /// `corners`: [NW, NE, SE, SW] in Local ENU coordinates
    pub fn create_textured_quad(device: &wgpu::Device, corners: [Vec3; 4]) -> Self {
        let normal = [0.0, 1.0, 0.0];
        let color = [1.0, 1.0, 1.0, 1.0];

        let vertices = [
            Vertex { position: [corners[0].x, corners[0].y, corners[0].z], normal, uv: [0.0, 0.0], color }, // NW
            Vertex { position: [corners[1].x, corners[1].y, corners[1].z], normal, uv: [1.0, 0.0], color }, // NE
            Vertex { position: [corners[2].x, corners[2].y, corners[2].z], normal, uv: [1.0, 1.0], color }, // SE
            Vertex { position: [corners[3].x, corners[3].y, corners[3].z], normal, uv: [0.0, 1.0], color }, // SW
        ];

        let indices: [u32; 6] = [0, 3, 2, 0, 2, 1];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tile Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tile Quad Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: 6,
        }
    }

    /// Curved 3D spherical tile surface patch on the WGS84 Ellipsoid (for Globe ECEF Mode)
    pub fn create_curved_globe_tile(
        device: &wgpu::Device,
        coord: crate::gis::basemap::TileCoord,
        subdivisions: u32,
    ) -> Self {
        let n_sub = if coord.z == 0 {
            64
        } else if coord.z == 1 {
            32
        } else if coord.z == 2 {
            16
        } else {
            subdivisions.max(8)
        };
        let mut vertices = Vec::with_capacity(((n_sub + 1) * (n_sub + 1)) as usize);
        let mut indices = Vec::with_capacity((n_sub * n_sub * 6) as usize);

        let n_tiles = (1u32 << coord.z) as f64;

        for j in 0..=n_sub {
            let v = j as f64 / n_sub as f64;
            let tile_y = coord.y as f64 + v;

            let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * tile_y / n_tiles))
                .sinh()
                .atan();
            let lat_deg = lat_rad.to_degrees().clamp(-85.05112878, 85.05112878);

            for i in 0..=n_sub {
                let u = i as f64 / n_sub as f64;
                let tile_x = coord.x as f64 + u;

                let lon_deg = (tile_x / n_tiles) * 360.0 - 180.0;

                let ecef_pos = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(
                    lat_deg,
                    lon_deg,
                    0.0,
                ));
                let normal = crate::gis::crs::geodetic_surface_normal(lat_deg, lon_deg);

                vertices.push(Vertex {
                    position: [ecef_pos.x, ecef_pos.y, ecef_pos.z],
                    normal: [normal.x, normal.y, normal.z],
                    uv: [u as f32, v as f32],
                    color: [1.0, 1.0, 1.0, 1.0],
                });
            }
        }

        let stride = n_sub + 1;
        for j in 0..n_sub {
            for i in 0..n_sub {
                let v00 = j * stride + i;
                let v10 = j * stride + (i + 1);
                let v01 = (j + 1) * stride + i;
                let v11 = (j + 1) * stride + (i + 1);

                // CCW winding
                indices.push(v00);
                indices.push(v11);
                indices.push(v10);

                indices.push(v00);
                indices.push(v01);
                indices.push(v11);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curved Globe Tile Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curved Globe Tile Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Creates a tessellated tile mesh containing both planar ENU coordinates (in `position`)
    /// and curved ECEF coordinates (in `color.xyz`) for GPU-accelerated terrain bending animation
    pub fn create_curved_morph_tile(
        device: &wgpu::Device,
        coord: TileCoord,
        origin: &crate::gis::crs::ProjectOrigin,
        n_sub: u32,
    ) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let n_tiles = 2.0_f64.powi(coord.z as i32);

        for j in 0..=n_sub {
            let v = j as f64 / n_sub as f64;
            let tile_y = coord.y as f64 + v;

            let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * tile_y / n_tiles))
                .sinh()
                .atan();
            let lat_deg = lat_rad.to_degrees().clamp(-85.05112878, 85.05112878);

            for i in 0..=n_sub {
                let u = i as f64 / n_sub as f64;
                let tile_x = coord.x as f64 + u;
                let lon_deg = (tile_x / n_tiles) * 360.0 - 180.0;

                // 1. Planar Local ENU coordinates
                let enu_pos = origin.lat_lon_to_local(lat_deg, lon_deg, 0.0);

                // 2. Globe 3D ECEF coordinates
                let ecef_pos = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(
                    lat_deg,
                    lon_deg,
                    0.0,
                ));

                vertices.push(Vertex {
                    position: [enu_pos.x, enu_pos.y, enu_pos.z],
                    normal: [0.0, 1.0, 0.0],
                    uv: [u as f32, v as f32],
                    color: [ecef_pos.x, ecef_pos.y, ecef_pos.z, 1.0],
                });
            }
        }

        let stride = n_sub + 1;
        for j in 0..n_sub {
            for i in 0..n_sub {
                let v00 = j * stride + i;
                let v10 = j * stride + (i + 1);
                let v01 = (j + 1) * stride + i;
                let v11 = (j + 1) * stride + (i + 1);

                // CCW winding
                indices.push(v00);
                indices.push(v11);
                indices.push(v10);

                indices.push(v00);
                indices.push(v01);
                indices.push(v11);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curved Morph Tile Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curved Morph Tile Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Creates a 3D Terrain Elevation tile mesh using Terrarium DEM heightmap data
    /// with continuous boundary sampling and deep edge skirts to eliminate tile gaps and cracks
    pub fn create_terrain_elevation_tile(
        device: &wgpu::Device,
        coord: TileCoord,
        origin: &crate::gis::crs::ProjectOrigin,
        terrain: Option<&crate::gis::terrain::TerrainManager>,
        terrain_tile: Option<&crate::gis::terrain::DecodedTerrainTile>,
        height_exaggeration: f32,
        n_sub: u32,
    ) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let n_tiles = 2.0_f64.powi(coord.z as i32);
        let stride = (n_sub + 1) as usize;

        // 1. Generate surface vertices with elevation
        for j in 0..=n_sub {
            let v = j as f64 / n_sub as f64;
            let tile_y = coord.y as f64 + v;

            let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * tile_y / n_tiles))
                .sinh()
                .atan();
            let lat_deg = lat_rad.to_degrees().clamp(-85.05112878, 85.05112878);

            for i in 0..=n_sub {
                let u = i as f64 / n_sub as f64;
                let tile_x = coord.x as f64 + u;
                let lon_deg = (tile_x / n_tiles) * 360.0 - 180.0;

                let base_elev = origin.origin.elevation;

                // Sample elevation height in meters from the decoded terrain DEM tile (defaulting to origin elevation Y=0)
                let geodetic_elev = if let Some(t) = terrain_tile {
                    t.sample_uv(u as f32, v as f32) as f64 * height_exaggeration as f64
                } else if let Some(mgr) = terrain {
                    if mgr.is_enabled {
                        if let Some(t) = mgr.get_terrain_for_tile(coord) {
                            t.sample_uv(u as f32, v as f32) as f64 * height_exaggeration as f64
                        } else {
                            mgr.sample_elevation(lat_deg, lon_deg)
                                .map(|h| h as f64)
                                .unwrap_or(base_elev)
                        }
                    } else {
                        base_elev
                    }
                } else {
                    base_elev
                };

                // Compute surface normal via central differences
                let normal = if let Some(t) = terrain_tile {
                    let eps = 1.0 / (n_sub as f32 * 2.0);
                    let h_left = t.sample_uv((u as f32 - eps).max(0.0), v as f32) * height_exaggeration;
                    let h_right = t.sample_uv((u as f32 + eps).min(1.0), v as f32) * height_exaggeration;
                    let h_up = t.sample_uv(u as f32, (v as f32 - eps).max(0.0)) * height_exaggeration;
                    let h_down = t.sample_uv(u as f32, (v as f32 + eps).min(1.0)) * height_exaggeration;

                    let meters_per_deg_lon = 111139.0 * lat_rad.cos().max(0.01);
                    let dx_meters = (360.0 / n_tiles * eps as f64 * 2.0 * meters_per_deg_lon) as f32;
                    let dy_meters = dx_meters;

                    let dh_dx = (h_right - h_left) / dx_meters.max(1.0);
                    let dh_dy = (h_down - h_up) / dy_meters.max(1.0);

                    let n = Vec3::new(-dh_dx, 1.0, -dh_dy).normalize_or_zero();
                    [n.x, n.y, n.z]
                } else if let Some(mgr) = terrain {
                    if mgr.is_enabled {
                        if let Some(t) = mgr.get_terrain_for_tile(coord) {
                            let eps = 1.0 / (n_sub as f32 * 2.0);
                            let h_left = t.sample_uv((u as f32 - eps).max(0.0), v as f32) * height_exaggeration;
                            let h_right = t.sample_uv((u as f32 + eps).min(1.0), v as f32) * height_exaggeration;
                            let h_up = t.sample_uv(u as f32, (v as f32 - eps).max(0.0)) * height_exaggeration;
                            let h_down = t.sample_uv(u as f32, (v as f32 + eps).min(1.0)) * height_exaggeration;

                            let meters_per_deg_lon = 111139.0 * lat_rad.cos().max(0.01);
                            let dx_meters = (360.0 / n_tiles * eps as f64 * 2.0 * meters_per_deg_lon) as f32;
                            let dy_meters = dx_meters;

                            let dh_dx = (h_right - h_left) / dx_meters.max(1.0);
                            let dh_dy = (h_down - h_up) / dy_meters.max(1.0);

                            let n = Vec3::new(-dh_dx, 1.0, -dh_dy).normalize_or_zero();
                            [n.x, n.y, n.z]
                        } else {
                            [0.0, 1.0, 0.0]
                        }
                    } else {
                        [0.0, 1.0, 0.0]
                    }
                } else {
                    [0.0, 1.0, 0.0]
                };

                // 1. Planar Local ENU coordinates with terrain height
                let enu_pos = origin.lat_lon_to_local(lat_deg, lon_deg, geodetic_elev);

                // 2. Globe 3D ECEF coordinates with terrain height
                let ecef_pos = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(
                    lat_deg,
                    lon_deg,
                    geodetic_elev,
                ));

                vertices.push(Vertex {
                    position: [enu_pos.x, enu_pos.y, enu_pos.z],
                    normal,
                    uv: [u as f32, v as f32],
                    color: [ecef_pos.x, ecef_pos.y, ecef_pos.z, 1.0],
                });
            }
        }

        // 2. Generate surface grid triangle indices
        let stride_u32 = stride as u32;
        for j in 0..n_sub {
            for i in 0..n_sub {
                let v00 = j * stride_u32 + i;
                let v10 = j * stride_u32 + (i + 1);
                let v01 = (j + 1) * stride_u32 + i;
                let v11 = (j + 1) * stride_u32 + (i + 1);

                indices.push(v00);
                indices.push(v11);
                indices.push(v10);

                indices.push(v00);
                indices.push(v01);
                indices.push(v11);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("3D Terrain Tile Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("3D Terrain Tile Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Solid Earth base ellipsoid (ocean body underneath globe tiles)
    pub fn create_earth_sphere(device: &wgpu::Device, rings: u32, sectors: u32) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let ocean_color = [0.07, 0.16, 0.28, 1.0];

        for r_idx in 0..=rings {
            let lat_frac = r_idx as f64 / rings as f64;
            let lat_deg = (lat_frac - 0.5) * 180.0; // -90 to +90

            for s_idx in 0..=sectors {
                let lon_frac = s_idx as f64 / sectors as f64;
                let lon_deg = lon_frac * 360.0 - 180.0; // -180 to +180

                // 10m beneath WGS84 ellipsoid so solid ocean backdrop is always flush directly behind surface
                let ecef_pos = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(
                    lat_deg,
                    lon_deg,
                    -10.0,
                ));
                let norm = crate::gis::crs::geodetic_surface_normal(lat_deg, lon_deg);

                vertices.push(Vertex {
                    position: [ecef_pos.x, ecef_pos.y, ecef_pos.z],
                    normal: [norm.x, norm.y, norm.z],
                    uv: [lon_frac as f32, 1.0 - lat_frac as f32],
                    color: ocean_color,
                });
            }
        }

        let stride = sectors + 1;
        for r_idx in 0..rings {
            for s_idx in 0..sectors {
                let v00 = r_idx * stride + s_idx;
                let v10 = r_idx * stride + (s_idx + 1);
                let v01 = (r_idx + 1) * stride + s_idx;
                let v11 = (r_idx + 1) * stride + (s_idx + 1);

                // CCW winding
                indices.push(v00);
                indices.push(v11);
                indices.push(v10);

                indices.push(v00);
                indices.push(v01);
                indices.push(v11);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Earth Sphere Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Earth Sphere Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }
}

