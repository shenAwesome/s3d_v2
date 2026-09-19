use crate::gis::basemap::TileCoord;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

/// AWS Terrain Tiles (Terrarium DEM format)
/// Free, open global elevation dataset on AWS S3 Open Data.
/// Sourced from 3DEP (~10m in USA), SRTM (~30m globally), and GMTED fallback.
pub const AWS_TERRARIUM_URL: &str = "https://s3.amazonaws.com/elevation-tiles-prod/terrarium";

/// Esri WorldElevation3D Terrain3D ImageServer (LERC compressed high-res global elevation up to ~10m)
pub const ESRI_TERRAIN3D_URL: &str = "https://elevation3d.arcgis.com/arcgis/rest/services/WorldElevation3D/Terrain3D/ImageServer";

/// Selectable Elevation Terrain Data Providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainProvider {
    /// Esri WorldElevation3D Terrain3D (LERC compressed 257x257 float32 raster tiles)
    EsriTerrain3D,
    /// AWS Open Data Terrarium (256x256 RGB encoded PNG tiles)
    AwsTerrarium,
}

impl Default for TerrainProvider {
    fn default() -> Self {
        Self::EsriTerrain3D
    }
}

impl TerrainProvider {
    pub const ALL: &'static [TerrainProvider] = &[
        TerrainProvider::EsriTerrain3D,
        TerrainProvider::AwsTerrarium,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            TerrainProvider::EsriTerrain3D => "🏔 Esri WorldElevation3D (LERC)",
            TerrainProvider::AwsTerrarium => "⛰ AWS Terrarium (Open Data)",
        }
    }

    pub fn tile_url(&self, coord: TileCoord) -> String {
        match self {
            TerrainProvider::EsriTerrain3D => {
                // Esri REST ImageServer uses /tile/{z}/{row}/{col} -> /tile/{z}/{y}/{x}
                format!("{}/tile/{}/{}/{}", ESRI_TERRAIN3D_URL, coord.z, coord.y, coord.x)
            }
            TerrainProvider::AwsTerrarium => {
                // AWS Terrarium uses /{z}/{x}/{y}.png
                format!("{}/{}/{}/{}.png", AWS_TERRARIUM_URL, coord.z, coord.x, coord.y)
            }
        }
    }

    pub fn decode_tile(&self, coord: TileCoord, bytes: &[u8]) -> Result<DecodedTerrainTile, String> {
        match self {
            TerrainProvider::EsriTerrain3D => DecodedTerrainTile::from_lerc_bytes(coord, bytes),
            TerrainProvider::AwsTerrarium => DecodedTerrainTile::from_image_bytes(coord, bytes),
        }
    }
}

/// Decodes Terrarium RGB values into elevation in meters.
/// Formula: (R * 256 + G + B / 256) - 32768
#[inline]
pub fn decode_terrarium_height(r: u8, g: u8, b: u8) -> f32 {
    (r as f32 * 256.0 + g as f32 + b as f32 / 256.0) - 32768.0
}

/// Encodes elevation in meters into Terrarium RGB values (useful for unit testing and offline caching)
pub fn encode_terrarium_height(meters: f32) -> [u8; 3] {
    let value = (meters + 32768.0).clamp(0.0, 65535.996);
    let r = (value / 256.0).floor() as u8;
    let remainder = value - (r as f32 * 256.0);
    let g = remainder.floor() as u8;
    let b = ((remainder - g as f32) * 256.0).clamp(0.0, 255.0).round() as u8;
    [r, g, b]
}

/// 3D Terrain Shading Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainShadingMode {
    /// Textured with active Satellite / Basemap imagery
    TexturedBasemap,
    /// Colorized elevation ramp (Green valley -> Brown slopes -> Snow peak)
    ElevationRamp,
    /// Topographic hillshade contours
    Hillshade,
}

impl Default for TerrainShadingMode {
    fn default() -> Self {
        Self::TexturedBasemap
    }
}

/// Grid resolution of heightmap sampled per tile (e.g. 33 x 33 grid = 1089 vertices)
pub const TERRAIN_GRID_RES: usize = 33;

/// Decoded 3D Terrain Elevation Tile
#[derive(Clone)]
pub struct DecodedTerrainTile {
    pub coord: TileCoord,
    /// Elevation grid in meters of size (TERRAIN_GRID_RES x TERRAIN_GRID_RES)
    pub grid: [[f32; TERRAIN_GRID_RES]; TERRAIN_GRID_RES],
    pub min_height: f32,
    pub max_height: f32,
}

