use super::b3dm::B3dmParser;
use super::spec::{
    calculate_screen_space_error, get_bounding_volume_center_ecef,
    get_bounding_volume_radius, lat_lon_alt_to_standard_ecef, RefinementMode, Tile3DNode, TilesetJson,
};
use crate::gis::crs::ProjectOrigin;
use crate::renderer::camera::Camera;
use glam::{DMat4, DVec3, Vec2, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

// ----------------------------------------------------
// 3D Tiles Presets
// ----------------------------------------------------

pub struct ThreeDTilePreset {
    pub name: &'static str,
    pub url: &'static str,
}

pub const THREE_D_TILES_PRESETS: &[ThreeDTilePreset] = &[
    ThreeDTilePreset {
        name: "Geelong CBD (3D Buildings)",
        url: "https://dt-geelong.s3.ap-southeast-2.amazonaws.com/building/geelong/tileset.json",
    },
];

// ----------------------------------------------------
// ----------------------------------------------------
// Decoded GPU 3D Tile Mesh Payload
// ----------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecodedThreeDTileSubMesh {
    pub index_offset: u32,
    pub index_count: u32,
    pub base_color: [f32; 4],
    pub image_rgba: Option<std::sync::Arc<(u32, u32, Vec<u8>)>>,
}

#[derive(Debug, Clone)]
pub struct ThreeDTileFeatureMesh {
    pub feature_id: u32,
    pub name: Option<String>,
    pub vertex_indices: Vec<u32>,
    pub raw_mesh: crate::gis::extrusion::RawMeshData,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub center_geo: crate::gis::crs::GeoCoord,
    pub height: f32,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct DecodedThreeDTileMesh {
    pub id: String,
    pub positions: Vec<Vec3>,     // Local ENU coordinates
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub ecef_positions: Vec<Vec3>, // Globe ECEF coordinates (for Planar <-> Globe morphing)
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub image_rgba: Option<std::sync::Arc<(u32, u32, Vec<u8>)>>,
    pub submeshes: Vec<DecodedThreeDTileSubMesh>,
    pub batch_table: HashMap<String, Vec<serde_json::Value>>,
    pub features: Vec<ThreeDTileFeatureMesh>,
}

impl DecodedThreeDTileMesh {
    /// Converts this 3D tile mesh into an engine RawMeshData suitable for selection highlighting and collision
    pub fn to_raw_mesh(&self) -> crate::gis::extrusion::RawMeshData {
        crate::gis::extrusion::RawMeshData {
            positions: self.positions.iter().map(|p| [p.x, p.y, p.z]).collect(),
            normals: self.normals.iter().map(|n| [n.x, n.y, n.z]).collect(),
            uvs: self.uvs.iter().map(|u| [u.x, u.y]).collect(),
            colors: vec![self.base_color; self.positions.len()],
            indices: self.indices.clone(),
        }
    }
}

// ----------------------------------------------------
// Asynchronous Background Worker Messages
// ----------------------------------------------------

#[allow(dead_code)]
enum TileRequest {
    FetchTileset {
        url: String,
    },
    FetchContent {
        id: String,
        full_url: String,
        accum_transform: DMat4,
        origin: ProjectOrigin,
        gltf_up_axis: Option<String>,
        height_offset: f32,
    },
}

enum TileResponse {
    TilesetLoaded {
        tileset: Result<TilesetJson, String>,
    },
    ContentLoaded {
        id: String,
        result: Result<DecodedThreeDTileMesh, String>,
    },
}

// ----------------------------------------------------
// Tiles3D Manager
// ----------------------------------------------------

pub struct Tiles3DManager {
    pub service_url: String,
    pub is_enabled: bool,
    pub opacity: f32,
    pub tint: [f32; 3],
    pub replace_texture: bool,
    pub height_offset: f32,
    pub maximum_screen_space_error: f32,

    pub tileset: Option<TilesetJson>,
    pub base_url: String,

    pub pending_requests: HashSet<String>,
    pub loaded_tiles: HashMap<String, DecodedThreeDTileMesh>,
    pub current_origin: Option<ProjectOrigin>,

    #[allow(dead_code)]
    tx_req: Sender<TileRequest>,
    #[allow(dead_code)]
    tx_resp: Sender<TileResponse>,
    rx_resp: Mutex<Receiver<TileResponse>>,

