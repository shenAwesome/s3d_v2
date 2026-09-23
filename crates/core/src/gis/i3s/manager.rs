use crate::gis::crs::ProjectOrigin;
use crate::gis::extrusion::RawMeshData;
use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Condvar, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
use web_time::Instant;
use super::spec::*;
use super::decoder::*;

#[derive(Debug, Clone)]
pub struct I3SWorkTask {
    pub node_id: u32,
    pub geom_url: String,
    pub texture_info: Option<(String, u32)>,
    pub geom_info: I3SGeometryInfo,
    pub geom_buf_def: Option<I3SGeometryBufferDef>,
    pub obb_center: [f64; 3],
    pub base_color: [f32; 4],
}

pub struct I3SWorkQueue {
    pub pending_geometries: Vec<I3SWorkTask>,
    pub in_flight: usize,
    pub is_shutdown: bool,
}

pub struct I3SManager {
    pub service_url: String,
    pub is_enabled: bool,
    pub opacity: f32,
    pub lod_threshold_scale: f32,

    pub layer_metadata: Option<I3SSceneLayer>,
    pub node_cache: HashMap<u32, I3SNode>,
    pub cached_pages: HashSet<u32>,
    pub pending_pages: HashSet<u32>,

    pub requested_node_ids: HashSet<u32>,
    pub loaded_node_ids: HashSet<u32>,
    pub raw_meshes: HashMap<u32, RawMeshData>,
    pub raw_features: HashMap<String, I3SFeatureMesh>,

    /// Shared scene origin passed to worker threads for ENU coordinate decoding.
    /// Updated via `set_origin()` whenever the scene origin changes.
    origin: Arc<Mutex<ProjectOrigin>>,

    // Threading
    work_queue: Arc<Mutex<I3SWorkQueue>>,
    work_condvar: Arc<Condvar>,
    result_receiver: Mutex<Receiver<I3SDownloadResult>>,
    result_sender: std::sync::mpsc::Sender<I3SDownloadResult>,

    pub active_nodes_count: usize,
    pub is_loading_metadata: bool,
    pub server_symbol: Option<crate::gis::i3s::spec::I3SServerSymbol>,
    pub has_new_server_symbol: bool,
    pub nodes_to_request: HashSet<u32>,

    // Throttling & camera motion tracking for LOD recalculation
    pub last_calc_instant: Option<Instant>,
    pub last_camera_eye: glam::Vec3,
    pub last_camera_pitch: f32,
    pub last_camera_yaw: f32,
    pub cached_visible_nodes: Vec<u32>,
    pub last_loaded_count: usize,
}

impl I3SManager {
    pub const MAX_CONCURRENT_WORKERS: usize = 12;
    pub const MAX_PENDING_QUEUE: usize = 512;