impl DecodedTerrainTile {
    /// Creates a decoded terrain tile from Esri LERC compressed elevation bytes (257x257 float)
    pub fn from_lerc_bytes(coord: TileCoord, lerc_bytes: &[u8]) -> Result<Self, String> {
        let slice = lerc::decode_slice::<f32>(lerc_bytes)
            .map_err(|e| format!("Failed to decode LERC terrain: {:?}", e))?;

        let width = slice.width as usize;
        let height = slice.height as usize;
        if width == 0 || height == 0 || slice.pixels.is_empty() {
            return Err("Empty LERC raster data".to_string());
        }

        let mut grid = [[0.0f32; TERRAIN_GRID_RES]; TERRAIN_GRID_RES];
        let mut min_h = f32::INFINITY;
        let mut max_h = f32::NEG_INFINITY;

        let max_x = (width.saturating_sub(1)) as f32;
        let max_y = (height.saturating_sub(1)) as f32;

        for j in 0..TERRAIN_GRID_RES {
            let v = j as f32 / (TERRAIN_GRID_RES - 1) as f32;
            let fy = v * max_y;
            let py0 = (fy.floor() as usize).min(height.saturating_sub(1));
            let py1 = (py0 + 1).min(height.saturating_sub(1));
            let wy = fy - py0 as f32;

            for i in 0..TERRAIN_GRID_RES {
                let u = i as f32 / (TERRAIN_GRID_RES - 1) as f32;
                let fx = u * max_x;
                let px0 = (fx.floor() as usize).min(width.saturating_sub(1));
                let px1 = (px0 + 1).min(width.saturating_sub(1));
                let wx = fx - px0 as f32;

                let h00 = slice.pixels[py0 * width + px0];
                let h10 = slice.pixels[py0 * width + px1];
                let h01 = slice.pixels[py1 * width + px0];
                let h11 = slice.pixels[py1 * width + px1];

                // Sanitize invalid/no-data values
                let sanitize = |v: f32| if v.is_nan() || v < -10000.0 || v > 100000.0 { 0.0 } else { v };
                let h00 = sanitize(h00);
                let h10 = sanitize(h10);
                let h01 = sanitize(h01);
                let h11 = sanitize(h11);

                let top = h00 * (1.0 - wx) + h10 * wx;
                let bot = h01 * (1.0 - wx) + h11 * wx;
                let h = top * (1.0 - wy) + bot * wy;

                grid[j][i] = h;
                min_h = min_h.min(h);
                max_h = max_h.max(h);
            }
        }

        if min_h == f32::INFINITY { min_h = 0.0; }
        if max_h == f32::NEG_INFINITY { max_h = 0.0; }

        Ok(Self {
            coord,
            grid,
            min_height: min_h,
            max_height: max_h,
        })
    }

    /// Creates a decoded terrain tile from 256x256 Terrarium PNG image bytes
    pub fn from_image_bytes(coord: TileCoord, image_bytes: &[u8]) -> Result<Self, String> {
        let img = image::load_from_memory(image_bytes)
            .map_err(|e| format!("Failed to decode terrain PNG: {}", e))?;
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();

        let mut grid = [[0.0f32; TERRAIN_GRID_RES]; TERRAIN_GRID_RES];
        let mut min_h = f32::INFINITY;
        let mut max_h = f32::NEG_INFINITY;

        let max_x = (width.saturating_sub(1)) as f32;
        let max_y = (height.saturating_sub(1)) as f32;

        for j in 0..TERRAIN_GRID_RES {
            let v = j as f32 / (TERRAIN_GRID_RES - 1) as f32;
            let fy = v * max_y;
            let py0 = (fy.floor() as u32).min(width.saturating_sub(1));
            let py1 = (py0 + 1).min(height.saturating_sub(1));
            let wy = fy - py0 as f32;

            for i in 0..TERRAIN_GRID_RES {
                let u = i as f32 / (TERRAIN_GRID_RES - 1) as f32;
                let fx = u * max_x;
                let px0 = (fx.floor() as u32).min(width.saturating_sub(1));
                let px1 = (px0 + 1).min(width.saturating_sub(1));
                let wx = fx - px0 as f32;

                let p00 = rgb.get_pixel(px0, py0);
                let p10 = rgb.get_pixel(px1, py0);
                let p01 = rgb.get_pixel(px0, py1);
                let p11 = rgb.get_pixel(px1, py1);

                let h00 = decode_terrarium_height(p00[0], p00[1], p00[2]);
                let h10 = decode_terrarium_height(p10[0], p10[1], p10[2]);
                let h01 = decode_terrarium_height(p01[0], p01[1], p01[2]);
                let h11 = decode_terrarium_height(p11[0], p11[1], p11[2]);

                let top = h00 * (1.0 - wx) + h10 * wx;
                let bot = h01 * (1.0 - wx) + h11 * wx;
                let h = top * (1.0 - wy) + bot * wy;

                grid[j][i] = h;
                min_h = min_h.min(h);
                max_h = max_h.max(h);
            }
        }

        Ok(Self {
            coord,
            grid,
            min_height: min_h,
            max_height: max_h,
        })
    }