    pub status_message: String,
    pub is_loading: bool,
}

impl Tiles3DManager {
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let (tx_req, rx_req) = channel::<TileRequest>();
        #[cfg(target_arch = "wasm32")]
        let (tx_req, _rx_req) = channel::<TileRequest>();
        let (tx_resp, rx_resp) = channel::<TileResponse>();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let tx_worker = tx_resp.clone();
            // Multi-threaded background streaming worker
            thread::Builder::new()
                .name("s3d-3dtiles-worker".to_string())
                .spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(std::time::Duration::from_secs(8))
                    .timeout_read(std::time::Duration::from_secs(15))
                    .build();

                while let Ok(req) = rx_req.recv() {
                    match req {
                        TileRequest::FetchTileset { url } => {
                            log::debug!("[3D Tiles] Fetching tileset.json: {}", url);
                            let res = (|| -> Result<TilesetJson, String> {
                                let resp = agent
                                    .get(&url)
                                    .call()
                                    .map_err(|e| format!("HTTP request failed: {}", e))?;
                                let text = resp
                                    .into_string()
                                    .map_err(|e| format!("Failed to read text: {}", e))?;
                                let ts = TilesetJson::from_json_str(&text)?;
                                log::debug!("[3D Tiles] 200 OK (Tileset loaded): {}", url);
                                Ok(ts)
                            })();
                            let _ = tx_worker.send(TileResponse::TilesetLoaded {
                                tileset: res,
                            });
                        }
                        TileRequest::FetchContent {
                            id,
                            full_url,
                            accum_transform,
                            origin,
                            gltf_up_axis,
                            height_offset,
                        } => {
                            log::debug!("[3D Tiles] Fetching B3DM: {}", full_url);
                            let res = (|| -> Result<DecodedThreeDTileMesh, String> {
                                let mut reader = agent
                                    .get(&full_url)
                                    .call()
                                    .map_err(|e| {
                                        log::warn!("[3D Tiles] Error fetching {}: {}", full_url, e);
                                        format!("HTTP request failed: {}", e)
                                    })?
                                    .into_reader();
                                let mut bytes = Vec::new();
                                std::io::Read::read_to_end(&mut reader, &mut bytes)
                                    .map_err(|e| format!("Failed to read bytes: {}", e))?;

                                let parsed = B3dmParser::parse(&bytes)?;
                                let mesh = Self::convert_parsed_to_mesh(&id, parsed, &accum_transform, &origin, gltf_up_axis.as_deref(), height_offset)?;
                                if !mesh.positions.is_empty() {
                                    log::debug!(
                                        "[3D Tiles] Decoded tile '{}': {} vertices, {} triangles",
                                        id, mesh.positions.len(), mesh.indices.len() / 3
                                    );
                                }
                                Ok(mesh)
                            })();
                            let _ = tx_worker.send(TileResponse::ContentLoaded { id, result: res });
                        }
                    }
                }
            })
            .ok();
        }

        Self {
            service_url: THREE_D_TILES_PRESETS[0].url.to_string(),
            is_enabled: false,
            opacity: 1.0,
            tint: [1.0, 1.0, 1.0],
            replace_texture: false,
            height_offset: 0.0,
            maximum_screen_space_error: 16.0,
            tileset: None,
            base_url: String::new(),
            pending_requests: HashSet::new(),
            loaded_tiles: HashMap::new(),
            current_origin: None,
            tx_req,
            tx_resp,
            rx_resp: Mutex::new(rx_resp),
            status_message: "Ready".to_string(),
            is_loading: false,
        }
    }

    pub fn reset_texture(&mut self) {
        self.replace_texture = false;
        self.tint = [1.0, 1.0, 1.0];
    }

    pub fn set_service_url(&mut self, url: &str) {
        let trimmed = url.trim();
        if trimmed == self.service_url && self.tileset.is_some() {
            return;
        }
        self.service_url = trimmed.to_string();
        self.tileset = None;
        self.pending_requests.clear();
        self.loaded_tiles.clear();
        self.current_origin = None;

        if let Some(pos) = trimmed.rfind('/') {
            self.base_url = trimmed[..=pos].to_string();
        } else {
            self.base_url = format!("{}/", trimmed);
        }

        if self.is_enabled && !self.service_url.is_empty() {
            self.fetch_tileset();
        }
    }

    pub fn fetch_tileset(&mut self) {
        if self.service_url.is_empty() {
            return;
        }
        self.is_loading = true;
        self.status_message = format!("Fetching tileset from {}", self.service_url);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.tx_req.send(TileRequest::FetchTileset {
            url: self.service_url.clone(),
        });
        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.tx_resp.clone();
            let url = self.service_url.clone();
            crate::gis::platform::http::fetch_text(&url, move |res| {
                let ts_res = res.and_then(|text| TilesetJson::from_json_str(&text));
                let _ = tx.send(TileResponse::TilesetLoaded { tileset: ts_res });
            });
        }
    }

    pub fn clear(&mut self) {
        self.loaded_tiles.clear();
        self.pending_requests.clear();
        self.tileset = None;
        self.current_origin = None;
    }

    /// Returns true if 3D tileset or 3D tile geometries are actively downloading in background
    pub fn is_streaming(&self) -> bool {
        self.is_enabled && (self.is_loading || !self.pending_requests.is_empty() || (self.tileset.is_none() && !self.service_url.is_empty()))
    }

    /// Clears cached mesh buffers without discarding the loaded tileset tree
    pub fn clear_mesh_cache(&mut self) {
        self.loaded_tiles.clear();
        self.pending_requests.clear();
        self.current_origin = None;
    }

    /// Reprojects all in-memory loaded 3D tile meshes to a new ProjectOrigin without network refetching
    pub fn reproject_all(&mut self, origin: &ProjectOrigin) -> Vec<DecodedThreeDTileMesh> {
        self.current_origin = Some(*origin);
        let lat0_rad = origin.origin.latitude.to_radians();
        let lon0_rad = origin.origin.longitude.to_radians();
        let sin_lat0 = lat0_rad.sin();
        let cos_lat0 = lat0_rad.cos();
        let sin_lon0 = lon0_rad.sin();
        let cos_lon0 = lon0_rad.cos();

        let n_wgs = 6378137.0 / (1.0 - 0.00669437999014 * sin_lat0 * sin_lat0).sqrt();
        let origin_ecef = DVec3::new(
            (n_wgs + origin.origin.elevation) * cos_lat0 * cos_lon0,
            (n_wgs + origin.origin.elevation) * cos_lat0 * sin_lon0,
            (n_wgs * (1.0 - 0.00669437999014) + origin.origin.elevation) * sin_lat0,
        );

        let mut reprojected = Vec::with_capacity(self.loaded_tiles.len());
        for mesh in self.loaded_tiles.values_mut() {
            let mut min_pos = Vec3::splat(f32::MAX);
            let mut max_pos = Vec3::splat(f32::MIN);

            for (i, ecef_vert) in mesh.ecef_positions.iter().enumerate() {
                // ecef_positions in Engine ECEF (X, Z, -Y), so standard ECEF is (X, -Z_engine, Y_engine)
                let ecef_d = DVec3::new(ecef_vert.x as f64, -ecef_vert.z as f64, ecef_vert.y as f64);
                let rel_ecef = ecef_d - origin_ecef;
                let enu_east = -sin_lon0 * rel_ecef.x + cos_lon0 * rel_ecef.y;
                let enu_north = -sin_lat0 * cos_lon0 * rel_ecef.x - sin_lat0 * sin_lon0 * rel_ecef.y + cos_lat0 * rel_ecef.z;
                let enu_up = cos_lat0 * cos_lon0 * rel_ecef.x + cos_lat0 * sin_lon0 * rel_ecef.y + sin_lat0 * rel_ecef.z;

                let p_local = Vec3::new(enu_east as f32, (enu_up as f32) + self.height_offset, -enu_north as f32);
                if i < mesh.positions.len() {
                    mesh.positions[i] = p_local;
                }
                min_pos = min_pos.min(p_local);
                max_pos = max_pos.max(p_local);
            }
            if !mesh.positions.is_empty() {
                mesh.bounds_min = min_pos;
                mesh.bounds_max = max_pos;
            }

            // Update individual segmented features
            for feat in &mut mesh.features {
                let mut f_min = Vec3::splat(f32::MAX);
                let mut f_max = Vec3::splat(f32::MIN);
                for (slot, &src_idx) in feat.vertex_indices.iter().enumerate() {
                    if let Some(pos) = mesh.positions.get(src_idx as usize) {
                        if slot < feat.raw_mesh.positions.len() {
                            feat.raw_mesh.positions[slot] = [pos.x, pos.y, pos.z];
                        }
                        f_min = f_min.min(*pos);
                        f_max = f_max.max(*pos);
                    }
                }
                if !feat.vertex_indices.is_empty() {
                    feat.aabb_min = f_min;
                    feat.aabb_max = f_max;
                    feat.height = (f_max.y - f_min.y).max(0.0);
                    let center_local = (f_min + f_max) * 0.5;
                    feat.center_geo = origin.local_to_geo(center_local);
                }
            }

            reprojected.push(mesh.clone());
        }
        reprojected
    }

    /// Sets elevation offset in meters and recalculates all loaded 3D tile meshes
    pub fn set_height_offset(&mut self, offset: f32) -> Vec<DecodedThreeDTileMesh> {
        self.height_offset = offset;
        if let Some(origin) = self.current_origin {
            self.reproject_all(&origin)
        } else {
            Vec::new()
        }
    }

    /// Converts parsed B3DM primitives into an engine-compatible `DecodedThreeDTileMesh`
    pub(crate) fn convert_parsed_to_mesh(
        id: &str,
        parsed: super::b3dm::ParsedB3dmModel,
        accum_transform: &DMat4,
        origin: &ProjectOrigin,
        gltf_up_axis: Option<&str>,
        height_offset: f32,
    ) -> Result<DecodedThreeDTileMesh, String> {
        let is_z_up = gltf_up_axis.map(|s| s.eq_ignore_ascii_case("z")).unwrap_or(false);
        let mut total_positions = Vec::new();
        let mut total_normals = Vec::new();
        let mut total_uvs = Vec::new();
        let mut total_ecef = Vec::new();
        let mut total_indices = Vec::new();
        let mut total_batch_ids = Vec::new();
        let mut base_color = [0.85, 0.88, 0.92, 1.0];

        let rtc_offset = if let Some(rtc) = parsed.rtc_center {
            DVec3::new(rtc[0], rtc[1], rtc[2])
        } else {
            DVec3::ZERO
        };

        let image_rgba = parsed.model.image_rgba;

        // Exact origin ECEF center and orthonormal ENU rotation basis
        let lat0_rad = origin.origin.latitude.to_radians();
        let lon0_rad = origin.origin.longitude.to_radians();
        let sin_lat0 = lat0_rad.sin();
        let cos_lat0 = lat0_rad.cos();
        let sin_lon0 = lon0_rad.sin();
        let cos_lon0 = lon0_rad.cos();

        // WGS84 ECEF coordinates of project origin
        let n_wgs = 6378137.0 / (1.0 - 0.00669437999014 * sin_lat0 * sin_lat0).sqrt();
        let origin_ecef = DVec3::new(
            (n_wgs + origin.origin.elevation) * cos_lat0 * cos_lon0,
            (n_wgs + origin.origin.elevation) * cos_lat0 * sin_lon0,
            (n_wgs * (1.0 - 0.00669437999014) + origin.origin.elevation) * sin_lat0,
        );

        let mut submeshes = Vec::new();

        for prim in &parsed.model.primitives {
            base_color = prim.base_color;
            let start_index = total_indices.len() as u32;
            let index_offset = total_positions.len() as u32;

            for (i, p) in prim.positions.iter().enumerate() {
                // If gltfUpAxis is "Z", geometry is already Z-up [X, Y, Z]. Otherwise standard glTF is Y-up [X, -Z, Y]
                let zup_p = if is_z_up {
                    DVec3::new(p.x as f64, p.y as f64, p.z as f64) + rtc_offset
                } else {
                    DVec3::new(p.x as f64, -(p.z as f64), p.y as f64) + rtc_offset
                };
                let ecef_d = accum_transform.transform_point3(zup_p);

                // 1. Direct Orthonormal ECEF-to-ENU Transformation (Rigid 3D Isometry, Zero Angular Distortion)
                let rel_ecef = ecef_d - origin_ecef;
                let enu_east = -sin_lon0 * rel_ecef.x + cos_lon0 * rel_ecef.y;
                let enu_north = -sin_lat0 * cos_lon0 * rel_ecef.x - sin_lat0 * sin_lon0 * rel_ecef.y + cos_lat0 * rel_ecef.z;
                let enu_up = cos_lat0 * cos_lon0 * rel_ecef.x + cos_lat0 * sin_lon0 * rel_ecef.y + sin_lat0 * rel_ecef.z;

                // Local Engine Cartesian Space (+X = East, +Y = Up, -Z = North) with vertical elevation offset
                total_positions.push(Vec3::new(enu_east as f32, (enu_up as f32) + height_offset, -enu_north as f32));

                // 2. Globe ECEF coordinate (for 3D Earth morphing) - in Engine ECEF (Y-Up: X, Z, -Y)
                total_ecef.push(Vec3::new(ecef_d.x as f32, ecef_d.z as f32, -ecef_d.y as f32));

                // 3. Normal vector transformed from glTF -> Standard ECEF -> Local ENU (+X=East, +Y=Up, -Z=North)
                let n = prim.normals.get(i).copied().unwrap_or(Vec3::Y);
                let zup_n = if is_z_up {
                    DVec3::new(n.x as f64, n.y as f64, n.z as f64)
                } else {
                    DVec3::new(n.x as f64, -(n.z as f64), n.y as f64)
                };
                let norm_ecef = accum_transform.transform_vector3(zup_n).normalize_or_zero();

                let norm_east = -sin_lon0 * norm_ecef.x + cos_lon0 * norm_ecef.y;
                let norm_north = -sin_lat0 * cos_lon0 * norm_ecef.x - sin_lat0 * sin_lon0 * norm_ecef.y + cos_lat0 * norm_ecef.z;
                let norm_up = cos_lat0 * cos_lon0 * norm_ecef.x + cos_lat0 * sin_lon0 * norm_ecef.y + sin_lat0 * norm_ecef.z;

                let local_norm = Vec3::new(norm_east as f32, norm_up as f32, -norm_north as f32).normalize_or_zero();
                total_normals.push(if local_norm.length_squared() > 0.1 { local_norm } else { Vec3::Y });

                // 4. UV
                let uv = prim.uvs.get(i).copied().unwrap_or(Vec2::ZERO);
                total_uvs.push(uv);

                // 5. Batch ID
                let b_id = prim.batch_ids.get(i).copied().unwrap_or(0);
                total_batch_ids.push(b_id);
            }

            for idx in &prim.indices {
                total_indices.push(index_offset + idx);
            }

            submeshes.push(DecodedThreeDTileSubMesh {
                index_offset: start_index,
                index_count: prim.indices.len() as u32,
                base_color: prim.base_color,
                image_rgba: prim.image_rgba.clone(),
            });
        }

        let mut bounds_min = Vec3::splat(f32::MAX);
        let mut bounds_max = Vec3::splat(f32::MIN);
        for p in &total_positions {
            bounds_min = bounds_min.min(*p);
            bounds_max = bounds_max.max(*p);
        }

        if total_positions.is_empty() {
            bounds_min = Vec3::ZERO;
            bounds_max = Vec3::ZERO;
        }

        // Segment into individual building features (via _BATCHID or Connected Component graph clustering)
        let mut features = Vec::new();
        let num_verts = total_positions.len();

        if num_verts > 0 && total_indices.len() >= 3 {
            let has_batch_ids = parsed.batch_length > 1 || parsed.model.primitives.iter().any(|p| p.batch_ids.iter().any(|&b| b > 0));

            if has_batch_ids {
                // 1. OGC 3D Tiles _BATCHID Partitioning
                let mut batch_tris: HashMap<u32, Vec<u32>> = HashMap::new();
                for chunk in total_indices.chunks_exact(3) {
                    let b0 = total_batch_ids.get(chunk[0] as usize).copied().unwrap_or(0);
                    batch_tris.entry(b0).or_default().extend_from_slice(chunk);
                }

                for (b_id, tri_indices) in batch_tris {
                    let mut f_min = Vec3::splat(f32::MAX);
                    let mut f_max = Vec3::splat(f32::MIN);
                    let mut used_map: HashMap<u32, u32> = HashMap::new();
                    let mut f_pos = Vec::new();
                    let mut f_norm = Vec::new();
                    let mut f_uv = Vec::new();
                    let mut vert_indices = Vec::new();
                    let mut remapped_indices = Vec::with_capacity(tri_indices.len());

                    for &old_idx in &tri_indices {
                        let new_idx = *used_map.entry(old_idx).or_insert_with(|| {
                            let n = f_pos.len() as u32;
                            let p = total_positions[old_idx as usize];
                            f_min = f_min.min(p);
                            f_max = f_max.max(p);
                            f_pos.push([p.x, p.y, p.z]);
                            f_norm.push(total_normals.get(old_idx as usize).map(|n| [n.x, n.y, n.z]).unwrap_or([0.0, 1.0, 0.0]));
                            f_uv.push(total_uvs.get(old_idx as usize).map(|u| [u.x, u.y]).unwrap_or([0.0, 0.0]));
                            vert_indices.push(old_idx);
                            n
                        });
                        remapped_indices.push(new_idx);
                    }

                    let center_local = (f_min + f_max) * 0.5;
                    let center_geo = origin.local_to_geo(center_local);
                    let height = (f_max.y - f_min.y).max(0.0);

                    let mut props = HashMap::new();
                    for (k, vals) in &parsed.batch_table_json {
                        if let Some(v) = vals.get(b_id as usize) {
                            props.insert(k.clone(), v.to_string().trim_matches('"').to_string());
                        }
                    }

                    let name = props.get("name").or_else(|| props.get("Name")).or_else(|| props.get("id")).cloned();
                    features.push(ThreeDTileFeatureMesh {
                        feature_id: b_id,
                        name,
                        vertex_indices: vert_indices,
                        raw_mesh: crate::gis::extrusion::RawMeshData {
                            positions: f_pos,
                            normals: f_norm,
                            uvs: f_uv,
                            colors: vec![base_color; remapped_indices.len()],
                            indices: remapped_indices,
                        },
                        aabb_min: f_min,
                        aabb_max: f_max,
                        center_geo,
                        height,
                        properties: props,
                    });
                }
            } else {
                // 2. Connected Component Disjoint Graph Clustering (for reality meshes / photogrammetry without _BATCHID)
                let mut parent: Vec<usize> = (0..num_verts).collect();
                fn find_root(parent: &mut [usize], mut i: usize) -> usize {
                    let mut root = i;
                    while root != parent[root] {
                        root = parent[root];
                    }
                    while i != root {
                        let next = parent[i];
                        parent[i] = root;
                        i = next;
                    }
                    root
                }

                fn union_verts(parent: &mut [usize], i: usize, j: usize) {
                    let r1 = find_root(parent, i);
                    let r2 = find_root(parent, j);
                    if r1 != r2 {
                        parent[r1] = r2;
                    }
                }

                // A. Spatial position welding (unify vertices sharing identical XYZ positions across hard normals / UV seams)
                let mut pos_map: HashMap<(i64, i64, i64), usize> = HashMap::new();
                for (i, p) in total_positions.iter().enumerate() {
                    // Quantize to 2mm tolerance (0.002m)
                    let key = (
                        (p.x * 500.0).round() as i64,
                        (p.y * 500.0).round() as i64,
                        (p.z * 500.0).round() as i64,
                    );
                    if let Some(&first_idx) = pos_map.get(&key) {
                        union_verts(&mut parent, i, first_idx);
                    } else {
                        pos_map.insert(key, i);
                    }
                }

                // B. Triangle edge connectivity (connect all vertices within each triangle)
                for chunk in total_indices.chunks_exact(3) {
                    union_verts(&mut parent, chunk[0] as usize, chunk[1] as usize);
                    union_verts(&mut parent, chunk[1] as usize, chunk[2] as usize);
                }

                let mut comp_tris: HashMap<usize, Vec<u32>> = HashMap::new();
                for chunk in total_indices.chunks_exact(3) {
                    let root = find_root(&mut parent, chunk[0] as usize);
                    let tris = comp_tris.entry(root).or_default();
                    tris.push(chunk[0]);
                    tris.push(chunk[1]);
                    tris.push(chunk[2]);
                }

                let mut comp_list: Vec<(usize, Vec<u32>)> = comp_tris.into_iter().collect();
                // Sort largest building component first
                comp_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

                for (comp_idx, (_, tri_indices)) in comp_list.into_iter().enumerate() {
                    if tri_indices.len() < 3 {
                        continue;
                    }
                    let mut f_min = Vec3::splat(f32::MAX);
                    let mut f_max = Vec3::splat(f32::MIN);
                    let mut used_map: HashMap<u32, u32> = HashMap::new();
                    let mut f_pos = Vec::new();
                    let mut f_norm = Vec::new();
                    let mut f_uv = Vec::new();
                    let mut vert_indices = Vec::new();
                    let mut remapped_indices = Vec::with_capacity(tri_indices.len());

                    for &old_idx in &tri_indices {
                        let new_idx = *used_map.entry(old_idx).or_insert_with(|| {
                            let n = f_pos.len() as u32;
                            let p = total_positions[old_idx as usize];
                            f_min = f_min.min(p);
                            f_max = f_max.max(p);
                            f_pos.push([p.x, p.y, p.z]);
                            f_norm.push(total_normals.get(old_idx as usize).map(|n| [n.x, n.y, n.z]).unwrap_or([0.0, 1.0, 0.0]));
                            f_uv.push(total_uvs.get(old_idx as usize).map(|u| [u.x, u.y]).unwrap_or([0.0, 0.0]));
                            vert_indices.push(old_idx);
                            n
                        });
                        remapped_indices.push(new_idx);
                    }

                    let height = (f_max.y - f_min.y).max(0.0);
                    let span_x = (f_max.x - f_min.x).abs();
                    let span_z = (f_max.z - f_min.z).abs();

                    // Filter out microscopic slivers / floating degenerate noise (< 0.2m)
                    if height < 0.2 && span_x < 0.2 && span_z < 0.2 && remapped_indices.len() < 12 {
                        continue;
                    }

                    let center_local = (f_min + f_max) * 0.5;
                    let center_geo = origin.local_to_geo(center_local);

                    features.push(ThreeDTileFeatureMesh {
                        feature_id: comp_idx as u32,
                        name: Some(format!("Building #{}", comp_idx + 1)),
                        vertex_indices: vert_indices,
                        raw_mesh: crate::gis::extrusion::RawMeshData {
                            positions: f_pos,
                            normals: f_norm,
                            uvs: f_uv,
                            colors: vec![base_color; remapped_indices.len()],
                            indices: remapped_indices,
                        },
                        aabb_min: f_min,
                        aabb_max: f_max,
                        center_geo,
                        height,
                        properties: HashMap::new(),
                    });
                }
            }
        }

        Ok(DecodedThreeDTileMesh {
            id: id.to_string(),
            positions: total_positions,
            normals: total_normals,
            uvs: total_uvs,
            ecef_positions: total_ecef,
            indices: total_indices,
            base_color,
            bounds_min,
            bounds_max,
            image_rgba,
            submeshes,
            batch_table: parsed.batch_table_json,
            features,
        })
    }

    /// Evaluates which 3D tiles to draw and which to fetch based on camera view and SSE LOD
    pub fn update_streaming(
        &mut self,
        origin: &ProjectOrigin,
        camera: &Camera,
        _screen_w: f32,
        screen_h: f32,
        projection_mode: crate::gis::crs::ProjectionMode,
    ) -> (Vec<String>, Vec<DecodedThreeDTileMesh>) {
        let mut newly_loaded = Vec::new();

        // 0. If origin has changed, immediately reproject all cached in-memory tiles to the new origin
        if self.current_origin.as_ref() != Some(origin) {
            let reprojected = self.reproject_all(origin);
            newly_loaded.extend(reprojected);
        }

        // 1. Drain responses from background worker
        if let Ok(rx) = self.rx_resp.lock() {
            while let Ok(resp) = rx.try_recv() {
                match resp {
                    TileResponse::TilesetLoaded { tileset } => {
                        self.is_loading = false;
                        match tileset {
                            Ok(ts) => {
                                self.status_message = format!("Tileset active (v{})", ts.asset.version);
                                self.tileset = Some(ts);
                            }
                            Err(e) => {
                                self.status_message = format!("Tileset load error: {}", e);
                            }
                        }
                    }
                    TileResponse::ContentLoaded { id, result } => {
                        self.pending_requests.remove(&id);
                        match result {
                            Ok(mesh) => {
                                newly_loaded.push(mesh.clone());
                                self.loaded_tiles.insert(id, mesh);
                            }
                            Err(e) => {
                                eprintln!("Failed to decode 3D tile {}: {}", id, e);
                            }
                        }
                    }
                }
            }
        }

        let mut active_to_draw = Vec::new();
        let tileset = match &self.tileset {
            Some(ts) => ts,
            None => return (active_to_draw, newly_loaded),
        };

        // Determine camera position in Standard WGS84 ECEF
        let cam_eye_ecef = if projection_mode == crate::gis::crs::ProjectionMode::GlobeECEF {
            let eye = camera.eye_position();
            // Engine ECEF (X, Z, -Y) -> Standard WGS84 ECEF (X, -Z_engine, Y_engine)
            DVec3::new(eye.x as f64, -eye.z as f64, eye.y as f64)
        } else {
            let cam_eye_enu = camera.eye_position();
            let cam_eye_geo = origin.local_to_geo(cam_eye_enu);
            lat_lon_alt_to_standard_ecef(
                cam_eye_geo.latitude,
                cam_eye_geo.longitude,
                cam_eye_geo.elevation,
            )
        };

        // 2. Traverse tree from root node (starting with IDENTITY parent transform so root.transform is applied once)
        let initial_transform = DMat4::IDENTITY;

        let mut tiles_to_request = Vec::new();
        Self::traverse_node(
            &tileset.root,
            &initial_transform,
            &cam_eye_ecef,
            screen_h,
            camera.fov_y,
            self.maximum_screen_space_error,
            &self.loaded_tiles,
            &mut active_to_draw,
            &mut tiles_to_request,
            &self.base_url,
        );

        let gltf_up_axis = self.tileset.as_ref().and_then(|ts| ts.asset.gltf_up_axis.clone());

        // 3. Dispatch new tile requests
        for (tile_id, full_url, accum_trans) in tiles_to_request {
            if !self.pending_requests.contains(&tile_id) && !self.loaded_tiles.contains_key(&tile_id) {
                self.pending_requests.insert(tile_id.clone());
                #[cfg(not(target_arch = "wasm32"))]
                let _ = self.tx_req.send(TileRequest::FetchContent {
                    id: tile_id,
                    full_url,
                    accum_transform: accum_trans,
                    origin: *origin,
                    gltf_up_axis: gltf_up_axis.clone(),
                    height_offset: self.height_offset,
                });
                #[cfg(target_arch = "wasm32")]
                {
                    let tx = self.tx_resp.clone();
                    let id = tile_id;
                    let origin_val = *origin;
                    let gltf_axis = gltf_up_axis.clone();
                    let h_offset = self.height_offset;
                    crate::gis::platform::http::fetch_bytes(&full_url, move |res| {
                        let mesh_res = res.and_then(|bytes| {
                            let parsed = B3dmParser::parse(&bytes)?;
                            Self::convert_parsed_to_mesh(&id, parsed, &accum_trans, &origin_val, gltf_axis.as_deref(), h_offset)
                        });
                        let _ = tx.send(TileResponse::ContentLoaded { id, result: mesh_res });
                    });
                }
            }
        }

        if !self.pending_requests.is_empty() {
            self.is_loading = true;
            self.status_message = format!("Loading {} tiles ({} ready)", self.pending_requests.len(), self.loaded_tiles.len());
        } else if self.is_loading {
            self.is_loading = false;
            self.status_message = format!("Active ({} tiles loaded)", self.loaded_tiles.len());
        }

        (active_to_draw, newly_loaded)
    }

    fn traverse_node(
        node: &Tile3DNode,
        parent_transform: &DMat4,
        cam_eye_ecef: &DVec3,
        screen_h: f32,
        fov_y: f32,
        max_sse: f32,
        loaded_tiles: &HashMap<String, DecodedThreeDTileMesh>,
        active_to_draw: &mut Vec<String>,
        tiles_to_request: &mut Vec<(String, String, DMat4)>,
        base_url: &str,
    ) {
        let current_transform = if let Some(t) = &node.transform {
            *parent_transform * DMat4::from_cols_array(t)
        } else {
            *parent_transform
        };

        let center_ecef = get_bounding_volume_center_ecef(&node.bounding_volume, Some(&current_transform));
        let radius = get_bounding_volume_radius(&node.bounding_volume, Some(&current_transform));
        let dist = (cam_eye_ecef.distance(center_ecef) - radius).max(1.0);

        let sse = calculate_screen_space_error(node.geometric_error, dist, screen_h, fov_y);

        let has_content = node.content.as_ref().and_then(|c| c.content_uri()).is_some();
        let content_id = node.content.as_ref().and_then(|c| c.content_uri()).unwrap_or("");
        let is_content_loaded = loaded_tiles.contains_key(content_id);

        let should_refine = (sse > max_sse || !has_content) && !node.children.is_empty();

        if should_refine {
            if node.refine == RefinementMode::Add && has_content {
                if is_content_loaded {
                    active_to_draw.push(content_id.to_string());
                } else if !content_id.is_empty() {
                    let full_url = format!("{}{}", base_url, content_id);
                    tiles_to_request.push((content_id.to_string(), full_url, current_transform));
                }
            }

            for child in &node.children {
                Self::traverse_node(
                    child,
                    &current_transform,
                    cam_eye_ecef,
                    screen_h,
                    fov_y,
                    max_sse,
                    loaded_tiles,
                    active_to_draw,
                    tiles_to_request,
                    base_url,
                );
            }
        } else if has_content {
            if is_content_loaded {
                active_to_draw.push(content_id.to_string());
            } else if !content_id.is_empty() {
                let full_url = format!("{}{}", base_url, content_id);
                tiles_to_request.push((content_id.to_string(), full_url, current_transform));
            }
        }
    }
}

