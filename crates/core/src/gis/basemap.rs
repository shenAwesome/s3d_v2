use crate::gis::crs::{GeoCoord, ProjectOrigin};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

/// Supported Basemap Services
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BasemapProvider {
    EsriImagery,
    EsriTopo,
    EsriStreet,
    CartoDark,
    CartoLight,
    OpenStreetMap,
    None,
}

impl Default for BasemapProvider {
    fn default() -> Self {
        Self::OpenStreetMap
    }
}

impl BasemapProvider {
    pub fn all() -> &'static [BasemapProvider] {
        &[
            BasemapProvider::EsriImagery,
            BasemapProvider::CartoDark,
            BasemapProvider::CartoLight,
            BasemapProvider::OpenStreetMap,
            BasemapProvider::EsriTopo,
            BasemapProvider::EsriStreet,
            BasemapProvider::None,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            BasemapProvider::EsriImagery => "Esri World Imagery (Satellite)",
            BasemapProvider::EsriTopo => "Esri World Topo",
            BasemapProvider::EsriStreet => "Esri World Streets",
            BasemapProvider::CartoDark => "CartoDB Dark Matter",
            BasemapProvider::CartoLight => "CartoDB Positron",
            BasemapProvider::OpenStreetMap => "OpenStreetMap Standard",
            BasemapProvider::None => "None (CAD Grid)",
        }
    }

    pub fn tile_url(&self, z: u32, x: u32, y: u32) -> Option<String> {
        match self {
            BasemapProvider::EsriImagery => Some(format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{}/{}/{}",
                z, y, x
            )),
            BasemapProvider::EsriTopo => Some(format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{}/{}/{}",
                z, y, x
            )),
            BasemapProvider::EsriStreet => Some(format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{}/{}/{}",
                z, y, x
            )),
            BasemapProvider::CartoDark => Some(format!(
                "https://basemaps.cartocdn.com/rastertiles/dark_all/{}/{}/{}.png",
                z, x, y
            )),
            BasemapProvider::CartoLight => Some(format!(
                "https://basemaps.cartocdn.com/rastertiles/light_all/{}/{}/{}.png",
                z, x, y
            )),
            BasemapProvider::OpenStreetMap => Some(format!(
                "https://tile.openstreetmap.org/{}/{}/{}.png",
                z, x, y
            )),
            BasemapProvider::None => None,
        }
    }

    pub fn attribution(&self) -> &'static str {
        match self {
            BasemapProvider::EsriImagery | BasemapProvider::EsriTopo | BasemapProvider::EsriStreet => {
                "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics"
            }
            BasemapProvider::CartoDark | BasemapProvider::CartoLight => {
                "© OpenStreetMap contributors, © CARTO"
            }
            BasemapProvider::OpenStreetMap => "© OpenStreetMap contributors",
            BasemapProvider::None => "",
        }
    }
}

/// Slippy map tile coordinate at a specific zoom level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

impl TileCoord {
    pub fn new(z: u32, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }

    /// Converts latitude & longitude in WGS84 to tile coordinates at zoom level `z`
    pub fn from_geo(lat: f64, lon: f64, z: u32) -> Self {
        let n = 2.0_f64.powi(z as i32);
        let x = (((lon + 180.0) / 360.0) * n).floor() as u32;

        let lat_rad = lat.to_radians();
        let y = ((1.0 - (lat_rad.tan() + (1.0 / lat_rad.cos())).ln() / std::f64::consts::PI) / 2.0 * n).floor() as u32;

        let max_idx = (1u32 << z).saturating_sub(1);
        Self {
            z,
            x: x.min(max_idx),
            y: y.min(max_idx),
        }
    }

    /// Computes the geographic bounding box (min_lat, max_lat, min_lon, max_lon) in degrees
    pub fn geo_bounds(&self) -> (f64, f64, f64, f64) {
        let n = 2.0_f64.powi(self.z as i32);

        let min_lon = (self.x as f64 / n) * 360.0 - 180.0;
        let max_lon = ((self.x + 1) as f64 / n) * 360.0 - 180.0;

        let y_top = self.y as f64;
        let y_bottom = (self.y + 1) as f64;

        let max_lat = (std::f64::consts::PI * (1.0 - 2.0 * y_top / n)).sinh().atan().to_degrees();
        let min_lat = (std::f64::consts::PI * (1.0 - 2.0 * y_bottom / n)).sinh().atan().to_degrees();

        (min_lat, max_lat, min_lon, max_lon)
    }

    /// Converts tile corners to Local ENU coordinates around a ProjectOrigin (y is elevation)
    pub fn to_enu_corners(&self, origin: &ProjectOrigin) -> [Vec3; 4] {
        let (min_lat, max_lat, min_lon, max_lon) = self.geo_bounds();

        let elev = origin.origin.elevation;
        let nw = origin.geo_to_local(&GeoCoord::new(max_lat, min_lon, elev));
        let ne = origin.geo_to_local(&GeoCoord::new(max_lat, max_lon, elev));
        let se = origin.geo_to_local(&GeoCoord::new(min_lat, max_lon, elev));
        let sw = origin.geo_to_local(&GeoCoord::new(min_lat, min_lon, elev));

        [nw, ne, se, sw]
    }