    /// Samples elevation at normalized UV coordinate [0..1, 0..1] within the tile with bilinear interpolation
    pub fn sample_uv(&self, u: f32, v: f32) -> f32 {
        let u_clamped = u.clamp(0.0, 1.0) * (TERRAIN_GRID_RES - 1) as f32;
        let v_clamped = v.clamp(0.0, 1.0) * (TERRAIN_GRID_RES - 1) as f32;

        let i0 = (u_clamped.floor() as usize).min(TERRAIN_GRID_RES - 1);
        let i1 = (i0 + 1).min(TERRAIN_GRID_RES - 1);
        let j0 = (v_clamped.floor() as usize).min(TERRAIN_GRID_RES - 1);
        let j1 = (j0 + 1).min(TERRAIN_GRID_RES - 1);

        let fx = u_clamped - i0 as f32;
        let fy = v_clamped - j0 as f32;

        let h00 = self.grid[j0][i0];
        let h10 = self.grid[j0][i1];
        let h01 = self.grid[j1][i0];
        let h11 = self.grid[j1][i1];

        let top = h00 * (1.0 - fx) + h10 * fx;
        let bot = h01 * (1.0 - fx) + h11 * fx;
        top * (1.0 - fy) + bot * fy
    }
}

/// Download result message sent from worker threads to main thread
pub enum TerrainDownloadResult {
    Success(Box<DecodedTerrainTile>),
    Failure(TileCoord),
}

/// Work queue for terrain tile download threads
pub struct TerrainWorkQueue {
    pub tasks: Vec<TileCoord>,
    pub valid_keys: HashSet<TileCoord>,
    pub in_flight: usize,
    pub unconsumed_results: usize,
    pub is_shutdown: bool,
    pub provider: TerrainProvider,
}

/// Manages multi-resolution 3D Terrain streaming, caching, and elevation queries
pub struct TerrainManager {
    pub is_enabled: bool,
    pub provider: TerrainProvider,
    pub height_exaggeration: f32,
    pub shading_mode: TerrainShadingMode,
    pub wireframe: bool,

    /// Cache of decoded terrain height tiles in memory
    pub tile_cache: HashMap<TileCoord, Arc<DecodedTerrainTile>>,

    // Download tracking
    pub requested_tiles: HashSet<TileCoord>,
    work_queue: Arc<Mutex<TerrainWorkQueue>>,
    work_condvar: Arc<Condvar>,
    #[allow(dead_code)]
    result_sender: Sender<TerrainDownloadResult>,
    result_receiver: Receiver<TerrainDownloadResult>,

    pub active_tiles_count: usize,
}

impl TerrainManager {
    pub const MAX_WORKERS: usize = 6;
    pub const MAX_PENDING_QUEUE: usize = 64;

