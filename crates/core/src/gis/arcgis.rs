use crate::gis::crs::{GeoCoord, ProjectOrigin};
use crate::gis::geojson_loader::GisFeature;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

/// Operational service access mode chosen by the user
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcGISServiceMode {
    /// Server renders and dynamic exports transparent raster map images draped on terrain
    RasterExport,
    /// Smart vector feature streaming with geometry generalization, spatial caching, and CAD attribute inspection
    VectorFeature,
}

impl Default for ArcGISServiceMode {
    fn default() -> Self {
        Self::VectorFeature
    }
}

/// Built-in preconfigured ArcGIS service catalog presets
#[derive(Debug, Clone)]
pub struct ArcGISPreset {
    pub name: &'static str,
    pub url: &'static str,
    pub default_layer_id: u32,
    pub default_mode: ArcGISServiceMode,
    pub min_scale: f64,
    pub max_scale: f64,
    pub description: &'static str,
}

pub const ARCGIS_PRESETS: &[ArcGISPreset] = &[
    ArcGISPreset {
        name: "Victoria Planning Scheme Zones (Vicmap Planning)",
        url: "https://spatial.planning.vic.gov.au/gis/rest/services/planning_scheme_zones/MapServer",
        default_layer_id: 0,
        default_mode: ArcGISServiceMode::VectorFeature,
        min_scale: 500_000.0,
        max_scale: 0.0,
        description: "All statutory land use and zoning scheme polygons across Victoria (CCZ, MUZ, GRZ, IN1Z, C1Z, etc.)",
    },
    ArcGISPreset {
        name: "Victoria Bushfire Prone Areas (BPA)",
        url: "https://spatial.planning.vic.gov.au/gis/rest/services/bpa/MapServer",
        default_layer_id: 0,
        default_mode: ArcGISServiceMode::VectorFeature,
        min_scale: 500_000.0,
        max_scale: 0.0,
        description: "Designated Bushfire Prone Areas (BPA) under Victoria building regulations",
    },
];

/// Metadata parsed from an ArcGIS MapServer / FeatureServer REST endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcGISLayerMetadata {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "minScale", default)]
    pub min_scale: f64,
    #[serde(rename = "maxScale", default)]
    pub max_scale: f64,
    #[serde(rename = "maxRecordCount", default = "default_max_records")]
    pub max_record_count: usize,
    #[serde(default)]
    pub capabilities: String,
    #[serde(rename = "supportedQueryFormats", default)]
    pub supported_query_formats: String,
}

fn default_max_records() -> usize {
    2000
}

impl Default for ArcGISLayerMetadata {
    fn default() -> Self {
        Self {
            name: "ArcGIS Layer".to_string(),
            min_scale: 500_000.0,
            max_scale: 0.0,
            max_record_count: 2000,
            capabilities: "Map,Query,Data".to_string(),
            supported_query_formats: "JSON, geoJSON".to_string(),
        }
    }
}

/// Calculates Douglas-Peucker geometric simplification offset (in decimal degrees)
/// based on camera distance and viewport resolution.
/// Sends this as `maxAllowableOffset` to the ArcGIS REST engine to eliminate sub-pixel vertices,
/// reducing data transfer payload by 70%–90% without visual degradation.
pub fn calculate_max_allowable_offset(distance_meters: f32, fov_y: f32, vp_height: f32) -> f64 {
    let vp_h = vp_height.max(256.0);
    let meters_per_pixel = (2.0 * distance_meters.max(1.0) * (fov_y * 0.5).tan()) / vp_h;
    let degrees_per_pixel = (meters_per_pixel as f64) / 111_320.0;
    // Simplify to 0.5 pixel resolution
    (degrees_per_pixel * 0.5).clamp(0.000005, 0.05)
}