    /// Computes the 3D Axis-Aligned Bounding Box (AABB) in Local ENU space for this tile
    pub fn to_enu_aabb(&self, origin: &ProjectOrigin) -> (Vec3, Vec3) {
        let (min_lat, max_lat, min_lon, max_lon) = self.geo_bounds();
        let elev = origin.origin.elevation;

        let nw = origin.geo_to_local(&GeoCoord::new(max_lat, min_lon, elev));
        let ne = origin.geo_to_local(&GeoCoord::new(max_lat, max_lon, elev));
        let se = origin.geo_to_local(&GeoCoord::new(min_lat, max_lon, elev));
        let sw = origin.geo_to_local(&GeoCoord::new(min_lat, min_lon, elev));
        let center = origin.geo_to_local(&GeoCoord::new((min_lat + max_lat) * 0.5, (min_lon + max_lon) * 0.5, elev));

        let mut min_pos = nw.min(ne).min(se).min(sw).min(center);
        let mut max_pos = nw.max(ne).max(se).max(sw).max(center);

        // In Planar ENU space, ground surface features sit on or near Y=0.
        // For large tiles covering whole continents/hemispheres, the sampled corners sag
        // thousands of kilometers into negative ENU altitudes due to Earth curvature.
        // If this tile encloses the origin, Y=0 is geometrically inside the tile bounds!
        let origin_lat = origin.origin.latitude;
        let origin_lon = origin.origin.longitude;
        if min_lat <= origin_lat && origin_lat <= max_lat && min_lon <= origin_lon && origin_lon <= max_lon {
            max_pos.y = max_pos.y.max(0.0);
            min_pos.y = min_pos.y.min(0.0);
        }

        // Include reasonable vertical height bounds around the ground plane.
        // For large continental tiles, curvature sag dictates vertical extent.
        // For local/street-level tiles (Z >= 8), bounding box height is scaled
        // to the tile's horizontal scale rather than creating a 900-meter-tall vertical pillar
        // that intersects the camera perspective frustum far outside the viewport!
        let tile_extent = (max_pos.x - min_pos.x).max(max_pos.z - min_pos.z);
        let (down_margin, up_margin) = if self.z >= 12 {
            let down = (tile_extent * 0.35).clamp(20.0, 150.0);
            let up = (tile_extent * 0.50).clamp(35.0, 300.0);
            (down, up)
        } else if self.z >= 8 {
            (100.0, 400.0)
        } else {
            (250.0, 600.0)
        };

        min_pos.y -= down_margin;
        max_pos.y += up_margin;

        (min_pos, max_pos)
    }

    /// Computes minimum squared distance from an ENU point (e.g. camera eye) to this tile on the ground plane (Y=0)
    pub fn distance_squared_to_point(&self, origin: &ProjectOrigin, point: Vec3) -> f32 {
        let (min_box, max_box) = self.to_enu_aabb(origin);
        let dx = (min_box.x - point.x).max(0.0).max(point.x - max_box.x);
        let dy = point.y.abs(); // Distance to ground plane Y=0
        let dz = (min_box.z - point.z).max(0.0).max(point.z - max_box.z);
        dx * dx + dy * dy + dz * dz
    }

    /// Computes the 3D Axis-Aligned Bounding Box (AABB) in ECEF space for this tile
    pub fn to_ecef_aabb(&self) -> (Vec3, Vec3) {
        let (min_lat, max_lat, min_lon, max_lon) = self.geo_bounds();
        let center_lat = (min_lat + max_lat) * 0.5;
        let center_lon = (min_lon + max_lon) * 0.5;

        let nw = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(max_lat, min_lon, 0.0));
        let ne = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(max_lat, max_lon, 0.0));
        let se = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(min_lat, max_lon, 0.0));
        let sw = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(min_lat, min_lon, 0.0));
        let center = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(center_lat, center_lon, 0.0));
        let n_mid = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(max_lat, center_lon, 0.0));
        let s_mid = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(min_lat, center_lon, 0.0));
        let w_mid = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(center_lat, min_lon, 0.0));
        let e_mid = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(center_lat, max_lon, 0.0));

        let mut min_pos = nw.min(ne).min(se).min(sw).min(center).min(n_mid).min(s_mid).min(w_mid).min(e_mid);
        let mut max_pos = nw.max(ne).max(se).max(sw).max(center).max(n_mid).max(s_mid).max(w_mid).max(e_mid);

        // Include elevation margin for curved Earth surface scaled with tile dimension
        let tile_extent = (max_pos - min_pos).length();
        let margin = (tile_extent * 0.10 + 40.0).min(1200.0);
        min_pos -= Vec3::splat(margin);
        max_pos += Vec3::splat(margin);

        (min_pos, max_pos)
    }

    /// Computes minimum squared distance from an ECEF point (e.g. camera eye) to this tile's ECEF AABB
    pub fn distance_squared_to_ecef_point(&self, point: Vec3) -> f32 {
        let (min_box, max_box) = self.to_ecef_aabb();
        let dx = (min_box.x - point.x).max(0.0).max(point.x - max_box.x);
        let dy = (min_box.y - point.y).max(0.0).max(point.y - max_box.y);
        let dz = (min_box.z - point.z).max(0.0).max(point.z - max_box.z);
        dx * dx + dy * dy + dz * dz
    }

    /// Returns the ancestor coordinate at `zoom_delta` levels higher (coarser)
    pub fn ancestor(&self, zoom_delta: u32) -> Self {
        if zoom_delta >= self.z {
            Self { z: 0, x: 0, y: 0 }
        } else {
            Self {
                z: self.z - zoom_delta,
                x: self.x >> zoom_delta,
                y: self.y >> zoom_delta,
            }
        }
    }

    /// Checks if this coordinate is a child/descendant of `other`
    pub fn is_descendant_of(&self, other: &TileCoord) -> bool {
        if self.z <= other.z {
            return false;
        }
        let dz = self.z - other.z;
        (self.x >> dz) == other.x && (self.y >> dz) == other.y
    }
}

/// Decoded RGBA tile image ready for GPU texture upload
#[derive(Clone, Debug)]
pub struct DecodedTile {
    pub coord: TileCoord,
    pub provider: BasemapProvider,
    pub width: u32,
    pub height: u32,
    pub rgba_bytes: Vec<u8>,
}

impl DecodedTile {
    pub fn dummy(coord: TileCoord) -> Self {
        Self {
            coord,
            provider: BasemapProvider::None,
            width: 2,
            height: 2,
            rgba_bytes: vec![128, 128, 128, 255, 128, 128, 128, 255, 128, 128, 128, 255, 128, 128, 128, 255],
        }
    }
}

/// In-memory host CPU RAM tile cache (LRU) holding decoded raster RGBA images.
/// Lives strictly in system memory (RAM heap) with ZERO GPU VRAM allocation!
pub struct TileRamCache {
    entries: std::collections::HashMap<(TileCoord, BasemapProvider), DecodedTile>,
    order: std::collections::VecDeque<(TileCoord, BasemapProvider)>,
    capacity: usize,
}

