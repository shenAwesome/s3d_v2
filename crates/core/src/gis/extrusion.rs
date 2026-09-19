use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    #[serde(default)]
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl RawMeshData {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn append(&mut self, other: RawMeshData) {
        let base_idx = self.positions.len() as u32;
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.uvs.extend(other.uvs);
        self.colors.extend(other.colors);
        for idx in other.indices {
            self.indices.push(base_idx + idx);
        }
    }

    /// Creates an axis-aligned 3D box mesh between `min` and `max` coordinates with outward-facing facet normals and UVs
    pub fn create_box(min: Vec3, max: Vec3) -> Self {
        let x0 = min.x;
        let y0 = min.y;
        let z0 = min.z;
        let x1 = max.x;
        let y1 = max.y;
        let z1 = max.z;

        let mut positions = Vec::with_capacity(24);
        let mut normals = Vec::with_capacity(24);
        let mut uvs = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);

        let faces = [
            // Top (+Y)
            ([[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]], [0.0, 1.0, 0.0]),
            // Bottom (-Y)
            ([[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], [0.0, -1.0, 0.0]),
            // Front (+Z)
            ([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], [0.0, 0.0, 1.0]),
            // Back (-Z)
            ([[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]], [0.0, 0.0, -1.0]),
            // Right (+X)
            ([[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]], [1.0, 0.0, 0.0]),
            // Left (-X)
            ([[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]], [-1.0, 0.0, 0.0]),
        ];

        for (quad, normal) in faces {
            let base = positions.len() as u32;
            for p in quad {
                positions.push(p);
                normals.push(normal);
            }
            uvs.push([0.0, 0.0]);
            uvs.push([1.0, 0.0]);
            uvs.push([1.0, 1.0]);
            uvs.push([0.0, 1.0]);

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        Self {
            positions,
            normals,
            uvs,
            colors: Vec::new(),
            indices,
        }
    }

    /// Creates a 3D box mesh centered at `center` with dimensions `size` (width, height, depth)
    pub fn create_box_centered(center: Vec3, size: Vec3) -> Self {
        let half = size * 0.5;
        Self::create_box(center - half, center + half)
    }

    /// Creates a 3D cylinder mesh standing vertically from base `bottom_center` with given radius and height
    pub fn create_cylinder(bottom_center: Vec3, radius: f32, height: f32, segments: usize) -> Self {
        let segs = segments.max(6);
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();

        let y0 = bottom_center.y;
        let y1 = bottom_center.y + height;

        // Side vertices and quads
        for i in 0..segs {
            let theta0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let theta1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;

            let c0 = theta0.cos();
            let s0 = theta0.sin();
            let c1 = theta1.cos();
            let s1 = theta1.sin();

            let p00 = [bottom_center.x + radius * c0, y0, bottom_center.z + radius * s0];
            let p01 = [bottom_center.x + radius * c0, y1, bottom_center.z + radius * s0];
            let p10 = [bottom_center.x + radius * c1, y0, bottom_center.z + radius * s1];
            let p11 = [bottom_center.x + radius * c1, y1, bottom_center.z + radius * s1];

            let n0 = [c0, 0.0, s0];
            let n1 = [c1, 0.0, s1];

            let base = positions.len() as u32;
            positions.push(p00); normals.push(n0); uvs.push([i as f32 / segs as f32, 0.0]);
            positions.push(p10); normals.push(n1); uvs.push([(i + 1) as f32 / segs as f32, 0.0]);
            positions.push(p11); normals.push(n1); uvs.push([(i + 1) as f32 / segs as f32, 1.0]);
            positions.push(p01); normals.push(n0); uvs.push([i as f32 / segs as f32, 1.0]);

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        // Top cap (+Y)
        let top_center_idx = positions.len() as u32;
        positions.push([bottom_center.x, y1, bottom_center.z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.5, 0.5]);

        let top_rim_start = positions.len() as u32;
        for i in 0..segs {
            let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let c = theta.cos();
            let s = theta.sin();
            positions.push([bottom_center.x + radius * c, y1, bottom_center.z + radius * s]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.5 + 0.5 * c, 0.5 + 0.5 * s]);
        }
        for i in 0..segs {
            let i0 = top_rim_start + (i as u32);
            let i1 = top_rim_start + (((i + 1) % segs) as u32);
            indices.extend_from_slice(&[top_center_idx, i1, i0]);
        }

        // Bottom cap (-Y)
        let bot_center_idx = positions.len() as u32;
        positions.push([bottom_center.x, y0, bottom_center.z]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([0.5, 0.5]);

        let bot_rim_start = positions.len() as u32;
        for i in 0..segs {
            let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let c = theta.cos();
            let s = theta.sin();
            positions.push([bottom_center.x + radius * c, y0, bottom_center.z + radius * s]);
            normals.push([0.0, -1.0, 0.0]);
            uvs.push([0.5 + 0.5 * c, 0.5 + 0.5 * s]);
        }
        for i in 0..segs {
            let i0 = bot_rim_start + (i as u32);
            let i1 = bot_rim_start + (((i + 1) % segs) as u32);
            indices.extend_from_slice(&[bot_center_idx, i0, i1]);
        }

        Self {
            positions,
            normals,
            uvs,
            colors: Vec::new(),
            indices,
        }
    }

    /// Creates a 3D UV-sphere mesh centered at `center` with given radius
    pub fn create_sphere(center: Vec3, radius: f32, rings: usize, sectors: usize) -> Self {
        let num_rings = rings.max(6);
        let num_sectors = sectors.max(8);

        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();

        for r in 0..=num_rings {
            let phi = std::f32::consts::PI * (r as f32 / num_rings as f32);
            let y = phi.cos();
            let sin_phi = phi.sin();

            for s in 0..=num_sectors {
                let theta = std::f32::consts::TAU * (s as f32 / num_sectors as f32);
                let x = sin_phi * theta.cos();
                let z = sin_phi * theta.sin();

                let normal = [x, y, z];
                let pos = [center.x + radius * x, center.y + radius * y, center.z + radius * z];
                let u = s as f32 / num_sectors as f32;
                let v = r as f32 / num_rings as f32;

                positions.push(pos);
                normals.push(normal);
                uvs.push([u, v]);
            }
        }

        for r in 0..num_rings {
            for s in 0..num_sectors {
                let first = (r * (num_sectors + 1) + s) as u32;
                let second = first + (num_sectors + 1) as u32;

                indices.extend_from_slice(&[first, second, first + 1]);
                indices.extend_from_slice(&[second, second + 1, first + 1]);
            }
        }

        Self {
            positions,
            normals,
            uvs,
            colors: Vec::new(),
            indices,
        }
    }

    /// Extracts sharp geometric feature crease edges (dihedral angle > crease_angle_deg)
    /// and boundary/silhouette edges, filtering out coplanar and smooth cylinder facets.
    pub fn extract_crease_edges(&self, crease_angle_deg: f32) -> Vec<(Vec3, Vec3)> {
        if self.indices.len() < 3 || self.positions.is_empty() {
            return Vec::new();
        }

        // Map quantized position (1mm precision) to canonical vertex ID
        let mut pos_map: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::with_capacity(self.positions.len());
        let mut canonical_idx = Vec::with_capacity(self.positions.len());
        for pos in &self.positions {
            let key = [
                (pos[0] * 1000.0).round() as i64,
                (pos[1] * 1000.0).round() as i64,
                (pos[2] * 1000.0).round() as i64,
            ];
            let next_id = pos_map.len() as u32;
            let id = *pos_map.entry(key).or_insert(next_id);
            canonical_idx.push(id);
        }

        struct EdgeInfo {
            p1: Vec3,
            p2: Vec3,
            n1: Vec3,
            n2: Option<Vec3>,
            extra_count: u32,
        }

        let num_triangles = self.indices.len() / 3;
        let mut edge_map: std::collections::HashMap<(u32, u32), EdgeInfo> = std::collections::HashMap::with_capacity(num_triangles * 2);

        for t in 0..num_triangles {
            let i0 = self.indices[t * 3] as usize;
            let i1 = self.indices[t * 3 + 1] as usize;
            let i2 = self.indices[t * 3 + 2] as usize;

            if i0 >= self.positions.len() || i1 >= self.positions.len() || i2 >= self.positions.len() {
                continue;
            }

            let p0 = Vec3::from_array(self.positions[i0]);
            let p1 = Vec3::from_array(self.positions[i1]);
            let p2 = Vec3::from_array(self.positions[i2]);

            let normal = (p1 - p0).cross(p2 - p0);
            let len_sq = normal.length_squared();
            if len_sq < 1e-10 {
                continue; // Degenerate triangle
            }
            let face_normal = normal / len_sq.sqrt();

            let c0 = canonical_idx[i0];
            let c1 = canonical_idx[i1];
            let c2 = canonical_idx[i2];

            let tri_edges = [
                (c0, c1, p0, p1),
                (c1, c2, p1, p2),
                (c2, c0, p2, p0),
            ];

            for (ca, cb, pa, pb) in tri_edges {
                if ca == cb {
                    continue;
                }
                let (min_c, max_c, orig_pa, orig_pb) = if ca < cb {
                    (ca, cb, pa, pb)
                } else {
                    (cb, ca, pb, pa)
                };

                match edge_map.get_mut(&(min_c, max_c)) {
                    Some(info) => {
                        if info.n2.is_none() {
                            info.n2 = Some(face_normal);
                        } else {
                            info.extra_count += 1;
                        }
                    }
                    None => {
                        edge_map.insert((min_c, max_c), EdgeInfo {
                            p1: orig_pa,
                            p2: orig_pb,
                            n1: face_normal,
                            n2: None,
                            extra_count: 0,
                        });
                    }
                }
            }
        }

        let cos_thresh = crease_angle_deg.to_radians().cos();
        let mut result = Vec::new();

        for (_, edge) in edge_map {
            if edge.n2.is_none() {
                // Boundary / silhouette edge (e.g. building base or open rooftop contour)
                result.push((edge.p1, edge.p2));
            } else if let Some(n2) = edge.n2 {
                if edge.extra_count > 0 {
                    // Non-manifold edge
                    result.push((edge.p1, edge.p2));
                } else {
                    let dot = edge.n1.dot(n2);
                    // If dot < cos_thresh, dihedral angle > crease_angle_deg
                    if dot < cos_thresh {
                        result.push((edge.p1, edge.p2));
                    }
                }
            }
        }

        result
    }
}

/// Compute signed 2D area using shoelace formula
/// Positive = Counter-Clockwise (CCW), Negative = Clockwise (CW)
pub fn signed_ring_area_2d(ring: &[Vec2]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let next = (i + 1) % n;
        area += (ring[i].x as f64) * (ring[next].y as f64) - (ring[next].x as f64) * (ring[i].y as f64);
    }
    area * 0.5
}

/// Compute true geometric area centroid (center of mass) of a 2D ring using the Green's theorem / Shoelace formula.
/// Strictly rotation-invariant for any 2D polygon.
pub fn polygon_centroid_2d(ring: &[Vec2]) -> Vec2 {
    let n = ring.len();
    if n < 3 {
        if ring.is_empty() {
            return Vec2::ZERO;
        }
        let sum: Vec2 = ring.iter().copied().sum();
        return sum / (n as f32);
    }
    let mut area = 0.0f64;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    for i in 0..n {
        let next = (i + 1) % n;
        let xi = ring[i].x as f64;
        let yi = ring[i].y as f64;
        let xj = ring[next].x as f64;
        let yj = ring[next].y as f64;
        let factor = xi * yj - xj * yi;
        area += factor;
        cx += (xi + xj) * factor;
        cy += (yi + yj) * factor;
    }
    area *= 0.5;
    if area.abs() > 1e-6 {
        let inv = 1.0 / (6.0 * area);
        Vec2::new((cx * inv) as f32, (cy * inv) as f32)
    } else {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &p in ring {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5)
    }
}

/// Clean ring by removing duplicate consecutive points and closing points
fn clean_ring(pts: &[Vec2]) -> Vec<Vec2> {
    let mut clean: Vec<Vec2> = Vec::new();
    for &p in pts {
        if let Some(&last) = clean.last() {
            let diff: Vec2 = p - last;
            if diff.length_squared() > 1e-8_f32 {
                clean.push(p);
            }
        } else {
            clean.push(p);
        }
    }
    // Remove closing point if same as start
    if clean.len() > 2 {
        let diff: Vec2 = clean[0] - *clean.last().unwrap();
        if diff.length_squared() < 1e-8_f32 {
            clean.pop();
        }
    }
    clean
}

/// Extrude a 2D polygon with optional interior holes and optional per-vertex ground elevations
/// rings: 0 is exterior boundary, 1.. are interior holes
/// Each ring is a list of 2D coordinates in local meters (x = East, y = North = -z in 3D)
pub fn extrude_polygon_with_elevation(
    rings: &[Vec<Vec2>],
    base_elevations: Option<&[Vec<f32>]>,
    uniform_base: f32,
    height: f32,
) -> Option<RawMeshData> {
    if rings.is_empty() {
        return None;
    }

    // Clean and normalize orientations:
    // Exterior ring must be CCW (signed_area > 0)
    // Hole rings must be CW (signed_area < 0)
    let mut normalized_rings: Vec<Vec<Vec2>> = Vec::new();
    let mut normalized_elevs: Vec<Vec<f32>> = Vec::new();

    for (i, r) in rings.iter().enumerate() {
        let mut clean = clean_ring(r);
        if clean.len() < 3 {
            continue;
        }
        let area = signed_ring_area_2d(&clean);

        let mut elevs = if let Some(base_elevs) = base_elevations {
            if let Some(r_elev) = base_elevs.get(i) {
                if r_elev.len() == r.len() {
                    let mut clean_elev = Vec::new();
                    for (idx, &p) in r.iter().enumerate() {
                        if clean.contains(&p) {
                            clean_elev.push(r_elev[idx]);
                        }
                    }
                    while clean_elev.len() < clean.len() {
                        clean_elev.push(uniform_base);
                    }
                    clean_elev.truncate(clean.len());
                    clean_elev
                } else {
                    vec![uniform_base; clean.len()]
                }
            } else {
                vec![uniform_base; clean.len()]
            }
        } else {
            vec![uniform_base; clean.len()]
        };

        if i == 0 {
            if area < 0.0 {
                clean.reverse();
                elevs.reverse();
            }
        } else {
            if area > 0.0 {
                clean.reverse();
                elevs.reverse();
            }
        }
        normalized_rings.push(clean);
        normalized_elevs.push(elevs);
    }

    if normalized_rings.is_empty() || normalized_rings[0].len() < 3 {
        return None;
    }

    let mut max_base = uniform_base;
    for r_elev in &normalized_elevs {
        for &h in r_elev {
            max_base = max_base.max(h);
        }
    }
    let _ = max_base;
    let mut mesh = RawMeshData::new();

    // 1. Flatten 2D rings for earcutr triangulation
    let mut flat_coords: Vec<f64> = Vec::new();
    let mut hole_indices: Vec<usize> = Vec::new();
    let mut current_idx = 0;

    for (ring_idx, ring) in normalized_rings.iter().enumerate() {
        if ring_idx > 0 {
            hole_indices.push(current_idx);
        }
        for pt in ring {
            flat_coords.push(pt.x as f64);
            flat_coords.push(pt.y as f64);
            current_idx += 1;
        }
    }

    let ear_indices = match earcutr::earcut(&flat_coords, &hole_indices, 2) {
        Ok(idx) if !idx.is_empty() => idx,
        _ => return None,
    };

    // 2. Generate Roof Cap (triangulated roof honoring per-vertex elevations)
    let roof_base_index = mesh.positions.len() as u32;
    for (ring_idx, ring) in normalized_rings.iter().enumerate() {
        for (pt_idx, pt) in ring.iter().enumerate() {
            let bot_y = normalized_elevs[ring_idx][pt_idx];
            let v_top_y = bot_y + height;
            mesh.positions.push([pt.x, v_top_y, -pt.y]);
            mesh.normals.push([0.0, 1.0, 0.0]); // initial default normal, will refine with face normals
            mesh.uvs.push([pt.x * 0.1, pt.y * 0.1]);
        }
    }

    for chunk in ear_indices.chunks_exact(3) {
        let idx0 = roof_base_index + chunk[0] as u32;
        let idx1 = roof_base_index + chunk[1] as u32;
        let idx2 = roof_base_index + chunk[2] as u32;
        mesh.indices.push(idx0);
        mesh.indices.push(idx1);
        mesh.indices.push(idx2);

        // Compute face normal for slanted / 3D terrain roofs
        let p0 = Vec3::from_array(mesh.positions[idx0 as usize]);
        let p1 = Vec3::from_array(mesh.positions[idx1 as usize]);
        let p2 = Vec3::from_array(mesh.positions[idx2 as usize]);
        let fnorm = (p1 - p0).cross(p2 - p0).normalize_or_zero();
        if fnorm.length_squared() > 0.5 {
            let n_arr = [fnorm.x, fnorm.y, fnorm.z];
            mesh.normals[idx0 as usize] = n_arr;
            mesh.normals[idx1 as usize] = n_arr;
            mesh.normals[idx2 as usize] = n_arr;
        }
    }

    // 3. Generate Floor Cap (bottom, normal [0, -1, 0] at each vertex's base elevation)
    let floor_base_index = mesh.positions.len() as u32;
    for (ring_idx, ring) in normalized_rings.iter().enumerate() {
        for (pt_idx, pt) in ring.iter().enumerate() {
            let bot_y = normalized_elevs[ring_idx][pt_idx];
            mesh.positions.push([pt.x, bot_y, -pt.y]);
            mesh.normals.push([0.0, -1.0, 0.0]);
            mesh.uvs.push([pt.x * 0.1, pt.y * 0.1]);
        }
    }

    for chunk in ear_indices.chunks_exact(3) {
        mesh.indices.push(floor_base_index + chunk[0] as u32);
        mesh.indices.push(floor_base_index + chunk[2] as u32);
        mesh.indices.push(floor_base_index + chunk[1] as u32);
    }

    // 4. Generate Vertical Wall Quads
    for (ring_idx, ring) in normalized_rings.iter().enumerate() {
        let n = ring.len();
        if n < 3 {
            continue;
        }

        for i in 0..n {
            let next_i = (i + 1) % n;
            let p0_2d = ring[i];
            let p1_2d = ring[next_i];

            let bot_y0 = normalized_elevs[ring_idx][i];
            let bot_y1 = normalized_elevs[ring_idx][next_i];
            let top_y0 = bot_y0 + height;
            let top_y1 = bot_y1 + height;

            let edge_len = (p1_2d - p0_2d).length();
            if edge_len < 1e-4 {
                continue;
            }

            let p0_3d_bot = Vec3::new(p0_2d.x, bot_y0, -p0_2d.y);
            let p1_3d_bot = Vec3::new(p1_2d.x, bot_y1, -p1_2d.y);
            let p1_3d_top = Vec3::new(p1_2d.x, top_y1, -p1_2d.y);
            let p0_3d_top = Vec3::new(p0_2d.x, top_y0, -p0_2d.y);

            let tangent = Vec3::new(p1_2d.x - p0_2d.x, 0.0, -(p1_2d.y - p0_2d.y));
            let up = Vec3::new(0.0, 1.0, 0.0);
            let wall_normal = tangent.cross(up).normalize_or_zero();

            let v_start = mesh.positions.len() as u32;

            mesh.positions.push([p0_3d_bot.x, p0_3d_bot.y, p0_3d_bot.z]); // V0 bottom-left
            mesh.positions.push([p1_3d_bot.x, p1_3d_bot.y, p1_3d_bot.z]); // V1 bottom-right
            mesh.positions.push([p1_3d_top.x, p1_3d_top.y, p1_3d_top.z]); // V2 top-right
            mesh.positions.push([p0_3d_top.x, p0_3d_top.y, p0_3d_top.z]); // V3 top-left

            for _ in 0..4 {
                mesh.normals.push([wall_normal.x, wall_normal.y, wall_normal.z]);
            }

            mesh.uvs.push([0.0, 0.0]);
            mesh.uvs.push([edge_len * 0.2, 0.0]);
            mesh.uvs.push([edge_len * 0.2, height * 0.2]);
            mesh.uvs.push([0.0, height * 0.2]);

            mesh.indices.push(v_start);
            mesh.indices.push(v_start + 1);
            mesh.indices.push(v_start + 2);

            mesh.indices.push(v_start);
            mesh.indices.push(v_start + 2);
            mesh.indices.push(v_start + 3);
        }
    }

    Some(mesh)
}

/// Extrude a 2D polygon with uniform base elevation
pub fn extrude_polygon(
    rings: &[Vec<Vec2>],
    base_elevation: f32,
    height: f32,
) -> Option<RawMeshData> {
    extrude_polygon_with_elevation(rings, None, base_elevation, height)
}

/// Helper to extrude and merge multiple polygon features into a single RawMeshData.
/// Eliminates code duplication between `rebuild_layer_feature_meshes` and `update_features_elevation_from_terrain_tile`.
pub fn merge_polygons_to_mesh(
    polygons: &[Vec<Vec<Vec2>>],
    base_y: f32,
    height: f32,
) -> Option<RawMeshData> {
    let mut combined_mesh = RawMeshData::new();
    for poly in polygons {
        if let Some(poly_mesh) = extrude_polygon(poly, base_y, height) {
            combined_mesh.append(poly_mesh);
        }
    }
    if combined_mesh.is_empty() {
        None
    } else {
        Some(combined_mesh)
    }
}

/// Drapes a 2D GIS polygon directly onto the 3D terrain surface like Esri ArcGIS Pro.
/// Samples elevations per-vertex, triangulates the surface cap, and adds a crisp perimeter border ribbon.
pub fn drape_polygon_on_terrain(
    rings: &[Vec<Vec2>],
    base_elevations: Option<&[Vec<f32>]>,
    uniform_base: f32,
    border_width: f32,
    fill_color: [f32; 4],
    stroke_color: [f32; 4],
) -> Option<RawMeshData> {
    if rings.is_empty() {
        return None;
    }

    let mut normalized_rings: Vec<Vec<Vec2>> = Vec::new();
    let mut normalized_elevs: Vec<Vec<f32>> = Vec::new();

    for (i, r) in rings.iter().enumerate() {
        let mut clean = clean_ring(r);
        if clean.len() < 3 {
            continue;
        }
        let area = signed_ring_area_2d(&clean);

        let mut elevs = if let Some(base_elevs) = base_elevations {
            if let Some(r_elev) = base_elevs.get(i) {
                if r_elev.len() == r.len() {
                    let mut clean_elev = Vec::new();
                    for (idx, &p) in r.iter().enumerate() {
                        if clean.contains(&p) {
                            clean_elev.push(r_elev[idx]);
                        }
                    }
                    while clean_elev.len() < clean.len() {
                        clean_elev.push(uniform_base);
                    }
                    clean_elev.truncate(clean.len());
                    clean_elev
                } else {
                    vec![uniform_base; clean.len()]
                }
            } else {
                vec![uniform_base; clean.len()]
            }
        } else {
            vec![uniform_base; clean.len()]
        };

        if i == 0 {
            if area < 0.0 {
                clean.reverse();
                elevs.reverse();
            }
        } else {
            if area > 0.0 {
                clean.reverse();
                elevs.reverse();
            }
        }
        normalized_rings.push(clean);
        normalized_elevs.push(elevs);
    }

    if normalized_rings.is_empty() || normalized_rings[0].len() < 3 {
        return None;
    }

    let mut mesh = RawMeshData::new();

    // 1. Flatten 2D rings for earcutr triangulation
    let mut flat_coords: Vec<f64> = Vec::new();
    let mut hole_indices: Vec<usize> = Vec::new();
    let mut current_idx = 0;

    for (ring_idx, ring) in normalized_rings.iter().enumerate() {
        if ring_idx > 0 {
            hole_indices.push(current_idx);
        }
        for pt in ring {
            flat_coords.push(pt.x as f64);
            flat_coords.push(pt.y as f64);
            current_idx += 1;
        }
    }

    let ear_indices = match earcutr::earcut(&flat_coords, &hole_indices, 2) {
        Ok(idx) if !idx.is_empty() => idx,
        _ => return None,
    };

    // 2. Draped Top Surface Cap (normal pointing straight UP [0, 1, 0] at vertex terrain elevation)
    let cap_base_index = mesh.positions.len() as u32;
    for (ring_idx, ring) in normalized_rings.iter().enumerate() {
        for (pt_idx, pt) in ring.iter().enumerate() {
            let elev_y = normalized_elevs[ring_idx][pt_idx];
            mesh.positions.push([pt.x, elev_y, -pt.y]);
            mesh.normals.push([0.0, 1.0, 0.0]);
            mesh.uvs.push([pt.x * 0.05, pt.y * 0.05]);
            mesh.colors.push(fill_color);
        }
    }

    for chunk in ear_indices.chunks_exact(3) {
        mesh.indices.push(cap_base_index + chunk[0] as u32);
        mesh.indices.push(cap_base_index + chunk[1] as u32);
        mesh.indices.push(cap_base_index + chunk[2] as u32);
    }

    // 3. Crisp Planar Perimeter Border Ribbon (flat on terrain, elevated +0.08m above fill cap)
    if border_width > 0.0 {
        let width = border_width.clamp(0.8, 3.5);
        for (ring_idx, ring) in normalized_rings.iter().enumerate() {
            let n = ring.len();
            if n < 3 {
                continue;
            }

            let ring_base = mesh.positions.len() as u32;

            for i in 0..n {
                let prev_i = (i + n - 1) % n;
                let next_i = (i + 1) % n;

                let p_prev = ring[prev_i];
                let p_curr = ring[i];
                let p_next = ring[next_i];

                let d0 = (p_curr - p_prev).normalize_or_zero();
                let d1 = (p_next - p_curr).normalize_or_zero();

                let n0 = Vec2::new(-d0.y, d0.x);
                let n1 = Vec2::new(-d1.y, d1.x);
                let mut miter = n0 + n1;
                if miter.length_squared() < 1e-4 {
                    miter = n0;
                } else {
                    miter = miter.normalize();
                }

                // Elevated +0.08m above the fill cap so the fill NEVER covers or blocks the border line!
                let elev_y = normalized_elevs[ring_idx][i] + 0.08;
                let p_inner = p_curr + miter * width;

                // Outer boundary vertex
                mesh.positions.push([p_curr.x, elev_y, -p_curr.y]);
                mesh.normals.push([0.0, 1.0, 0.0]);
                mesh.uvs.push([0.0, 0.0]);
                mesh.colors.push(stroke_color);

                // Inner perimeter vertex
                mesh.positions.push([p_inner.x, elev_y, -p_inner.y]);
                mesh.normals.push([0.0, 1.0, 0.0]);
                mesh.uvs.push([1.0, 1.0]);
                mesh.colors.push(stroke_color);
            }

            for i in 0..n {
                let next_i = (i + 1) % n;
                let v0 = ring_base + (i * 2) as u32;
                let v1 = ring_base + (i * 2 + 1) as u32;
                let v2 = ring_base + (next_i * 2) as u32;
                let v3 = ring_base + (next_i * 2 + 1) as u32;

                mesh.indices.push(v0);
                mesh.indices.push(v1);
                mesh.indices.push(v2);

                mesh.indices.push(v2);
                mesh.indices.push(v1);
                mesh.indices.push(v3);
            }
        }
    }

    Some(mesh)
}