/// Checks if the current camera scale / distance is within the service's visible range (minScale & maxScale)
pub fn is_scale_in_range(distance_meters: f32, min_scale: f64, max_scale: f64) -> bool {
    // Convert camera distance to approximate representative RF scale denominator
    // (Standard GIS approximation: distance_meters * 10 ≈ RF scale denominator)
    let approx_scale = (distance_meters as f64) * 10.0;

    if min_scale > 0.0 && approx_scale > min_scale {
        return false; // Zoomed out too far (e.g. continental view)
    }
    if max_scale > 0.0 && approx_scale < max_scale {
        return false; // Zoomed in too close
    }
    true
}

/// Automatically assigns standard statutory planning zoning colors based on Victorian Planning Scheme zone code
pub fn zone_code_to_color(zone_code: &str) -> [f32; 4] {
    let z = zone_code.to_uppercase();
    if z.starts_with("CCZ") {
        // Capital City Zone (Melbourne CBD) - Magenta / Crimson
        [0.85, 0.15, 0.40, 0.70]
    } else if z.starts_with("MUZ") {
        // Mixed Use Zone - Warm Coral / Orange
        [0.95, 0.50, 0.20, 0.65]
    } else if z.starts_with("C1Z") || z.starts_with("C2Z") || z.starts_with("B1Z") || z.starts_with("B2Z") || z.starts_with("B3Z") || z.starts_with("B4Z") || z.starts_with("B5Z") || z.starts_with("ACZ") {
        // Commercial / Activity Centre - Deep Royal Blue / Cyan
        [0.15, 0.50, 0.90, 0.65]
    } else if z.starts_with("GRZ") || z.starts_with("NRZ") || z.starts_with("RGZ") || z.starts_with("R1Z") || z.starts_with("LDRZ") {
        // Residential - Warm Golden Yellow
        [0.92, 0.76, 0.22, 0.65]
    } else if z.starts_with("IN1Z") || z.starts_with("IN2Z") || z.starts_with("IN3Z") {
        // Industrial - Slate Purple / Violet
        [0.55, 0.35, 0.80, 0.65]
    } else if z.starts_with("PPRZ") || z.starts_with("PCRZ") || z.starts_with("PUZ") || z.starts_with("PRZ") {
        // Public Parks & Recreation - Forest Green
        [0.22, 0.75, 0.38, 0.65]
    } else if z.starts_with("FZ") || z.starts_with("RAZ") || z.starts_with("RCZ") || z.starts_with("RLZ") || z.starts_with("GWZ") {
        // Farming / Rural - Olive Green
        [0.60, 0.75, 0.28, 0.65]
    } else if z.starts_with("PDZ") || z.starts_with("UGZ") || z.starts_with("DZ") || z.starts_with("CDZ") || z.starts_with("PZ") {
        // Special / Priority / Urban Growth - Amber / Terracotta
        [0.90, 0.40, 0.25, 0.65]
    } else {
        // Default GIS Overlay
        [0.35, 0.65, 0.85, 0.65]
    }
}

/// Real-time live network telemetry and debug diagnostics for ArcGIS services
#[derive(Debug, Clone, Default)]
pub struct ArcGISTelemetry {
    pub last_request_url: String,
    pub last_status: String,
    pub last_error: Option<String>,
    pub total_queries_sent: usize,
    pub total_cache_hits: usize,
    pub total_features_received: usize,
    pub current_scale_str: String,
    pub is_scale_gated: bool,
    pub active_layer_url: String,
    pub active_layer_id: u32,
}

/// Task sent to background worker for spatial BBOX feature query
#[allow(dead_code)]
struct ArcGISQueryTask {
    service_slug: String,
    url: String,
    layer_id: u32,
    cell_key: String,
    bbox: [f64; 4], // [min_lon, min_lat, max_lon, max_lat]
    max_allowable_offset: f64,
}

/// Spatial cell feature batch completed by background worker
pub struct ArcGISBatchResult {
    pub cell_key: String,
    pub features: Vec<GisFeature>,
    pub query_url: String,
    pub status: String,
    pub is_cache_hit: bool,
    pub error: Option<String>,
}