impl TileRamCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::with_capacity(capacity),
            order: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn get(&mut self, coord: TileCoord, provider: BasemapProvider) -> Option<DecodedTile> {
        let key = (coord, provider);
        if let Some(tile) = self.entries.get(&key) {
            let tile = tile.clone();
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
            Some(tile)
        } else {
            None
        }
    }

    pub fn contains(&self, coord: TileCoord, provider: BasemapProvider) -> bool {
        self.entries.contains_key(&(coord, provider))
    }

    pub fn insert(&mut self, tile: DecodedTile) {
        let key = (tile.coord, tile.provider);
        if self.entries.contains_key(&key) {
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
        } else if self.entries.len() >= self.capacity {
            if let Some(oldest_key) = self.order.pop_front() {
                self.entries.remove(&oldest_key);
            }
        }
        self.order.push_back(key);
        self.entries.insert(key, tile);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

pub enum TileDownloadResult {
    Success(DecodedTile, u64 /* epoch */),
    Failure(TileCoord, BasemapProvider, u64 /* epoch */),
}

#[derive(Debug, Clone)]
pub struct TileDownloadTask {
    pub coord: TileCoord,
    pub provider: BasemapProvider,
    pub url: String,
    pub priority: i32, // Higher number = higher priority
    pub epoch: u64,    // cache generation; tiles from stale epochs are discarded on drain
    pub retries: u8,
}


pub struct TileWorkQueue {
    pub tasks: Vec<TileDownloadTask>,
    pub valid_keys: HashSet<(TileCoord, BasemapProvider)>,
    pub in_flight: usize,
    pub unconsumed_results: usize,
    pub is_shutdown: bool,
}

impl TileWorkQueue {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            valid_keys: HashSet::new(),
            in_flight: 0,
            unconsumed_results: 0,
            is_shutdown: false,
        }
    }
}

/// Asynchronous Basemap Tile Downloader & Cache Manager
pub struct BasemapManager {
    pub provider: BasemapProvider,
    pub zoom: u32,
    pub opacity: f32,
    pub is_enabled: bool,
    pub show_debug_borders: bool,

    // Thread-safe prioritized work queue & condition variable
    work_queue: Arc<Mutex<TileWorkQueue>>,
    work_condvar: Arc<Condvar>,

    #[allow(dead_code)]
    result_sender: Sender<TileDownloadResult>,
    result_receiver: Receiver<TileDownloadResult>,

    // Tracking state
    requested_tiles: HashSet<(TileCoord, BasemapProvider)>,
    pub batch_total: usize,
    pub batch_completed: usize,
    pub loaded_count: usize,
    pub target_active_count: usize,
    pub previous_active_tiles: HashSet<TileCoord>,
    pub ram_cache: TileRamCache,
    ram_hits: Vec<DecodedTile>,

    /// Monotonically increasing generation counter.
    /// Incremented by `reset_cache()` so that tiles completed from a prior
    /// cache generation (in-flight at the time of reset) are silently dropped
    /// in `drain_completed_tiles()` instead of being uploaded to the GPU with
    /// mesh geometry built for a stale camera position.
    cache_epoch: u64,
}

impl BasemapManager {
    pub const MAX_CONCURRENT_WORKERS: usize = 6;
    pub const MAX_PENDING_QUEUE: usize = 128;

    pub fn new() -> Self {
        let work_queue = Arc::new(Mutex::new(TileWorkQueue::new()));
        let work_condvar = Arc::new(Condvar::new());
        let (result_sender, result_receiver) = channel::<TileDownloadResult>();

        #[cfg(not(target_arch = "wasm32"))]
        // Spawn 4 parallel background workers for controlled multithreaded tile downloading & decoding
        for _ in 0..Self::MAX_CONCURRENT_WORKERS {
            let queue_arc = work_queue.clone();
            let cond_arc = work_condvar.clone();
            let tx = result_sender.clone();

            thread::spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(std::time::Duration::from_secs(4))
                    .timeout_read(std::time::Duration::from_secs(6))
                    .user_agent("s3d-gis/0.1.0 (Antigravity 3D GIS & Solar Analysis Engine)")
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

                            // Try to pop the highest-priority valid task
                            if let Some(task) = guard.tasks.pop() {
                                // CANCELLATION CHECK:
                                // If the tile was panned out of view, its key was removed from valid_keys.
                                // In that case, we discard it immediately with zero network overhead!
                                if guard.valid_keys.contains(&(task.coord, task.provider)) {
                                    guard.in_flight += 1;
                                    break task;
                                }
                                // Discard stale/cancelled task and check next
                            } else {
                                guard = match cond_arc.wait(guard) {
                                    Ok(g) => g,
                                    Err(_) => return,
                                };
                            }
                        }
                    };

                    // 1. Check persistent disk cache first (offline instant hit)
                    if let Some(cached_bytes) = crate::gis::cache::DiskCacheManager::read_basemap_tile(task.provider, task.coord) {
                        if let Ok(img) = image::load_from_memory(&cached_bytes) {
                            let rgba = img.to_rgba8();
                            let (w, h) = rgba.dimensions();
                            let _ = tx.send(TileDownloadResult::Success(DecodedTile {
                                coord: task.coord,
                                provider: task.provider,
                                width: w,
                                height: h,
                                rgba_bytes: rgba.into_raw(),
                            }, task.epoch));
                            if let Ok(mut guard) = queue_arc.lock() {
                                guard.in_flight = guard.in_flight.saturating_sub(1);
                                guard.unconsumed_results += 1;
                            }
                            continue;
                        }
                    }

                    // 2. Perform network request and caching with up to 3 retries
                    let mut success = false;
                    const MAX_RETRIES: u8 = 3;
                    for attempt in 0..=MAX_RETRIES {
                        if attempt > 0 {
                            // Exponential backoff: 120ms, 240ms, 480ms
                            let backoff_ms = 120u64 * (1 << (attempt - 1));
                            thread::sleep(std::time::Duration::from_millis(backoff_ms));
                        }

                        if let Ok(response) = agent.get(&task.url).call() {
                            let mut bytes = Vec::new();
                            if response.into_reader().read_to_end(&mut bytes).is_ok() && !bytes.is_empty() {
                                // Write raw bytes to persistent disk cache
                                crate::gis::cache::DiskCacheManager::write_basemap_tile(task.provider, task.coord, &bytes);

                                if let Ok(img) = image::load_from_memory(&bytes) {
                                    let rgba = img.to_rgba8();
                                    let (w, h) = rgba.dimensions();
                                    let _ = tx.send(TileDownloadResult::Success(DecodedTile {
                                        coord: task.coord,
                                        provider: task.provider,
                                        width: w,
                                        height: h,
                                        rgba_bytes: rgba.into_raw(),
                                    }, task.epoch));
                                    success = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !success {
                        let _ = tx.send(TileDownloadResult::Failure(task.coord, task.provider, task.epoch));
                    }


                    // Decrement in_flight counter and record unconsumed result
                    if let Ok(mut guard) = queue_arc.lock() {
                        guard.in_flight = guard.in_flight.saturating_sub(1);
                        guard.unconsumed_results += 1;
                    }
                }
            });
        }