    pub fn new(initial_origin: ProjectOrigin) -> Self {
        let work_queue = Arc::new(Mutex::new(I3SWorkQueue {
            pending_geometries: Vec::new(),
            in_flight: 0,
            is_shutdown: false,
        }));
        let work_condvar = Arc::new(Condvar::new());
        let (result_sender, result_receiver) = channel::<I3SDownloadResult>();

        // Shared origin so workers decode geometry relative to the actual scene origin,
        // not the hardcoded Melbourne CBD fallback.
        let shared_origin = Arc::new(Mutex::new(initial_origin));

        // Default to Melbourne CBD 3D Buildings preset
        let default_preset = &I3S_PRESETS[0];

        let manager = Self {
            service_url: default_preset.url.to_string(),
            is_enabled: false,
            opacity: 1.0,
            lod_threshold_scale: 1.0,
            layer_metadata: None,
            node_cache: HashMap::new(),
            cached_pages: HashSet::new(),
            pending_pages: HashSet::new(),
            requested_node_ids: HashSet::new(),
            loaded_node_ids: HashSet::new(),
            raw_meshes: HashMap::new(),
            raw_features: HashMap::new(),
            origin: shared_origin.clone(),
            work_queue: work_queue.clone(),
            work_condvar: work_condvar.clone(),
            result_receiver: Mutex::new(result_receiver),
            result_sender: result_sender.clone(),
            active_nodes_count: 0,
            is_loading_metadata: false,
            server_symbol: None,
            has_new_server_symbol: false,
            nodes_to_request: HashSet::new(),
            last_calc_instant: None,
            last_camera_eye: glam::Vec3::ZERO,
            last_camera_pitch: 0.0,
            last_camera_yaw: 0.0,
            cached_visible_nodes: Vec::new(),
            last_loaded_count: 0,
        };

        #[cfg(not(target_arch = "wasm32"))]
        // Spawn background worker threads
        for _ in 0..Self::MAX_CONCURRENT_WORKERS {
            let queue_arc = work_queue.clone();
            let cond_arc = work_condvar.clone();
            let tx = result_sender.clone();
            let origin_arc = shared_origin.clone();

            thread::spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(std::time::Duration::from_secs(5))
                    .timeout_read(std::time::Duration::from_secs(8))
                    .user_agent("s3d-i3s/0.1.0 (Antigravity 3D GIS)")
                    .build();

                loop {
                    let task = {
                        let mut guard = match queue_arc.lock() {
                            Ok(g) => g,
                            Err(_) => break,
                        };

                        loop {
                            if guard.is_shutdown {
                                return;
                            }

                            if let Some(task) = guard.pending_geometries.pop() {
                                guard.in_flight += 1;
                                break task;
                            } else {
                                guard = match cond_arc.wait(guard) {
                                    Ok(g) => g,
                                    Err(_) => return,
                                };
                            }
                        }
                    };

                    let I3SWorkTask {
                        node_id,
                        geom_url,
                        texture_info,
                        geom_info,
                        geom_buf_def,
                        obb_center,
                        base_color,
                    } = task;

                    // Read the current scene origin (set by the main thread via set_origin)
                    let origin = match origin_arc.lock() {
                        Ok(o) => o.clone(),
                        Err(_) => break,
                    };

                    let service_slug = crate::gis::cache::DiskCacheManager::service_to_slug(&geom_url);
                    let mut bytes = crate::gis::cache::DiskCacheManager::read_i3s_geometry(&service_slug, geom_info.resource);

                    if bytes.is_none() {
                        if let Ok(resp) = agent.get(&geom_url).call() {
                            let mut downloaded = Vec::new();
                            if resp.into_reader().read_to_end(&mut downloaded).is_ok() {
                                crate::gis::cache::DiskCacheManager::write_i3s_geometry(&service_slug, geom_info.resource, &downloaded);
                                bytes = Some(downloaded);
                            }
                        }
                    }

                    // Optional texture downloading & decoding
                    let mut image_rgba = None;
                    if let Some((ref tex_url, tex_res_id)) = texture_info {
                        let mut tex_bytes = crate::gis::cache::DiskCacheManager::read_i3s_texture(&service_slug, tex_res_id);
                        if tex_bytes.is_none() {
                            if let Ok(resp) = agent.get(tex_url).call() {
                                let mut downloaded = Vec::new();
                                if resp.into_reader().read_to_end(&mut downloaded).is_ok() {
                                    crate::gis::cache::DiskCacheManager::write_i3s_texture(&service_slug, tex_res_id, &downloaded);
                                    tex_bytes = Some(downloaded);
                                }
                            }
                        }
                        if let Some(tb) = tex_bytes {
                            if let Ok(dyn_img) = image::load_from_memory(&tb) {
                                let rgba = dyn_img.to_rgba8();
                                image_rgba = Some(Arc::new((rgba.width(), rgba.height(), rgba.into_raw())));
                            }
                        }
                    }

                    let mut success = false;
                    if let Some(bytes) = bytes {
                        let is_wgs84 = obb_center[0].abs() <= 180.0 && obb_center[1].abs() <= 90.0;
                        if let Ok(mut decoded) = I3SGeometryDecoder::decode(
                            &bytes,
                            &geom_info,
                            geom_buf_def.as_ref(),
                            obb_center,
                            is_wgs84,
                            &origin,
                            base_color,
                        ) {
                            decoded.node_id = node_id;
                            decoded.image_rgba = image_rgba;
                            let _ = tx.send(I3SDownloadResult::Geometry(Box::new(decoded)));
                            success = true;
                        }
                    }

                    if !success {
                        let _ = tx.send(I3SDownloadResult::Failure(Some(node_id), format!("Failed to load node {}", node_id)));
                    }

                    if let Ok(mut guard) = queue_arc.lock() {
                        guard.in_flight = guard.in_flight.saturating_sub(1);
                    }
                }
            });
        }

        manager
    }

    /// Unmarks a node from the loaded and requested caches if evicted from GPU memory
    pub fn unmark_loaded(&mut self, node_id: u32) {
        self.loaded_node_ids.remove(&node_id);
        self.requested_node_ids.remove(&node_id);
        self.raw_meshes.remove(&node_id);
        self.raw_features.retain(|_, feat| feat.node_id != node_id);
    }

    /// Returns true if metadata, node pages, or 3D tile geometries are actively downloading in background
    pub fn is_streaming(&self) -> bool {
        if !self.is_enabled {
            return false;
        }
        if self.is_loading_metadata || !self.pending_pages.is_empty() {
            return true;
        }
        if let Ok(q) = self.work_queue.lock() {
            q.in_flight > 0 || !q.pending_geometries.is_empty()
        } else {
            false
        }
    }

    /// Returns the number of currently active / in-flight or queued download tasks
    pub fn pending_task_count(&self) -> usize {
        if !self.is_enabled {
            return 0;
        }
        let queue_count = self.work_queue.lock().map(|q| q.in_flight + q.pending_geometries.len()).unwrap_or(0);
        let mut count = queue_count + self.pending_pages.len();
        if self.is_loading_metadata { count += 1; }
        count
    }

    /// Returns the current origin used by worker threads
    pub fn origin(&self) -> Option<ProjectOrigin> {
        self.origin.lock().ok().map(|o| *o)
    }

    /// Update the scene origin used by all background worker threads for ENU decoding.
    /// Call this whenever the scene origin changes (e.g. when loading a new project).
    pub fn set_origin(&mut self, origin: ProjectOrigin) {
        let changed = if let Ok(mut o) = self.origin.lock() {
            if *o != origin {
                *o = origin;
                true
            } else {
                false
            }
        } else {
            false
        };

        if changed {
            self.clear_loaded_tiles();
        }
    }

    /// Clears only loaded tile meshes and pending geometry download tasks when origin shifts,
    /// preserving layer metadata and nodepage hierarchy cache.
    pub fn clear_loaded_tiles(&mut self) {
        self.requested_node_ids.clear();
        self.loaded_node_ids.clear();
        self.raw_meshes.clear();
        self.raw_features.clear();
        self.active_nodes_count = 0;
        self.nodes_to_request.clear();

        if let Ok(mut q) = self.work_queue.lock() {
            q.pending_geometries.clear();
            q.in_flight = 0;
        }

        self.last_calc_instant = None;
    }

    /// Clear all streaming state (node cache, request tracking, GPU-upload queue).
    /// Call this when toggling the layer off/on.
    pub fn clear_streaming_state(&mut self) {
        self.node_cache.clear();
        self.cached_pages.clear();
        self.pending_pages.clear();
        self.clear_loaded_tiles();

        // Drain any stale results from the channel
        if let Ok(rx) = self.result_receiver.lock() {
            while rx.try_recv().is_ok() {}
        }
    }

    /// Set service URL and trigger metadata loading
    pub fn set_service_url(&mut self, url: &str) {
        let mut clean_url = url.trim().trim_end_matches('/').to_string();
        if clean_url.ends_with("/layers/0") {
            clean_url = clean_url[..clean_url.len() - "/layers/0".len()].trim_end_matches('/').to_string();
        }
        if clean_url == self.service_url && self.layer_metadata.is_some() {
            return;
        }

        self.service_url = clean_url;
        self.reset_state();
        self.fetch_metadata();
    }

    /// Reset internal cache when service URL changes
    pub fn reset_state(&mut self) {
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.pending_geometries.clear();
        }
        self.layer_metadata = None;
        self.node_cache.clear();
        self.cached_pages.clear();
        self.pending_pages.clear();
        self.requested_node_ids.clear();
        self.loaded_node_ids.clear();
        self.raw_meshes.clear();
        self.active_nodes_count = 0;
        self.nodes_to_request.clear();
        self.server_symbol = None;
        self.has_new_server_symbol = false;
    }

    /// Triggers asynchronous metadata fetch for layer 0
    pub fn fetch_metadata(&mut self) {
        if self.is_loading_metadata {
            return;
        }

        self.is_loading_metadata = true;
        let url = format!("{}/layers/0?f=json", self.service_url);
        let tx = self.result_sender.clone();

        #[cfg(not(target_arch = "wasm32"))]
        thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(5))
                .timeout_read(std::time::Duration::from_secs(8))
                .user_agent("s3d-i3s/0.1.0 (Antigravity 3D GIS)")
                .build();

            if let Ok(resp) = agent.get(&url).call() {
                if let Ok(json_str) = resp.into_string() {
                    if let Ok(layer) = serde_json::from_str::<I3SSceneLayer>(&json_str) {
                        let _ = tx.send(I3SDownloadResult::Metadata(Box::new(layer)));
                        return;
                    }
                }
            }
            let _ = tx.send(I3SDownloadResult::Failure(None, "Failed to load I3S layer metadata".to_string()));
        });

        #[cfg(target_arch = "wasm32")]
        crate::gis::platform::http::fetch_text(&url, move |res| {
            if let Ok(json_str) = res {
                if let Ok(layer) = serde_json::from_str::<I3SSceneLayer>(&json_str) {
                    let _ = tx.send(I3SDownloadResult::Metadata(Box::new(layer)));
                    return;
                }
            }
            let _ = tx.send(I3SDownloadResult::Failure(None, "Failed to load I3S layer metadata".to_string()));
        });
    }

    /// Triggers asynchronous fetch of a specific node page
    pub fn fetch_node_page(&mut self, page_id: u32) {
        if self.cached_pages.contains(&page_id) {
            return;
        }
        self.cached_pages.insert(page_id);
        self.pending_pages.insert(page_id);

        let service_slug = crate::gis::cache::DiskCacheManager::service_to_slug(&self.service_url);
        // Fast-path: read nodepage JSON directly from disk cache
        if let Some(cached_json) = crate::gis::cache::DiskCacheManager::read_i3s_nodepage(&service_slug, page_id) {
            if let Ok(page) = serde_json::from_str::<I3SNodePage>(&cached_json) {
                let _ = self.result_sender.send(I3SDownloadResult::NodePage(page_id, Box::new(page)));
                return;
            }
        }

        let url = format!("{}/layers/0/nodepages/{}?f=json", self.service_url, page_id);
        let tx = self.result_sender.clone();
        let slug_clone = service_slug.clone();

        #[cfg(not(target_arch = "wasm32"))]
        thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(5))
                .timeout_read(std::time::Duration::from_secs(8))
                .user_agent("s3d-i3s/0.1.0 (Antigravity 3D GIS)")
                .build();

            if let Ok(resp) = agent.get(&url).call() {
                if let Ok(json_str) = resp.into_string() {
                    crate::gis::cache::DiskCacheManager::write_i3s_nodepage(&slug_clone, page_id, &json_str);
                    if let Ok(page) = serde_json::from_str::<I3SNodePage>(&json_str) {
                        let _ = tx.send(I3SDownloadResult::NodePage(page_id, Box::new(page)));
                        return;
                    }
                }
            }
            let _ = tx.send(I3SDownloadResult::PageFailure(page_id, format!("Failed to load nodepage {}", page_id)));
        });

        #[cfg(target_arch = "wasm32")]
        crate::gis::platform::http::fetch_text(&url, move |res| {
            if let Ok(json_str) = res {
                crate::gis::cache::DiskCacheManager::write_i3s_nodepage(&slug_clone, page_id, &json_str);
                if let Ok(page) = serde_json::from_str::<I3SNodePage>(&json_str) {
                    let _ = tx.send(I3SDownloadResult::NodePage(page_id, Box::new(page)));
                    return;
                }
            }
            let _ = tx.send(I3SDownloadResult::PageFailure(page_id, format!("Failed to load nodepage {}", page_id)));
        });
    }

    /// Evaluates hierarchical Screen-Space Error (SSE) to automatically determine visible I3S nodes
    /// following the OGC I3S 1.7 / 1.8 standard specification.
    pub fn calculate_visible_nodes(
        &mut self,
        camera: &crate::renderer::camera::Camera,
        viewport_width: f32,
        viewport_height: f32,
        origin: &ProjectOrigin,
    ) -> Vec<u32> {
        if !self.is_enabled || self.layer_metadata.is_none() {
            return Vec::new();
        }

        // Keep internal worker origin in sync with the current active scene origin.
        // If the scene origin changed (e.g. from local navigation, project load, etc.),
        // clear streaming state and reset cache so worker threads re-decode relative to the new origin.
        let origin_changed = if let Ok(mut o) = self.origin.lock() {
            if *o != *origin {
                *o = *origin;
                true
            } else {
                false
            }
        } else {
            false
        };

        if origin_changed {
            self.clear_loaded_tiles();
            return Vec::new();
        }

        let camera_eye = camera.eye_position();
        let pitch = camera.pitch;
        let yaw = camera.yaw;
        let now = Instant::now();
        let loaded_count = self.loaded_node_ids.len();

        // ── Throttled LOD Recalculation ──────────────────────────────────────
        // Re-evaluate LOD at most once every 150ms during continuous camera motion.
        // If camera is stationary and no new tiles finished loading, return immediately.
        if let Some(last_time) = self.last_calc_instant {
            let elapsed_ms = now.duration_since(last_time).as_millis();
            let moved = (camera_eye - self.last_camera_eye).length_squared() > 1.0
                || (pitch - self.last_camera_pitch).abs() > 0.02
                || (yaw - self.last_camera_yaw).abs() > 0.02;
            let loaded_changed = loaded_count != self.last_loaded_count;

            if !loaded_changed && !self.cached_visible_nodes.is_empty() {
                if !moved {
                    // Camera completely stationary and tiles didn't change -> return cached
                    return self.cached_visible_nodes.clone();
                } else if elapsed_ms < 150 {
                    // Continuous camera movement throttle
                    return self.cached_visible_nodes.clone();
                }
            }
        }

        let aspect = (viewport_width / viewport_height.max(1.0)).max(0.1);
        let vp = camera.view_proj_matrix(aspect);
        let frustum = crate::renderer::camera::Frustum::from_view_proj(vp);
        let fov_rad = camera.fov_y;
        let focal_length = viewport_height / (2.0 * (fov_rad * 0.5).tan());

        let nodes_per_page = self
            .layer_metadata
            .as_ref()
            .and_then(|m| m.node_pages.as_ref())
            .and_then(|np| np.nodes_per_page)
            .unwrap_or(64);

        let metric_type = self
            .layer_metadata
            .as_ref()
            .and_then(|m| m.node_pages.as_ref())
            .and_then(|np| np.lod_selection_metric_type.clone())
            .unwrap_or_else(|| "maxScreenThresholdSQ".to_string());

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut raw_selected = Vec::new();

        queue.push_back(0);

        while let Some(node_id) = queue.pop_front() {
            if visited.contains(&node_id) {
                continue;
            }
            visited.insert(node_id);

            let page_id = node_id / nodes_per_page;
            if !self.cached_pages.contains(&page_id) {
                self.fetch_node_page(page_id);
            }

            let (is_in_frustum, center_enu, radius, lod_threshold, has_children, has_mesh, children) = {
                let node = match self.node_cache.get(&node_id) {
                    Some(n) => n,
                    None => continue,
                };

                let (in_f, c_enu, rad) = if let Some(obb) = &node.obb {
                    let (center_enu, axes, half_size) = obb.to_engine_obb(origin);
                    let radius = obb.radius() as f32;

                    // Near-field frustum culling bypass:
                    // If camera is close to or inside the bounding sphere (dist <= radius * 1.25),
                    // near-plane clipping must NOT cull the node, preventing building facades from disappearing!
                    let dist_to_eye = (center_enu - camera_eye).length();
                    let in_frustum = node.index == 0
                        || dist_to_eye <= radius * 1.25
                        || (frustum.intersects_sphere(center_enu, radius) && frustum.intersects_obb(center_enu, axes, half_size));

                    (in_frustum, center_enu, radius)
                } else {
                    (true, Vec3::ZERO, 50.0)
                };

                let lod_t = node.lod_threshold.unwrap_or(0.0) * self.lod_threshold_scale as f64;
                let has_c = node.children.as_ref().map(|c| !c.is_empty()).unwrap_or(false);
                let has_m = node.mesh.is_some();
                let ch = node.children.clone();
                (in_f, c_enu, rad, lod_t, has_c, has_m, ch)
            };

            if !is_in_frustum {
                continue;
            }

            let dist = (center_enu - camera_eye).length().max(1.0);
            let radius_px = (radius / dist) * focal_length;

            let wants_split = if !has_mesh && has_children {
                true
            } else if has_children {
                if dist <= radius {
                    true
                } else if lod_threshold <= 0.0 {
                    true
                } else {
                    match metric_type.as_str() {
                        "maxScreenThresholdSQ" | _ if metric_type.ends_with("SQ") => {
                            let screen_area = std::f64::consts::PI * (radius_px as f64 * radius_px as f64);
                            screen_area > lod_threshold
                        }
                        "maxScreenThreshold" => {
                            let diameter_px = 2.0 * radius_px as f64;
                            diameter_px > lod_threshold
                        }
                        "screenSpaceError" => {
                            let sse = radius_px as f64;
                            sse > lod_threshold.max(1.0)
                        }
                        _ => {
                            let screen_area = std::f64::consts::PI * (radius_px as f64 * radius_px as f64);
                            screen_area > lod_threshold
                        }
                    }
                }
            } else {
                false
            };

            if wants_split && has_children {
                if let Some(ref children_list) = children {
                    let mut all_children_ready = true;
                    let mut in_frustum_children_count = 0;

                    for &child_id in children_list {
                        let child_in_frustum = if let Some(cn) = self.node_cache.get(&child_id) {
                            if let Some(obb) = &cn.obb {
                                let (c_enu, axes, half_size) = obb.to_engine_obb(origin);
                                let r = obb.radius() as f32;
                                let c_dist = (c_enu - camera_eye).length();
                                c_dist <= r * 1.25
                                    || (frustum.intersects_sphere(c_enu, r) && frustum.intersects_obb(c_enu, axes, half_size))
                            } else {
                                true
                            }
                        } else {
                            true
                        };

                        if child_in_frustum {
                            in_frustum_children_count += 1;
                            let child_page = child_id / nodes_per_page;
                            if !self.cached_pages.contains(&child_page) {
                                self.fetch_node_page(child_page);
                            }

                            // Check if child node has loaded its mesh (if it has one)
                            let child_ready = if let Some(cn) = self.node_cache.get(&child_id) {
                                if cn.mesh.is_some() {
                                    self.loaded_node_ids.contains(&child_id)
                                } else {
                                    true
                                }
                            } else {
                                false
                            };

                            if !child_ready {
                                all_children_ready = false;
                            }

                            queue.push_back(child_id);
                        }
                    }

                    // HLOD Fallback: If this parent node has a mesh already in GPU memory,
                    // and its refined children are still streaming, keep the parent drawn
                    // to prevent buildings from disappearing!
                    if has_mesh && self.loaded_node_ids.contains(&node_id) && (!all_children_ready || in_frustum_children_count == 0) {
                        raw_selected.push(node_id);
                    }
                }
            } else if has_mesh {
                // Target leaf / required LOD node
                raw_selected.push(node_id);
            } else if has_children {
                // Node has no mesh and doesn't want to split -> traverse children to find visible geometry
                if let Some(ref children_list) = children {
                    for &child_id in children_list {
                        let child_page = child_id / nodes_per_page;
                        if !self.cached_pages.contains(&child_page) {
                            self.fetch_node_page(child_page);
                        }
                        queue.push_back(child_id);
                    }
                }
            }
        }

        let mut visible_nodes: Vec<u32> = raw_selected;
        visible_nodes.sort_unstable();
        visible_nodes.dedup();

        self.last_calc_instant = Some(now);
        self.last_camera_eye = camera_eye;
        self.last_camera_pitch = pitch;
        self.last_camera_yaw = yaw;
        self.cached_visible_nodes = visible_nodes.clone();
        self.last_loaded_count = loaded_count;
        self.active_nodes_count = visible_nodes.len();
        visible_nodes
    }

    /// Enqueues geometry download tasks for visible nodes and required refined children
    pub fn request_nodes(&mut self, node_ids: &[u32]) {
        if !self.is_enabled || self.layer_metadata.is_none() {
            return;
        }

        let geom_defs = match self.layer_metadata.as_ref().and_then(|m| m.geometry_definitions.as_ref()) {
            Some(g) => g,
            None => return,
        };

        let mut queue = match self.work_queue.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // All nodes needed: visible placeholder/leaf nodes plus in-frustum children being loaded
        let mut needed: HashSet<u32> = node_ids.iter().copied().collect();
        needed.extend(&self.nodes_to_request);

        if needed.is_empty() {
            return;
        }

        // Prune out-of-view tasks from work queue
        queue.pending_geometries.retain(|task| needed.contains(&task.node_id));

        // Keep requested_node_ids in sync so pruned tasks can be re-requested when camera returns
        self.requested_node_ids.retain(|id| self.loaded_node_ids.contains(id) || needed.contains(id));

        let available_slots = Self::MAX_PENDING_QUEUE.saturating_sub(queue.pending_geometries.len());
        let mut newly_enqueued = 0;

        let has_textures = self.layer_metadata.as_ref()
            .and_then(|m| m.texture_set_definitions.as_ref())
            .map(|defs| !defs.is_empty())
            .unwrap_or(false);

        for &node_id in &needed {
            if newly_enqueued >= available_slots {
                break;
            }

            if !self.requested_node_ids.contains(&node_id) && !self.loaded_node_ids.contains(&node_id) {
                if let Some(node) = self.node_cache.get(&node_id) {
                    if let Some(mesh) = &node.mesh {
                        if let Some(geom_info) = &mesh.geometry {
                            if let Some(obb) = &node.obb {
                                let geom_buf_def = geom_defs
                                    .get(geom_info.definition)
                                    .and_then(|d| d.geometry_buffers.first())
                                    .cloned();

                                let url = format!("{}/layers/0/nodes/{}/geometries/0", self.service_url, geom_info.resource);
                                let base_color = self.layer_metadata.as_ref()
                                    .and_then(|m| m.material_definitions.as_ref())
                                    .and_then(|defs| {
                                        let mat_idx = mesh.material.as_ref().map(|mat| mat.definition).unwrap_or(0);
                                        defs.get(mat_idx)
                                    })
                                    .and_then(|mat_def| mat_def.pbr_metallic_roughness.as_ref())
                                    .and_then(|pbr| pbr.base_color_factor)
                                    .unwrap_or([1.0, 1.0, 1.0, 1.0]);

                                let texture_info = if has_textures {
                                    mesh.material.as_ref().map(|mat| {
                                        let mat_res = mat.resource.unwrap_or(geom_info.resource);
                                        let tex_url = format!("{}/layers/0/nodes/{}/textures/0", self.service_url, mat_res);
                                        (tex_url, mat_res)
                                    })
                                } else {
                                    None
                                };

                                self.requested_node_ids.insert(node_id);
                                queue.pending_geometries.push(I3SWorkTask {
                                    node_id,
                                    geom_url: url,
                                    texture_info,
                                    geom_info: geom_info.clone(),
                                    geom_buf_def,
                                    obb_center: obb.center,
                                    base_color,
                                });
                                newly_enqueued += 1;
                            }
                        }
                    }
                }
            }
        }

        if newly_enqueued > 0 {
            self.work_condvar.notify_all();
        }

        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.result_sender.clone();
            let origin_val = self.origin.lock().map(|o| *o).unwrap_or_else(|_| ProjectOrigin::new(-37.8136, 144.9631, 0.0));
            let q_arc = self.work_queue.clone();
            const MAX_WASM_CONCURRENT: usize = 6;
            let mut tasks_to_dispatch = Vec::new();
            while queue.in_flight + tasks_to_dispatch.len() < MAX_WASM_CONCURRENT {
                if let Some(task) = queue.pending_geometries.pop() {
                    tasks_to_dispatch.push(task);
                } else {
                    break;
                }
            }
            queue.in_flight += tasks_to_dispatch.len();
            // Drop queue lock BEFORE initiating network requests
            drop(queue);

            for task in tasks_to_dispatch {
                let I3SWorkTask {
                    node_id,
                    geom_url,
                    texture_info,
                    geom_info,
                    geom_buf_def,
                    obb_center,
                    base_color,
                } = task;
                let tx_clone = tx.clone();
                let q_clone = q_arc.clone();
                crate::gis::platform::http::fetch_bytes(&geom_url, move |res| {
                    if let Ok(mut g) = q_clone.lock() {
                        g.in_flight = g.in_flight.saturating_sub(1);
                    }
                    if let Ok(bytes) = res {
                        let is_wgs84 = obb_center[0].abs() <= 180.0 && obb_center[1].abs() <= 90.0;
                        match I3SGeometryDecoder::decode(
                            &bytes,
                            &geom_info,
                            geom_buf_def.as_ref(),
                            obb_center,
                            is_wgs84,
                            &origin_val,
                            base_color,
                        ) {
                            Ok(mut decoded) => {
                                decoded.node_id = node_id;
                                if let Some((tex_url, _)) = texture_info {
                                    let tx_final = tx_clone.clone();
                                    crate::gis::platform::http::fetch_bytes(&tex_url, move |tex_res| {
                                        if let Ok(tb) = tex_res {
                                            if let Ok(dyn_img) = image::load_from_memory(&tb) {
                                                let rgba = dyn_img.to_rgba8();
                                                decoded.image_rgba = Some(std::sync::Arc::new((rgba.width(), rgba.height(), rgba.into_raw())));
                                            }
                                        }
                                        let _ = tx_final.send(I3SDownloadResult::Geometry(Box::new(decoded)));
                                    });
                                } else {
                                    let _ = tx_clone.send(I3SDownloadResult::Geometry(Box::new(decoded)));
                                }
                            }
                            Err(err) => {
                                log::warn!("[I3S] Failed to decode I3S node {}: {}", node_id, err);
                            }
                        }
                    } else {
                        let _ = tx_clone.send(I3SDownloadResult::Failure(Some(node_id), format!("Failed geometry {}", node_id)));
                    }
                });
            }
        }
    }

    /// Drains completed decoded meshes and metadata updates from background threads
    pub fn drain_completed(&mut self) -> Vec<DecodedI3SNode> {
        let mut completed_geometries = Vec::new();
        let mut incoming = Vec::new();

        if let Ok(rx) = self.result_receiver.lock() {
            while let Ok(res) = rx.try_recv() {
                incoming.push(res);
            }
        }

        for res in incoming {
            match res {
                I3SDownloadResult::Metadata(layer) => {
                    self.is_loading_metadata = false;
                    self.server_symbol = layer.extract_symbol();
                    self.has_new_server_symbol = true;
                    self.layer_metadata = Some(*layer);
                    // Automatically trigger root node page 0 fetch
                    self.fetch_node_page(0);
                    self.last_calc_instant = None;
                }
                I3SDownloadResult::NodePage(page_id, page) => {
                    self.pending_pages.remove(&page_id);
                    for node in &page.nodes {
                        if let Some(children) = &node.children {
                            for &child_id in children {
                                if let Some(child_node) = self.node_cache.get_mut(&child_id) {
                                    if child_node.parent_index.is_none() {
                                        child_node.parent_index = Some(node.index);
                                    }
                                }
                            }
                        }
                    }
                    for mut node in page.nodes {
                        if node.parent_index.is_none() {
                            for (_, cached_parent) in &self.node_cache {
                                if let Some(children) = &cached_parent.children {
                                    if children.contains(&node.index) {
                                        node.parent_index = Some(cached_parent.index);
                                        break;
                                    }
                                }
                            }
                        }
                        self.node_cache.insert(node.index, node);
                    }
                    self.last_calc_instant = None;
                }
                I3SDownloadResult::PageFailure(page_id, _) => {
                    self.pending_pages.remove(&page_id);
                    self.cached_pages.remove(&page_id);
                }
                I3SDownloadResult::Geometry(decoded) => {
                    self.loaded_node_ids.insert(decoded.node_id);
                    self.raw_meshes.insert(decoded.node_id, decoded.raw_mesh.clone());
                    for feat in &decoded.features {
                        let key = format!("i3s_feat_{}_{}", decoded.node_id, feat.feature_id);
                        self.raw_features.insert(key, feat.clone());
                    }
                    completed_geometries.push(*decoded);
                }
                I3SDownloadResult::Failure(failed_node_id, _) => {
                    if let Some(id) = failed_node_id {
                        self.requested_node_ids.remove(&id);
                    }
                }
            }
        }

        if !completed_geometries.is_empty() {
            // New tiles arrived: invalidate throttle so next frame retires old tiles immediately!
            self.last_calc_instant = None;
        }

        completed_geometries
    }

    pub fn unmark_requested(&mut self, node_id: u32) {
        self.requested_node_ids.remove(&node_id);
        self.loaded_node_ids.remove(&node_id);
        self.raw_meshes.remove(&node_id);
    }
}

impl Drop for I3SManager {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.is_shutdown = true;
        }
        self.work_condvar.notify_all();
    }
}