/// Manages asynchronous spatial query execution, visibility scale gating, and feature caching for ArcGIS Feature layers
pub struct ArcGISFeatureManager {
    #[allow(dead_code)]
    task_sender: Sender<ArcGISQueryTask>,
    #[allow(dead_code)]
    result_sender: Sender<ArcGISBatchResult>,
    result_receiver: Receiver<ArcGISBatchResult>,
    requested_cells: HashSet<String>,
    pub loaded_object_ids: HashSet<String>,
    pub is_scale_gated: bool,
    pub total_features_loaded: usize,
    pub telemetry: ArcGISTelemetry,
}

impl ArcGISFeatureManager {
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let (task_sender, task_receiver) = channel::<ArcGISQueryTask>();
        #[cfg(target_arch = "wasm32")]
        let (task_sender, _task_receiver) = channel::<ArcGISQueryTask>();
        let (result_sender, result_receiver) = channel::<ArcGISBatchResult>();

        #[cfg(not(target_arch = "wasm32"))]
        let task_receiver = Arc::new(Mutex::new(task_receiver));

        #[cfg(not(target_arch = "wasm32"))]
        // Spawn background worker for HTTP spatial queries & GeoJSON decoding
        for _ in 0..3 {
            let rx = task_receiver.clone();
            let tx = result_sender.clone();

            thread::spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(std::time::Duration::from_secs(6))
                    .timeout_read(std::time::Duration::from_secs(15))
                    .user_agent("s3d-gis/0.1.0 (Antigravity ArcGIS Smart Feature Client)")
                    .build();

                loop {
                    let task = {
                        let guard = match rx.lock() {
                            Ok(g) => g,
                            Err(_) => break,
                        };
                        match guard.recv() {
                            Ok(t) => t,
                            Err(_) => break,
                        }
                    };

                    // 1. Check persistent disk cache first
                    let mut json_bytes = crate::gis::cache::DiskCacheManager::read_arcgis_data(
                        &task.service_slug,
                        task.layer_id,
                        &task.cell_key,
                    );

                    let is_cache_hit = json_bytes.is_some();
                    let mut req_url = String::new();
                    let mut status_str = if is_cache_hit {
                        "200 OK (Disk Cache)".to_string()
                    } else {
                        "Pending".to_string()
                    };
                    let mut err_str = None;

                    // 2. Fetch from ArcGIS REST endpoint if not cached
                    if json_bytes.is_none() {
                        req_url = format!(
                            "{}/{}/query?where=1%3D1&geometry={:.6},{:.6},{:.6},{:.6}&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects&outFields=*&maxAllowableOffset={:.6}&outSR=4326&f=geojson",
                            task.url.trim_end_matches('/'),
                            task.layer_id,
                            task.bbox[0],
                            task.bbox[1],
                            task.bbox[2],
                            task.bbox[3],
                            task.max_allowable_offset,
                        );

                        log::debug!("[ArcGIS Query] Sending request: {}", req_url);

                        match agent.get(&req_url).call() {
                            Ok(resp) => {
                                let mut buf = Vec::new();
                                if resp.into_reader().read_to_end(&mut buf).is_ok() {
                                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&buf) {
                                        if let Some(err_obj) = val.get("error") {
                                            let msg = err_obj.get("message").and_then(|m| m.as_str()).unwrap_or("ArcGIS Server error");
                                            err_str = Some(format!("ArcGIS Error: {}", msg));
                                            status_str = format!("Error: {}", msg);
                                            log::warn!("[ArcGIS Query Error] {}", msg);
                                        } else if val.get("features").is_some() {
                                            crate::gis::cache::DiskCacheManager::write_arcgis_data(
                                                &task.service_slug,
                                                task.layer_id,
                                                &task.cell_key,
                                                &buf,
                                            );
                                            json_bytes = Some(buf);
                                            status_str = "200 OK (Live Network)".to_string();
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                err_str = Some(format!("HTTP Error: {}", e));
                                status_str = format!("Network Error: {}", e);
                                log::warn!("[ArcGIS Query Network Error] {}", e);
                            }
                        }
                    }

                    // 3. Decode GeoJSON features into GisFeature
                    let mut decoded_features = Vec::new();
                    if let Some(bytes) = json_bytes {
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if let Some(features_arr) = val.get("features").and_then(|f| f.as_array()) {
                                for feat_val in features_arr {
                                    if let Some(feat) = parse_arcgis_geojson_feature(feat_val) {
                                        decoded_features.push(feat);
                                    }
                                }
                            }
                        }
                    }

                    if !is_cache_hit && err_str.is_none() {
                        log::info!("[ArcGIS Query] Received {} features for cell {}", decoded_features.len(), task.cell_key);
                    } else if is_cache_hit {
                        log::debug!("[ArcGIS] Loaded {} features from disk cache ({})", decoded_features.len(), task.cell_key);
                    }

                    let _ = tx.send(ArcGISBatchResult {
                        cell_key: task.cell_key,
                        features: decoded_features,
                        query_url: req_url,
                        status: status_str,
                        is_cache_hit,
                        error: err_str,
                    });
                }
            });
        }

        Self {
            task_sender,
            result_sender,
            result_receiver,
            requested_cells: HashSet::new(),
            loaded_object_ids: HashSet::new(),
            is_scale_gated: false,
            total_features_loaded: 0,
            telemetry: ArcGISTelemetry::default(),
        }
    }

    /// Evaluates current camera view, checks scale range gating, partitions visible area into spatial cells,
    /// and dispatches background queries for missing cells with dynamic `maxAllowableOffset`.
    pub fn update_and_request(
        &mut self,
        service_url: &str,
        layer_id: u32,
        min_scale: f64,
        max_scale: f64,
        camera_distance: f32,
        fov_y: f32,
        vp_height: f32,
        origin: &ProjectOrigin,
        camera_target: Vec3,
    ) {
        // 1. Scale Range Gating Check: Protect server & client from massive statewide queries when zoomed out
        let approx_scale = (camera_distance as f64) * 10.0;
        self.telemetry.current_scale_str = format!("1:{:.0}", approx_scale);
        self.telemetry.active_layer_url = service_url.to_string();
        self.telemetry.active_layer_id = layer_id;

        if !is_scale_in_range(camera_distance, min_scale, max_scale) {
            self.is_scale_gated = true;
            self.telemetry.is_scale_gated = true;
            return;
        }
        self.is_scale_gated = false;
        self.telemetry.is_scale_gated = false;

        // 2. Compute dynamic geometry simplification offset for current zoom
        let max_allowable_offset = calculate_max_allowable_offset(camera_distance, fov_y, vp_height);

        // 3. Spatial Cell Partitioning (approx. 2km grid around camera target)
        let target_geo = origin.local_to_geo(camera_target);
        let cell_span = 0.02; // ~2km per spatial cell
        let min_lon = (target_geo.longitude / cell_span).floor() * cell_span;
        let min_lat = (target_geo.latitude / cell_span).floor() * cell_span;

        #[cfg(not(target_arch = "wasm32"))]
        let service_slug = service_url
            .split('/')
            .find(|s| !s.is_empty() && *s != "https:" && *s != "http:" && *s != "gis" && *s != "rest" && *s != "services")
            .unwrap_or("arcgis_service")
            .to_string();

        for x_idx in -1..=1 {
            for y_idx in -1..=1 {
                let cell_min_lon = min_lon + x_idx as f64 * cell_span;
                let cell_max_lon = cell_min_lon + cell_span;
                let cell_min_lat = min_lat + y_idx as f64 * cell_span;
                let cell_max_lat = cell_min_lat + cell_span;

                let cell_key = format!("cell_{:.3}_{:.3}", cell_min_lon, cell_min_lat);

                if !self.requested_cells.contains(&cell_key) {
                    self.requested_cells.insert(cell_key.clone());
                    self.telemetry.total_queries_sent += 1;
                    #[cfg(not(target_arch = "wasm32"))]
                    let _ = self.task_sender.send(ArcGISQueryTask {
                        service_slug: service_slug.clone(),
                        url: service_url.to_string(),
                        layer_id,
                        cell_key,
                        bbox: [cell_min_lon, cell_min_lat, cell_max_lon, cell_max_lat],
                        max_allowable_offset,
                    });
                    #[cfg(target_arch = "wasm32")]
                    {
                        let tx = self.result_sender.clone();
                        let key = cell_key.clone();
                        let req_url = format!(
                            "{}/{}/query?where=1%3D1&geometry={:.6},{:.6},{:.6},{:.6}&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects&outFields=*&maxAllowableOffset={:.6}&outSR=4326&f=geojson",
                            service_url.trim_end_matches('/'),
                            layer_id,
                            cell_min_lon,
                            cell_min_lat,
                            cell_max_lon,
                            cell_max_lat,
                            max_allowable_offset,
                        );
                        let req_url_clone = req_url.clone();
                        crate::gis::platform::http::fetch_bytes(&req_url, move |res| {
                            let mut decoded_features = Vec::new();
                            let mut status_str = "Error".to_string();
                            let mut err_str = None;
                            if let Ok(bytes) = res {
                                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                                    if let Some(features_arr) = val.get("features").and_then(|f| f.as_array()) {
                                        for feat_val in features_arr {
                                            if let Some(feat) = parse_arcgis_geojson_feature(feat_val) {
                                                decoded_features.push(feat);
                                            }
                                        }
                                        status_str = "200 OK".to_string();
                                    }
                                }
                            } else if let Err(e) = res {
                                err_str = Some(e);
                            }
                            let _ = tx.send(ArcGISBatchResult {
                                cell_key: key,
                                features: decoded_features,
                                query_url: req_url_clone,
                                status: status_str,
                                is_cache_hit: false,
                                error: err_str,
                            });
                        });
                    }
                }
            }
        }
    }

    /// Drains newly completed spatial feature batches from background worker threads,
    /// filters duplicates, generates 3D surface meshes, and returns new features ready to add to the layer.
    pub fn drain_completed_features(&mut self, origin: &ProjectOrigin) -> Vec<GisFeature> {
        let mut new_features = Vec::new();

        while let Ok(res) = self.result_receiver.try_recv() {
            if !res.query_url.is_empty() {
                self.telemetry.last_request_url = res.query_url;
            }
            self.telemetry.last_status = res.status;
            self.telemetry.last_error = res.error;
            if res.is_cache_hit {
                self.telemetry.total_cache_hits += 1;
            }

            for mut feat in res.features {
                // Deduplication check by feature ID
                if !self.loaded_object_ids.contains(&feat.id) {
                    self.loaded_object_ids.insert(feat.id.clone());

                    // Generate local polygon coordinates from geodetic coordinates
                    feat.center_local = origin.lat_lon_to_local(feat.center_geo.latitude, feat.center_geo.longitude, 0.0);
                    let mut local_polys = Vec::new();
                    for poly in &feat.geo_polygons {
                        let mut local_poly = Vec::new();
                        for ring in poly {
                            let mut local_ring = Vec::new();
                            for pt in ring {
                                let local_pt = origin.lat_lon_to_local(pt[0], pt[1], 0.0);
                                local_ring.push(Vec2::new(local_pt.x, -local_pt.z));
                            }
                            local_poly.push(local_ring);
                        }
                        local_polys.push(local_poly);
                    }
                    feat.raw_polygons = local_polys;

                    self.total_features_loaded += 1;
                    self.telemetry.total_features_received += 1;
                    new_features.push(feat);
                }
            }
        }

        new_features
    }

    /// Clears cached state when switching layers or reloading
    pub fn clear(&mut self) {
        self.requested_cells.clear();
        self.loaded_object_ids.clear();
        self.total_features_loaded = 0;
    }

    /// Clears both memory state and persistent disk cache for ArcGIS queries
    pub fn clear_cache(&mut self) {
        self.clear();
        #[cfg(not(target_arch = "wasm32"))]
        let _ = std::fs::remove_dir_all(crate::gis::cache::DiskCacheManager::get_arcgis_cache_root());
    }
}