        Self {
            provider: BasemapProvider::CartoLight,
            zoom: 16,
            opacity: 1.0,
            is_enabled: true,
            show_debug_borders: false,
            work_queue,
            work_condvar,
            result_sender,
            result_receiver,
            requested_tiles: HashSet::new(),
            batch_total: 0,
            batch_completed: 0,
            loaded_count: 0,
            target_active_count: 0,
            previous_active_tiles: HashSet::new(),
            #[cfg(target_arch = "wasm32")]
            ram_cache: TileRamCache::new(128),
            #[cfg(not(target_arch = "wasm32"))]
            ram_cache: TileRamCache::new(512),
            ram_hits: Vec::new(),
            cache_epoch: 0,
        }
    }

    /// Clears requested tracking and work queue
    pub fn clear_cache(&mut self) {
        self.requested_tiles.clear();
        self.previous_active_tiles.clear();
        self.ram_hits.clear();
        if let Ok(mut q) = self.work_queue.lock() {
            q.tasks.clear();
            q.valid_keys.clear();
            q.in_flight = 0;
        }
        self.batch_total = 0;
        self.batch_completed = 0;
    }

    /// Returns the number of tiles actively being downloaded
    pub fn in_flight_count(&self) -> usize {
        if let Ok(q) = self.work_queue.lock() {
            q.in_flight
        } else {
            0
        }
    }

    /// Alias for in_flight_count
    pub fn active_in_flight(&self) -> usize {
        self.in_flight_count()
    }

    /// Returns the number of tiles currently pending in the background worker queue
    pub fn pending_tasks_count(&self) -> usize {
        if let Ok(q) = self.work_queue.lock() {
            q.tasks.len()
        } else {
            0
        }
    }

    /// Returns the number of tiles held in the RAM LRU decoded cache
    pub fn cache_count(&self) -> usize {
        self.ram_cache.len()
    }

    /// Returns true if tiles are actively downloading in background or ready in RAM
    pub fn is_streaming(&self) -> bool {
        if !self.is_enabled || self.provider == BasemapProvider::None {
            return false;
        }
        if !self.ram_hits.is_empty() {
            return true;
        }
        if let Ok(q) = self.work_queue.lock() {
            q.in_flight > 0 || !q.tasks.is_empty() || q.unconsumed_results > 0
        } else {
            false
        }
    }

    /// Computes the continuous fractional zoom level (e.g. 15.42) matching MapLibre GL & Esri ArcGIS standard
    pub fn calculate_continuous_lod_zoom(
        distance_meters: f32,
        fov_y: f32,
        viewport_height: f32,
        latitude_deg: f64,
    ) -> f32 {
        let vp_h = viewport_height.max(256.0);
        let meters_per_pixel = (2.0 * distance_meters.max(1.0) * (fov_y * 0.5).tan()) / vp_h;
        let cos_lat = latitude_deg.to_radians().cos().abs().clamp(0.05, 1.0);
        let world_circumference = 40_075_016.686 * cos_lat;

        let continuous_zoom = (world_circumference / (256.0 * meters_per_pixel as f64)).log2() as f32;
        continuous_zoom.clamp(0.0, 24.0)
    }

    /// Dynamically computes the integer map tile zoom level based on camera distance, FOV, screen resolution, and latitude (MapLibre & Esri ArcGIS standard)
    pub fn calculate_camera_lod_zoom(
        distance_meters: f32,
        fov_y: f32,
        viewport_height: f32,
        latitude_deg: f64,
    ) -> u32 {
        let cont = Self::calculate_continuous_lod_zoom(distance_meters, fov_y, viewport_height, latitude_deg);
        cont.round().clamp(0.0, 18.0) as u32
    }

    /// Computes the integer map tile zoom level with a hysteresis deadband
    /// to eliminate threshold ping-pong flickering during continuous camera zoom/pan.
    pub fn calculate_camera_lod_zoom_with_hysteresis(
        distance_meters: f32,
        fov_y: f32,
        viewport_height: f32,
        latitude_deg: f64,
        current_zoom: u32,
    ) -> u32 {
        let cont = Self::calculate_continuous_lod_zoom(distance_meters, fov_y, viewport_height, latitude_deg);
        if current_zoom == 0 {
            return cont.round().clamp(0.0, 18.0) as u32;
        }
        let curr = current_zoom as f32;
        // Symmetric hysteresis deadband (+0.60 / -0.60)
        if cont >= curr + 0.60 {
            (cont.round() as u32).clamp(0, 18)
        } else if cont <= curr - 0.60 {
            (cont.round() as u32).clamp(0, 18)
        } else {
            current_zoom.clamp(0, 18)
        }
    }

    /// Dynamic ground distance horizon cutoff matching the perspective visual horizon
    pub fn calculate_horizon_cutoff(distance_meters: f32, pitch_radians: f32) -> f64 {
        let sin_pitch = pitch_radians.abs().sin().clamp(0.08, 1.0);
        (distance_meters as f64 * (20.0 / sin_pitch as f64)).clamp(50_000.0, 120_000.0)
    }

    /// Dynamic zoom fallback from distance
    pub fn dynamic_zoom_from_distance(distance_meters: f32) -> u32 {
        Self::calculate_camera_lod_zoom(distance_meters, 45.0f32.to_radians(), 720.0, -37.8136)
    }

    /// Calculates multi-scale asymmetrical LOD tile pyramid for perspective 3D rendering (MapLibre & Esri ArcGIS standard)
    pub fn calculate_asymmetric_pyramid_tiles(
        &self,
        origin: &ProjectOrigin,
        camera_target: glam::Vec3,
        camera_distance: f32,
        camera_pitch: f32,
        camera_yaw: f32,
    ) -> Vec<TileCoord> {
        self.calculate_camera_pyramid_tiles(
            origin,
            camera_target,
            camera_distance,
            camera_pitch,
            camera_yaw,
            45.0f32.to_radians(),
            720.0,
        )
    }

    /// Camera-aware multi-scale LOD tile pyramid calculation using 3D Frustum Culling & Screen-Space Error LOD
    pub fn calculate_camera_pyramid_tiles(
        &self,
        origin: &ProjectOrigin,
        camera_target: glam::Vec3,
        camera_distance: f32,
        camera_pitch: f32,
        camera_yaw: f32,
        fov_y: f32,
        viewport_height: f32,
    ) -> Vec<TileCoord> {
        let camera = crate::renderer::camera::Camera {
            target: camera_target,
            yaw: camera_yaw,
            pitch: camera_pitch,
            distance: camera_distance,
            fov_y,
            z_near: 1.0,
            z_far: (camera_distance * 50.0).clamp(2_000_000.0, 200_000_000.0),
            target_distance: camera_distance,
            target_lookat: camera_target,
            target_yaw: camera_yaw,
            target_pitch: camera_pitch,
        };
        self.calculate_camera_tiles(origin, &camera, 1280.0, viewport_height)
    }

    /// True Viewport & 3D Frustum-Culled Quadtree LOD tile pyramid calculation (MapLibre GL & Cesium standard)
    pub fn calculate_camera_tiles(
        &self,
        origin: &ProjectOrigin,
        camera: &crate::renderer::camera::Camera,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Vec<TileCoord> {
        let aspect = (viewport_width / viewport_height.max(1.0)).max(0.1);
        let vp = camera.view_proj_matrix(aspect);
        let frustum = crate::renderer::camera::Frustum::from_view_proj(vp);
        let camera_eye = camera.eye_position();

        let mut visible_tiles = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        let target_geo = origin.local_to_geo(camera.target);
        let center_ideal_zoom = Self::calculate_camera_lod_zoom_with_hysteresis(
            camera.distance,
            camera.fov_y,
            viewport_height,
            target_geo.latitude,
            self.zoom,
        );

        // Bounded LOD zoom window: allow multiscale LOD down to low zoom (Z4..10)
        // so distant horizon tiles use coarse low-detail tiles instead of exploding into hundreds of high-zoom tiles.
        let min_lod_zoom = center_ideal_zoom.saturating_sub(10).max(4);
        let max_lod_zoom = center_ideal_zoom.min(17);

        // Ground distance horizon cutoff: matches perspective visual horizon (50 - 120 km)
        // so ground coverage extends all the way into the atmospheric horizon fog (32 km) without gaps.
        let max_ground_dist = Self::calculate_horizon_cutoff(camera.distance, camera.pitch);

        // In Planar ENU Mode, the world coordinates are in meters centered around `origin`.
        // Start the quadtree traversal from root zoom Z=4 or Z=5.
        // At Z=4, each tile spans ~2,500 km, so a 3x3 grid spans >7,500 km, guaranteeing
        // that panning the camera does not jump across root tile boundaries or drop columns.
        let root_zoom = min_lod_zoom.saturating_sub(2).clamp(2, 5);
        let center_root_tile = TileCoord::from_geo(target_geo.latitude, target_geo.longitude, root_zoom);
        let max_root_idx = (1u32 << root_zoom).saturating_sub(1) as i32;
        let cr_x = center_root_tile.x as i32;
        let cr_y = center_root_tile.y as i32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let rx = cr_x + dx;
                let ry = cr_y + dy;
                if rx >= 0 && rx <= max_root_idx && ry >= 0 && ry <= max_root_idx {
                    queue.push_back(TileCoord::new(root_zoom, rx as u32, ry as u32));
                }
            }
        }

        while let Some(coord) = queue.pop_front() {
            let (min_box, max_box) = coord.to_enu_aabb(origin);

            // Add a 25% horizontal margin to prevent edge tiles chattering or popping on panning/rotation
            let dx = (max_box.x - min_box.x) * 0.25;
            let dz = (max_box.z - min_box.z) * 0.25;
            let test_min = glam::Vec3::new(min_box.x - dx, min_box.y, min_box.z - dz);
            let test_max = glam::Vec3::new(max_box.x + dx, max_box.y, max_box.z + dz);

            // 1. 3D Frustum Culling: If tile AABB is completely outside the camera view frustum, cull immediately.
            if !frustum.intersects_aabb(test_min, test_max) {
                continue;
            }

            // 2. Minimum distance from camera eye to tile on ground plane Y=0
            let min_dist_sq = coord.distance_squared_to_point(origin, camera_eye);
            let dist = min_dist_sq.sqrt().max(1.0);

            // Planar ENU Mode Horizon Cutoff
            if coord.z >= min_lod_zoom && dist > max_ground_dist as f32 {
                continue;
            }

            let (min_lat, max_lat, _, _) = coord.geo_bounds();
            let center_lat = (min_lat + max_lat) * 0.5;

            // 3. Screen-Space Error / Continuous LOD Zoom with Perspective Falloff Bias
            let cont_zoom = Self::calculate_continuous_lod_zoom(dist, camera.fov_y, viewport_height, center_lat);

            // Perspective distance falloff bias:
            // Focus area stays crisp and detailed, while remote/horizon areas transition faster
            // to coarser zoom levels (larger tiles) to cover vast ground spans efficiently without gaps.
            let dist_ratio = (dist / (camera.distance * 0.75).max(100.0)).max(1.0);
            let remote_bias = (dist_ratio.log2() * 1.0).clamp(0.0, 5.5);
            let target_cont_zoom = (cont_zoom - remote_bias).max(min_lod_zoom as f32);

            let is_previously_leaf = self.previous_active_tiles.contains(&coord);
            let is_previously_split = !is_previously_leaf && self.previous_active_tiles.iter().any(|t| t.is_descendant_of(&coord));

            let split_threshold = if is_previously_leaf {
                coord.z as f32 + 0.60
            } else if is_previously_split {
                coord.z as f32 + 0.40
            } else {
                coord.z as f32 + 0.50
            };

            // If tile hasn't reached minimum zoom or biased continuous zoom exceeds split threshold, subdivide into 4 children
            if (coord.z < min_lod_zoom || target_cont_zoom >= split_threshold) && coord.z < max_lod_zoom {
                let next_z = coord.z + 1;
                let children = [
                    TileCoord::new(next_z, coord.x * 2, coord.y * 2),
                    TileCoord::new(next_z, coord.x * 2 + 1, coord.y * 2),
                    TileCoord::new(next_z, coord.x * 2, coord.y * 2 + 1),
                    TileCoord::new(next_z, coord.x * 2 + 1, coord.y * 2 + 1),
                ];
                for c in children {
                    queue.push_back(c);
                }
            } else if coord.z >= min_lod_zoom {
                visible_tiles.push(coord);
            }
        }

        // Fallback: If frustum culled everything (e.g. extreme pitch looking into space), include tiles around target
        if visible_tiles.is_empty() {
            let ground_radius = (camera.distance as f64 * (camera.fov_y as f64 * 0.5).tan() * aspect.max(1.0) as f64 * 1.1).clamp(150.0, 2_500.0);
            let area_tiles = self.calculate_tiles_for_area(origin, camera.target, ground_radius, center_ideal_zoom);
            if !area_tiles.is_empty() {
                visible_tiles = area_tiles;
            } else {
                let center_tile = TileCoord::from_geo(target_geo.latitude, target_geo.longitude, center_ideal_zoom);
                visible_tiles.push(center_tile);
            }
        }

        // Deduplicate exact matches
        visible_tiles.sort_by_key(|t| (t.z, t.x, t.y));
        visible_tiles.dedup();

        // Strict Hierarchical LOD Invariant:
        // Pure quadtree traversal already produces disjoint leaves, but if any fallback
        // introduced overlaps, never include an ancestor if a finer descendant exists.
        let all_coords = visible_tiles.clone();
        visible_tiles.retain(|t| {
            !all_coords.iter().any(|other| other.is_descendant_of(t))
        });

        // Sort visible tiles: closest to camera eye first, and within same distance, lower zoom (parents) download first
        visible_tiles.sort_by(|a, b| {
            let d_a = a.distance_squared_to_point(origin, camera_eye);
            let d_b = b.distance_squared_to_point(origin, camera_eye);
            d_a.partial_cmp(&d_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.z.cmp(&b.z))
        });

        // Bounded active tile budget (up to 128 tiles max) to cover foreground, midground and horizon crisply
        if visible_tiles.len() > 128 {
            visible_tiles.truncate(128);
        }

        visible_tiles
    }

    /// Calculates the required tile coordinates to cover any given ENU center coordinate and radius at a specified zoom
    pub fn calculate_tiles_for_area(
        &self,
        origin: &ProjectOrigin,
        center_enu: glam::Vec3,
        radius_meters: f64,
        zoom: u32,
    ) -> Vec<TileCoord> {
        let center_geo = origin.local_to_geo(center_enu);

        let d_lat = radius_meters / 111_320.0;
        let d_lon = radius_meters / (111_320.0 * center_geo.latitude.to_radians().cos().abs().max(0.1));

        let min_lat = center_geo.latitude - d_lat;
        let max_lat = center_geo.latitude + d_lat;
        let min_lon = center_geo.longitude - d_lon;
        let max_lon = center_geo.longitude + d_lon;

        let nw_tile = TileCoord::from_geo(max_lat, min_lon, zoom);
        let se_tile = TileCoord::from_geo(min_lat, max_lon, zoom);
        let center_tile = TileCoord::from_geo(center_geo.latitude, center_geo.longitude, zoom);

        let max_tile_idx = (1u32 << zoom).saturating_sub(1) as i32;
        let raw_min_x = nw_tile.x.min(se_tile.x) as i32;
        let raw_max_x = nw_tile.x.max(se_tile.x) as i32;
        let raw_min_y = nw_tile.y.min(se_tile.y) as i32;
        let raw_max_y = nw_tile.y.max(se_tile.y) as i32;

        let c_x = center_tile.x as i32;
        let c_y = center_tile.y as i32;

        let max_span = if zoom <= 6 {
            4 // 9x9 = 81 tiles max for global overview
        } else if zoom <= 11 {
            3 // 7x7 = 49 tiles max for regional overview
        } else if zoom <= 15 {
            2 // 5x5 = 25 tiles max for city view
        } else {
            1 // 3x3 = 9 tiles max for street-level view (~800m x 800m)
        };
        let span_x = ((raw_max_x - raw_min_x + 1) / 2).clamp(1, max_span);
        let span_y = ((raw_max_y - raw_min_y + 1) / 2).clamp(1, max_span);

        let min_x = (c_x - span_x).clamp(0, max_tile_idx) as u32;
        let max_x = (c_x + span_x).clamp(0, max_tile_idx) as u32;
        let min_y = (c_y - span_y).clamp(0, max_tile_idx) as u32;
        let max_y = (c_y + span_y).clamp(0, max_tile_idx) as u32;

        let mut tiles = Vec::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                tiles.push(TileCoord::new(zoom, x, y));
            }
        }

        // Sort tiles by Manhattan distance from center so tiles right under the camera load first
        tiles.sort_by_key(|t| {
            (t.x as i32 - center_tile.x as i32).abs() + (t.y as i32 - center_tile.y as i32).abs()
        });

        tiles
    }

    /// Calculates multiscale LOD globe pyramid tiles facing the camera in 3D ECEF space
    pub fn calculate_globe_pyramid_tiles(&self, camera_eye: glam::Vec3) -> Vec<TileCoord> {
        let eye_len = camera_eye.length();
        let eye_alt = (eye_len - crate::gis::crs::WGS84_A as f32).max(10.0);
        let sub_sat = crate::gis::crs::ecef_to_geodetic(camera_eye);

        let camera = crate::renderer::camera::Camera {
            target: glam::Vec3::ZERO,
            yaw: (90.0 + sub_sat.longitude as f32).to_radians(),
            pitch: (sub_sat.latitude as f32).to_radians(),
            distance: eye_len.max(crate::gis::crs::WGS84_A as f32 + 10.0),
            fov_y: 45.0f32.to_radians(),
            z_near: (eye_alt * 0.005).clamp(100.0, 100_000.0),
            z_far: (eye_len * 5.0).clamp(150000.0, 200_000_000.0),
            target_distance: eye_len.max(crate::gis::crs::WGS84_A as f32 + 10.0),
            target_lookat: glam::Vec3::ZERO,
            target_yaw: (90.0 + sub_sat.longitude as f32).to_radians(),
            target_pitch: (sub_sat.latitude as f32).to_radians(),
        };

        self.calculate_globe_camera_tiles(&camera, 1280.0, 720.0)
    }

    /// Viewport & 3D Frustum-Culled Quadtree LOD tile pyramid calculation in 3D Globe ECEF space (Cesium & NASA WorldWind standard)
    pub fn calculate_globe_camera_tiles(
        &self,
        camera: &crate::renderer::camera::Camera,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Vec<TileCoord> {
        let aspect = (viewport_width / viewport_height.max(1.0)).max(0.1);
        let vp = camera.view_proj_matrix(aspect);
        let frustum = crate::renderer::camera::Frustum::from_view_proj(vp);
        let camera_eye = camera.eye_position();
        let eye_len = camera_eye.length();
        let eye_norm = if eye_len > 100.0 { camera_eye / eye_len } else { glam::Vec3::Y };

        let mut visible_tiles = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        // Start with root global tiles at Zoom 2 (16 tiles covering the planet)
        for y in 0..4 {
            for x in 0..4 {
                queue.push_back(TileCoord::new(2, x, y));
            }
        }

        let max_lod_zoom = 18u32;

        while let Some(coord) = queue.pop_front() {
            let (min_lat, max_lat, min_lon, max_lon) = coord.geo_bounds();
            let center_lat = (min_lat + max_lat) * 0.5;
            let center_lon = (min_lon + max_lon) * 0.5;

            let tile_center_ecef = crate::gis::crs::geodetic_to_ecef(&crate::gis::crs::GeoCoord::new(center_lat, center_lon, 0.0));
            let tile_normal = crate::gis::crs::geodetic_surface_normal(center_lat, center_lon);

            // 1. Horizon Culling: Is the tile on the back hemisphere of the Earth?
            let to_camera = camera_eye - tile_center_ecef;
            let dist = to_camera.length().max(1.0);
            let to_camera_dir = to_camera / dist;

            // Horizon angle threshold: tiles facing away from camera on the back of the sphere are culled
            if coord.z >= 3 && tile_normal.dot(to_camera_dir) < -0.20 && tile_normal.dot(eye_norm) < -0.10 {
                continue;
            }

            // 2. 3D Frustum Culling in ECEF Space (with 8% margin to prevent edge chattering)
            let (min_box, max_box) = coord.to_ecef_aabb();
            let extent = (max_box - min_box) * 0.08;
            let test_min = min_box - extent;
            let test_max = max_box + extent;
            if coord.z >= 3 && !frustum.intersects_aabb(test_min, test_max) {
                continue;
            }

            // 3. Screen-Space Error / Continuous LOD Zoom with Directional Hysteresis
            let cont_zoom = Self::calculate_continuous_lod_zoom(dist, camera.fov_y, viewport_height, center_lat)
                .clamp(2.0, max_lod_zoom as f32);

            let is_previously_leaf = self.previous_active_tiles.contains(&coord);
            let is_previously_split = !is_previously_leaf && self.previous_active_tiles.iter().any(|t| t.is_descendant_of(&coord));

            let split_threshold = if is_previously_leaf {
                coord.z as f32 + 0.60
            } else if is_previously_split {
                coord.z as f32 + 0.40
            } else {
                coord.z as f32 + 0.50
            };

            // If tile needs more detail and hasn't reached max zoom, subdivide into 4 children
            if (coord.z < 3 || cont_zoom >= split_threshold) && coord.z < max_lod_zoom {
                let next_z = coord.z + 1;
                let children = [
                    TileCoord::new(next_z, coord.x * 2, coord.y * 2),
                    TileCoord::new(next_z, coord.x * 2 + 1, coord.y * 2),
                    TileCoord::new(next_z, coord.x * 2, coord.y * 2 + 1),
                    TileCoord::new(next_z, coord.x * 2 + 1, coord.y * 2 + 1),
                ];
                for c in children {
                    queue.push_back(c);
                }
            } else {
                visible_tiles.push(coord);
            }
        }

        // Deduplicate
        visible_tiles.sort_by_key(|t| (t.z, t.x, t.y));
        visible_tiles.dedup();

        // Sort visible tiles: closest to camera eye first, and within same distance, lower zoom (parents) download first
        visible_tiles.sort_by(|a, b| {
            let d_a = a.distance_squared_to_ecef_point(camera_eye);
            let d_b = b.distance_squared_to_ecef_point(camera_eye);
            d_a.partial_cmp(&d_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.z.cmp(&b.z))
        });

        visible_tiles
    }

    /// Calculates the required tile coordinates to cover the local area around origin
    pub fn calculate_visible_tiles(&self, origin: &ProjectOrigin, radius_meters: f64) -> Vec<TileCoord> {
        self.calculate_tiles_for_area(origin, glam::Vec3::ZERO, radius_meters, self.zoom)
    }

    /// Requests downloads for tiles in view, canceling out-of-view tasks and maintaining a prioritized bounded queue
    pub fn request_tiles(&mut self, tiles: &[TileCoord]) {
        if self.provider == BasemapProvider::None || !self.is_enabled || tiles.is_empty() {
            return;
        }

        let mut queue = match self.work_queue.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        self.previous_active_tiles = tiles.iter().copied().collect();

        // 1. Build current visible keys set
        let mut current_visible = HashSet::with_capacity(tiles.len());
        for &coord in tiles {
            current_visible.insert((coord, self.provider));
        }

        // 2. CANCELLATION & PRUNING:
        // Cancel all pending tasks that are NO LONGER visible in current view!
        let prev_task_count = queue.tasks.len();
        let mut cancelled_keys = Vec::new();
        queue.tasks.retain(|task| {
            let key = (task.coord, task.provider);
            let keep = current_visible.contains(&key);
            if !keep {
                cancelled_keys.push(key);
            }
            keep
        });
        let cancelled_count = prev_task_count - queue.tasks.len();

        // Remove cancelled tasks from valid_keys AND from requested_tiles so they can be re-requested when visible!
        for key in cancelled_keys {
            queue.valid_keys.remove(&key);
            self.requested_tiles.remove(&key);
        }

        // 3. Enqueue new tiles with priority up to MAX_PENDING_QUEUE limit
        let mut newly_enqueued = 0;
        let available_slots = Self::MAX_PENDING_QUEUE.saturating_sub(queue.tasks.len());

        for (idx, &coord) in tiles.iter().enumerate() {
            if newly_enqueued >= available_slots {
                break;
            }

            let key = (coord, self.provider);
            if !self.requested_tiles.contains(&key) {
                // 1. Instant hit check from Host CPU RAM Cache (zero VRAM, zero network/disk latency)
                if let Some(cached_tile) = self.ram_cache.get(coord, self.provider) {
                    self.requested_tiles.insert(key);
                    self.ram_hits.push(cached_tile);
                    continue;
                }

                if let Some(url) = self.provider.tile_url(coord.z, coord.x, coord.y) {
                    self.requested_tiles.insert(key);
                    queue.valid_keys.insert(key);

                    // Priority formula (MapLibre standard):
                    // Highest priority = tiles closest to camera focal point / higher zoom levels
                    // Zoom boost: higher zoom gets higher priority (coord.z * 50)
                    // Proximity boost: lower idx in sorted visible_tiles = closer to camera eye
                    let zoom_prio = (coord.z as i32) * 50;
                    let distance_prio = (tiles.len().saturating_sub(idx)) as i32 * 25;
                    let priority = zoom_prio + distance_prio;

                    queue.tasks.push(TileDownloadTask {
                        coord,
                        provider: self.provider,
                        url,
                        priority,
                        epoch: self.cache_epoch,
                        retries: 0,
                    });
                    newly_enqueued += 1;
                }
            }
        }

        // 4. Sort queue so tasks.pop() retrieves the highest priority task first
        queue.tasks.sort_by_key(|t| t.priority);

        // 5. Update batch progress stats
        if newly_enqueued > 0 {
            if queue.tasks.len() == newly_enqueued {
                self.batch_total = newly_enqueued;
                self.batch_completed = 0;
            } else {
                self.batch_total = self.batch_total.saturating_sub(cancelled_count) + newly_enqueued;
            }
            self.work_condvar.notify_all();
        }

        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.result_sender.clone();
            let queue_arc = self.work_queue.clone();
            const MAX_WASM_CONCURRENT: usize = 8;
            let mut tasks_to_dispatch = Vec::new();
            while queue.in_flight + tasks_to_dispatch.len() < MAX_WASM_CONCURRENT {
                if let Some(task) = queue.tasks.pop() {
                    if queue.valid_keys.contains(&(task.coord, task.provider)) {
                        tasks_to_dispatch.push(task);
                    }
                } else {
                    break;
                }
            }
            queue.in_flight += tasks_to_dispatch.len();
            // Drop queue lock BEFORE initiating network requests
            drop(queue);

            for task in tasks_to_dispatch {
                let tx_clone = tx.clone();
                let q_clone = queue_arc.clone();
                let task_coord = task.coord;
                let task_provider = task.provider;
                let task_epoch = task.epoch;
                crate::gis::platform::http::fetch_bytes(&task.url, move |res| {
                    if let Ok(mut guard) = q_clone.lock() {
                        guard.in_flight = guard.in_flight.saturating_sub(1);
                    }
                    match res {
                        Ok(bytes) => {
                            match image::load_from_memory(&bytes) {
                                Ok(img) => {
                                    let rgba = img.to_rgba8();
                                    let (w, h) = rgba.dimensions();
                                    log::debug!("[Basemap] Decoded tile {:?} ({}x{}, {} bytes)", task_coord, w, h, bytes.len());
                                    let _ = tx_clone.send(TileDownloadResult::Success(DecodedTile {
                                        coord: task_coord,
                                        provider: task_provider,
                                        width: w,
                                        height: h,
                                        rgba_bytes: rgba.into_raw(),
                                    }, task_epoch));
                                }
                                Err(err) => {
                                    log::warn!("[Basemap] Image decode failed for tile {:?} ({} bytes): {}", task_coord, bytes.len(), err);
                                    let _ = tx_clone.send(TileDownloadResult::Failure(task_coord, task_provider, task_epoch));
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("[Basemap] Network fetch failed for tile {:?}: {}", task_coord, err);
                            let _ = tx_clone.send(TileDownloadResult::Failure(task_coord, task_provider, task_epoch));
                        }
                    }
                });
            }
        }
    }

    /// Drains any newly decoded tiles received from the background threads or RAM cache hits
    pub fn drain_completed_tiles(&mut self) -> Vec<DecodedTile> {
        let mut completed = std::mem::take(&mut self.ram_hits);
        let mut completed_keys = Vec::new();
        let mut failed_keys = Vec::new();

        while let Ok(res) = self.result_receiver.try_recv() {
            match res {
                TileDownloadResult::Success(tile, _epoch) => {
                    let key = (tile.coord, tile.provider);
                    completed_keys.push(key);
                    self.batch_completed += 1;

                    // Always retain valid decoded raster tile in Host CPU RAM cache
                    self.ram_cache.insert(tile.clone());

                    if tile.provider == self.provider {
                        self.loaded_count += 1;
                        completed.push(tile);
                    }
                }
                TileDownloadResult::Failure(coord, provider, _epoch) => {
                    let key = (coord, provider);
                    failed_keys.push(key);
                    self.batch_completed += 1;
                }
            }
        }

        if let Ok(mut queue) = self.work_queue.lock() {
            queue.unconsumed_results = 0;
            for key in completed_keys {
                queue.valid_keys.remove(&key);
            }
            for key in failed_keys {
                queue.valid_keys.remove(&key);
                self.requested_tiles.remove(&key);
            }
            if queue.tasks.is_empty() && queue.in_flight == 0 {
                self.batch_total = 0;
                self.batch_completed = 0;
            }
        }

        completed
    }

    /// Returns true if there are completed tiles waiting in RAM cache hits or receiver channel
    pub fn has_unconsumed_completed(&self) -> bool {
        if !self.ram_hits.is_empty() {
            return true;
        }
        if let Ok(q) = self.work_queue.lock() {
            q.unconsumed_results > 0
        } else {
            false
        }
    }

    /// Returns current tile loading progress: (progress_fraction_0_to_1, completed_count, total_count)
    pub fn loading_progress(&self) -> Option<(f32, usize, usize)> {
        if let Ok(queue) = self.work_queue.lock() {
            let pending = queue.tasks.len() + queue.in_flight;
            if pending > 0 && self.batch_total > 0 {
                let completed = self.batch_completed.min(self.batch_total);
                let total = self.batch_total.max(completed + pending);
                let frac = (completed as f32 / total as f32).clamp(0.05, 0.99);
                Some((frac, completed, total))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Unmark a tile as requested (e.g. if evicted from GPU cache) so it can be reloaded if needed
    pub fn unmark_requested(&mut self, coord: TileCoord) {
        let key = (coord, self.provider);
        self.requested_tiles.remove(&key);
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.valid_keys.remove(&key);
        }
        if self.loaded_count > 0 {
            self.loaded_count -= 1;
        }
    }

    /// Reset cache tracking when provider or zoom changes, or when basemap is toggled.
    pub fn reset_cache(&mut self) {
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.tasks.clear();
            queue.valid_keys.clear();
        }
        self.requested_tiles.clear();
        self.previous_active_tiles.clear();
        self.ram_hits.clear();
        self.batch_total = 0;
        self.batch_completed = 0;
        self.loaded_count = 0;
        self.cache_epoch = self.cache_epoch.wrapping_add(1);

        // Also drain any results already sitting in the channel buffer so they
        // don't accumulate and cause a spurious upload on the very next frame.
        while self.result_receiver.try_recv().is_ok() {}
    }
}

impl Drop for BasemapManager {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.is_shutdown = true;
        }
        self.work_condvar.notify_all();
    }
}