    pub fn new() -> Self {
        let default_provider = TerrainProvider::EsriTerrain3D;
        let work_queue = Arc::new(Mutex::new(TerrainWorkQueue {
            tasks: Vec::new(),
            valid_keys: HashSet::new(),
            in_flight: 0,
            unconsumed_results: 0,
            is_shutdown: false,
            provider: default_provider,
        }));
        let work_condvar = Arc::new(Condvar::new());
        let (result_sender, result_receiver) = channel::<TerrainDownloadResult>();

        let manager = Self {
            is_enabled: true, // Default enabled for 3D terrain elevation
            provider: default_provider,
            height_exaggeration: 1.0,
            shading_mode: TerrainShadingMode::TexturedBasemap,
            wireframe: false,
            tile_cache: HashMap::new(),
            requested_tiles: HashSet::new(),
            work_queue: work_queue.clone(),
            work_condvar: work_condvar.clone(),
            result_sender: result_sender.clone(),
            result_receiver,
            active_tiles_count: 0,
        };

        #[cfg(not(target_arch = "wasm32"))]
        // Spawn background worker threads
        for _ in 0..Self::MAX_WORKERS {
            let queue_arc = work_queue.clone();
            let cond_arc = work_condvar.clone();
            let tx = result_sender.clone();

            thread::spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(std::time::Duration::from_secs(5))
                    .timeout_read(std::time::Duration::from_secs(8))
                    .user_agent("s3d-terrain/0.1.0 (Antigravity 3D GIS)")
                    .build();

                loop {
                    let (coord, provider) = {
                        let mut guard = match queue_arc.lock() {
                            Ok(g) => g,
                            Err(_) => break,
                        };

                        loop {
                            if guard.is_shutdown {
                                return;
                            }

                            if let Some(coord) = guard.tasks.pop() {
                                if guard.valid_keys.contains(&coord) {
                                    guard.in_flight += 1;
                                    let prov = guard.provider;
                                    break (coord, prov);
                                }
                            } else {
                                guard = match cond_arc.wait(guard) {
                                    Ok(g) => g,
                                    Err(_) => return,
                                };
                            }
                        }
                    };

                    // 1. Check persistent disk cache first (offline instant hit)
                    if let Some(cached_bytes) = crate::gis::cache::DiskCacheManager::read_terrain_tile(provider, coord) {
                        if let Ok(decoded) = provider.decode_tile(coord, &cached_bytes) {
                            let _ = tx.send(TerrainDownloadResult::Success(Box::new(decoded)));
                            if let Ok(mut guard) = queue_arc.lock() {
                                guard.in_flight = guard.in_flight.saturating_sub(1);
                                guard.unconsumed_results += 1;
                            }
                            continue;
                        }
                    }

                    // 2. Perform network request and caching
                    let url = provider.tile_url(coord);
                    let mut success = false;

                    if let Ok(resp) = agent.get(&url).call() {
                        let mut bytes = Vec::new();
                        if resp.into_reader().read_to_end(&mut bytes).is_ok() {
                            // Write raw bytes to persistent disk cache
                            crate::gis::cache::DiskCacheManager::write_terrain_tile(provider, coord, &bytes);

                            if let Ok(decoded) = provider.decode_tile(coord, &bytes) {
                                let _ = tx.send(TerrainDownloadResult::Success(Box::new(decoded)));
                                success = true;
                            }
                        }
                    }

                    if !success {
                        let _ = tx.send(TerrainDownloadResult::Failure(coord));
                    }

                    if let Ok(mut guard) = queue_arc.lock() {
                        guard.in_flight = guard.in_flight.saturating_sub(1);
                        guard.unconsumed_results += 1;
                    }
                }
            });
        }

        manager
    }

    /// Sets the active terrain elevation provider (e.g. Esri WorldElevation3D or AWS Terrarium)
    /// and resets cached height tiles.
    pub fn set_provider(&mut self, provider: TerrainProvider) {
        if self.provider == provider {
            return;
        }
        self.provider = provider;
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.provider = provider;
            queue.tasks.clear();
            queue.valid_keys.clear();
        }
        self.clear_cache();
    }

    pub const MAX_TERRAIN_ZOOM: u32 = 14;

    /// Enqueues required terrain tiles matching the active camera quadtree
    pub fn request_tiles(&mut self, required_tiles: &[TileCoord]) {
        if !self.is_enabled || required_tiles.is_empty() {
            return;
        }

        let mut queue = match self.work_queue.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Map all requested tiles (including high-zoom child tiles > 14) to valid DEM tile coordinates <= 14
        let mut target_tiles = HashSet::new();
        for &coord in required_tiles {
            let target = if coord.z > Self::MAX_TERRAIN_ZOOM {
                coord.ancestor(coord.z - Self::MAX_TERRAIN_ZOOM)
            } else {
                coord
            };
            target_tiles.insert(target);
        }

        // Update valid keys for cancellation of out-of-view tiles
        queue.valid_keys = target_tiles.clone();
        let valid_set = &queue.valid_keys;

        // Prune requested_tiles tracker
        self.requested_tiles.retain(|t| self.tile_cache.contains_key(t) || valid_set.contains(t));

        let available_slots = Self::MAX_PENDING_QUEUE.saturating_sub(queue.tasks.len());
        let mut newly_added = 0;

        for &coord in &target_tiles {
            if newly_added >= available_slots {
                break;
            }

            if !self.requested_tiles.contains(&coord) && !self.tile_cache.contains_key(&coord) {
                self.requested_tiles.insert(coord);
                queue.tasks.push(coord);
                newly_added += 1;
            }
        }

        if newly_added > 0 {
            // Sort so highest-zoom close tiles download first
            queue.tasks.sort_by_key(|t| t.z);
            self.work_condvar.notify_all();
        }

        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.result_sender.clone();
            let queue_arc = self.work_queue.clone();
            let provider = self.provider;
            const MAX_WASM_CONCURRENT: usize = 6;
            let mut tasks_to_dispatch = Vec::new();
            while queue.in_flight + tasks_to_dispatch.len() < MAX_WASM_CONCURRENT {
                if let Some(coord) = queue.tasks.pop() {
                    if queue.valid_keys.contains(&coord) {
                        tasks_to_dispatch.push(coord);
                    }
                } else {
                    break;
                }
            }
            queue.in_flight += tasks_to_dispatch.len();
            // Drop queue lock BEFORE initiating network requests
            drop(queue);

            for coord in tasks_to_dispatch {
                let tx_clone = tx.clone();
                let q_clone = queue_arc.clone();
                let url = provider.tile_url(coord);
                crate::gis::platform::http::fetch_bytes(&url, move |res| {
                    if let Ok(mut guard) = q_clone.lock() {
                        guard.in_flight = guard.in_flight.saturating_sub(1);
                    }
                    if let Ok(bytes) = res {
                        if let Ok(decoded) = provider.decode_tile(coord, &bytes) {
                            let _ = tx_clone.send(TerrainDownloadResult::Success(Box::new(decoded)));
                            return;
                        }
                    }
                    let _ = tx_clone.send(TerrainDownloadResult::Failure(coord));
                });
            }
        }
    }

    /// Drains completed terrain height tiles from worker threads into the cache
    pub fn drain_completed(&mut self) -> Vec<Arc<DecodedTerrainTile>> {
        let mut new_tiles = Vec::new();

        while let Ok(res) = self.result_receiver.try_recv() {
            match res {
                TerrainDownloadResult::Success(decoded) => {
                    let coord = decoded.coord;
                    let arc_tile = Arc::new(*decoded);
                    self.tile_cache.insert(coord, arc_tile.clone());
                    new_tiles.push(arc_tile);
                }
                TerrainDownloadResult::Failure(coord) => {
                    self.requested_tiles.remove(&coord);
                }
            }
        }

        if let Ok(mut guard) = self.work_queue.lock() {
            guard.unconsumed_results = 0;
        }

        self.active_tiles_count = self.tile_cache.len();
        new_tiles
    }

    /// Retrieves exact terrain height data or smoothly resamples from nearest ancestor elevation tile
    /// for high-zoom basemap tiles (e.g. z=15, 16, 17, 18, 19, 20)
    pub fn get_terrain_for_tile(&self, coord: TileCoord) -> Option<DecodedTerrainTile> {
        if !self.is_enabled {
            return None;
        }

        // 1. Direct hit in tile cache
        if let Some(tile) = self.tile_cache.get(&coord) {
            return Some((**tile).clone());
        }

        // 2. Find closest ancestor tile in cache (searching from min(z-1, 14) down to 0)
        let max_anc_z = coord.z.min(Self::MAX_TERRAIN_ZOOM);
        for anc_z in (0..=max_anc_z).rev() {
            let dz = coord.z.saturating_sub(anc_z);
            let anc_coord = coord.ancestor(dz);

            if let Some(anc_tile) = self.tile_cache.get(&anc_coord) {
                // Resample 33x33 height grid within ancestor tile's UV sub-rectangle
                let num_sub_tiles = (1u32 << dz) as f32;
                let x_offset = (coord.x & ((1 << dz) - 1)) as f32;
                let y_offset = (coord.y & ((1 << dz) - 1)) as f32;

                let mut grid = [[0.0f32; TERRAIN_GRID_RES]; TERRAIN_GRID_RES];
                let mut min_h = f32::INFINITY;
                let mut max_h = f32::NEG_INFINITY;

                for j in 0..TERRAIN_GRID_RES {
                    let sub_v = j as f32 / (TERRAIN_GRID_RES - 1) as f32;
                    let parent_v = (y_offset + sub_v) / num_sub_tiles;
                    for i in 0..TERRAIN_GRID_RES {
                        let sub_u = i as f32 / (TERRAIN_GRID_RES - 1) as f32;
                        let parent_u = (x_offset + sub_u) / num_sub_tiles;
                        let h = anc_tile.sample_uv(parent_u, parent_v);
                        grid[j][i] = h;
                        min_h = min_h.min(h);
                        max_h = max_h.max(h);
                    }
                }

                return Some(DecodedTerrainTile {
                    coord,
                    grid,
                    min_height: min_h,
                    max_height: max_h,
                });
            }
        }

        None
    }

    /// Samples elevation in meters at a given geographic coordinate (lat, lon) using O(1) tile lookup
    pub fn sample_elevation(&self, lat: f64, lon: f64) -> Option<f32> {
        if self.tile_cache.is_empty() {
            return None;
        }

        let lat_clamped = lat.clamp(-85.05112878, 85.05112878);
        let lat_rad = lat_clamped.to_radians();
        let tan_lat = lat_rad.tan();
        let cos_lat = lat_rad.cos();
        let merc_y = (1.0 - (tan_lat + (1.0 / cos_lat)).ln() / std::f64::consts::PI) * 0.5;
        let merc_x = (lon + 180.0) / 360.0;

        // Search from highest zoom (14) down to 0 with direct O(1) hash lookups
        for z in (0..=Self::MAX_TERRAIN_ZOOM).rev() {
            let n_tiles = (1u32 << z) as f64;
            let exact_x = (merc_x * n_tiles).clamp(0.0, n_tiles - 1e-7);
            let exact_y = (merc_y * n_tiles).clamp(0.0, n_tiles - 1e-7);

            let tile_x = exact_x.floor() as u32;
            let tile_y = exact_y.floor() as u32;

            let coord = TileCoord::new(z, tile_x, tile_y);
            if let Some(tile) = self.tile_cache.get(&coord) {
                let u = (exact_x - tile_x as f64) as f32;
                let v = (exact_y - tile_y as f64) as f32;
                return Some(tile.sample_uv(u, v) * self.height_exaggeration);
            }

            // Boundary fallback: if on edge of tile and primary tile is not in cache, check neighbor tile
            let u = (exact_x - tile_x as f64) as f32;
            let v = (exact_y - tile_y as f64) as f32;
            if u < 1e-4 && tile_x > 0 {
                let neighbor_coord = TileCoord::new(z, tile_x - 1, tile_y);
                if let Some(tile) = self.tile_cache.get(&neighbor_coord) {
                    return Some(tile.sample_uv(1.0, v) * self.height_exaggeration);
                }
            }
            if v < 1e-4 && tile_y > 0 {
                let neighbor_coord = TileCoord::new(z, tile_x, tile_y - 1);
                if let Some(tile) = self.tile_cache.get(&neighbor_coord) {
                    return Some(tile.sample_uv(u, 1.0) * self.height_exaggeration);
                }
            }
        }

        None
    }

    /// Clear all terrain cache
    pub fn clear_cache(&mut self) {
        self.tile_cache.clear();
        self.requested_tiles.clear();
        if let Ok(mut q) = self.work_queue.lock() {
            q.tasks.clear();
            q.valid_keys.clear();
            q.in_flight = 0;
            q.unconsumed_results = 0;
        }
        self.active_tiles_count = 0;
    }

    /// Returns true if terrain tiles are actively downloading or ready to consume
    pub fn is_streaming(&self) -> bool {
        if !self.is_enabled {
            return false;
        }
        if let Ok(q) = self.work_queue.lock() {
            q.in_flight > 0 || !q.tasks.is_empty() || q.unconsumed_results > 0
        } else {
            false
        }
    }

    /// Returns true if there are completed terrain tiles ready to consume
    pub fn has_unconsumed_completed(&self) -> bool {
        if let Ok(q) = self.work_queue.lock() {
            q.unconsumed_results > 0
        } else {
            false
        }
    }
}

impl Drop for TerrainManager {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.work_queue.lock() {
            queue.is_shutdown = true;
        }
        self.work_condvar.notify_all();
    }
}