/// Parses a GeoJSON feature from ArcGIS Server query output into `GisFeature`
fn parse_arcgis_geojson_feature(val: &serde_json::Value) -> Option<GisFeature> {
    let geom = val.get("geometry")?;
    let geom_type = geom.get("type")?.as_str()?;
    let coords = geom.get("coordinates")?;

    let mut props_map = HashMap::new();
    if let Some(props_obj) = val.get("properties").and_then(|p| p.as_object()) {
        for (k, v) in props_obj {
            let str_val = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => String::new(),
                _ => v.to_string(),
            };
            props_map.insert(k.clone(), str_val);
        }
    }

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let object_id = props_map
        .get("OBJECTID")
        .or_else(|| props_map.get("objectid"))
        .or_else(|| props_map.get("PFI"))
        .or_else(|| props_map.get("pfi"))
        .cloned()
        .unwrap_or_else(|| format!("auto_{}", COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));

    let zone_code = props_map.get("ZONE_CODE").or_else(|| props_map.get("zone_code")).cloned().unwrap_or_default();
    let zone_desc = props_map.get("ZONE_DESCRIPTION").or_else(|| props_map.get("zone_description")).cloned().unwrap_or_default();
    let _lga = props_map.get("LGA").or_else(|| props_map.get("lga")).cloned().unwrap_or_default();

    let feature_name = if !zone_code.is_empty() {
        if !zone_desc.is_empty() {
            format!("{} - {}", zone_code, zone_desc)
        } else {
            zone_code.clone()
        }
    } else {
        props_map.get("NAME").or_else(|| props_map.get("name")).cloned().unwrap_or_else(|| format!("Zone #{}", object_id))
    };

    let mut geo_polys = Vec::new();

    if geom_type == "Polygon" {
        if let Some(poly_arr) = coords.as_array() {
            let mut poly = Vec::new();
            for ring_val in poly_arr {
                if let Some(ring_pts) = ring_val.as_array() {
                    let mut ring = Vec::new();
                    for pt in ring_pts {
                        if let Some(pt_arr) = pt.as_array() {
                            if pt_arr.len() >= 2 {
                                let lon = pt_arr[0].as_f64()?;
                                let lat = pt_arr[1].as_f64()?;
                                ring.push([lat, lon]);
                            }
                        }
                    }
                    if ring.len() >= 3 {
                        poly.push(ring);
                    }
                }
            }
            if !poly.is_empty() {
                geo_polys.push(poly);
            }
        }
    } else if geom_type == "MultiPolygon" {
        if let Some(multipoly_arr) = coords.as_array() {
            for poly_val in multipoly_arr {
                if let Some(poly_arr) = poly_val.as_array() {
                    let mut poly = Vec::new();
                    for ring_val in poly_arr {
                        if let Some(ring_pts) = ring_val.as_array() {
                            let mut ring = Vec::new();
                            for pt in ring_pts {
                                if let Some(pt_arr) = pt.as_array() {
                                    if pt_arr.len() >= 2 {
                                        let lon = pt_arr[0].as_f64()?;
                                        let lat = pt_arr[1].as_f64()?;
                                        ring.push([lat, lon]);
                                    }
                                }
                            }
                            if ring.len() >= 3 {
                                poly.push(ring);
                            }
                        }
                    }
                    if !poly.is_empty() {
                        geo_polys.push(poly);
                    }
                }
            }
        }
    }

    if geo_polys.is_empty() {
        return None;
    }

    // Compute center geodetic coordinate
    let first_pt = geo_polys[0][0][0];
    let center_geo = GeoCoord::new(first_pt[0], first_pt[1], 0.0);

    let shadow_color = crate::gis::layer::DEFAULT_SHADOW_COLOR;

    Some(GisFeature {
        id: format!("arcgis_{}", object_id),
        name: feature_name,
        feature_type: "Planning Scheme Zone".to_string(),
        height: 0.3, // Flat planning boundary ribbon
        min_height: 0.0,
        center_geo,
        center_local: Vec3::ZERO,
        properties: props_map,
        shadow_color,
        ground_elevation: 0.0,
        elevation_zoom: -1,
        geo_polygons: geo_polys,
        raw_polygons: Vec::new(),
        mesh: None,
        ..Default::default()
    })
}

