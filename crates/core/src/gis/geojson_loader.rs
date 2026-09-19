use crate::compute::parallel::*;
use crate::gis::crs::{GeoCoord, ProjectOrigin};
use crate::gis::extrusion::{extrude_polygon, RawMeshData};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GisFeature {
    pub id: String,
    pub name: String,
    pub feature_type: String,
    pub height: f32,
    pub min_height: f32,
    pub center_geo: GeoCoord,
    pub center_local: Vec3,
    pub properties: HashMap<String, String>,
    pub shadow_color: [f32; 4],
    #[serde(default)]
    pub color: Option<[f32; 4]>,
    #[serde(default = "default_true")]
    pub edge_enabled: bool,
    #[serde(default)]
    pub stroke_color: Option<[f32; 4]>,
    #[serde(default)]
    pub stroke_width: Option<f32>,
    #[serde(default)]
    pub ground_elevation: f32,
    #[serde(default = "default_elevation_zoom")]
    pub elevation_zoom: i32,
    /// Polygon rings stored in **geodetic coordinates** (lat, lon) — the canonical
    /// source-of-truth that survives any `ProjectOrigin` change (e.g. globe→planar transition).
    /// Each entry is one polygon; each polygon is a list of rings; each ring is a list of [lat, lon].
    #[serde(default)]
    pub geo_polygons: Vec<Vec<Vec<[f64; 2]>>>,
    /// Cached local-space rings re-projected from `geo_polygons` at the *current* scene origin.
    /// Rebuilt by `rebuild_layer_feature_meshes` whenever the origin changes.
    #[serde(default)]
    pub raw_polygons: Vec<Vec<Vec<Vec2>>>,
    #[serde(default)]
    pub mesh: Option<RawMeshData>,
    #[serde(default)]
    pub geometry: Option<crate::gis::feature::Geometry>,
    #[serde(default)]
    pub symbol: Option<crate::gis::feature::PointSymbol3D>,
}

fn default_true() -> bool {
    true
}

impl Default for GisFeature {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            feature_type: String::new(),
            height: 0.0,
            min_height: 0.0,
            center_geo: GeoCoord::default(),
            center_local: Vec3::ZERO,
            properties: HashMap::new(),
            shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
            color: None,
            edge_enabled: true,
            stroke_color: None,
            stroke_width: None,
            ground_elevation: 0.0,
            elevation_zoom: -1,
            geo_polygons: Vec::new(),
            raw_polygons: Vec::new(),
            mesh: None,
            geometry: None,
            symbol: None,
        }
    }
}

impl GisFeature {
    /// Creates a 3D mesh feature with an ID, name, mesh data, and RGBA color
    pub fn new_mesh_feature(
        id: impl Into<String>,
        name: impl Into<String>,
        mesh: RawMeshData,
        color: [f32; 4],
    ) -> Self {
        let mut min_pt = Vec3::splat(f32::INFINITY);
        let mut max_pt = Vec3::splat(f32::NEG_INFINITY);
        for p in &mesh.positions {
            let v = Vec3::from_array(*p);
            min_pt = min_pt.min(v);
            max_pt = max_pt.max(v);
        }
        let center = if mesh.positions.is_empty() {
            Vec3::ZERO
        } else {
            (min_pt + max_pt) * 0.5
        };
        let height = if mesh.positions.is_empty() {
            0.0
        } else {
            max_pt.y - min_pt.y
        };

        Self {
            id: id.into(),
            name: name.into(),
            feature_type: "Building".to_string(),
            height,
            min_height: min_pt.y.max(0.0),
            center_geo: GeoCoord::default(),
            center_local: center,
            properties: HashMap::new(),
            shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
            color: Some(color),
            edge_enabled: true,
            stroke_color: Some([0.15, 0.18, 0.22, 1.0]),
            stroke_width: Some(1.5),
            ground_elevation: 0.0,
            elevation_zoom: 16,
            geo_polygons: Vec::new(),
            raw_polygons: Vec::new(),
            mesh: Some(mesh.clone()),
            geometry: Some(crate::gis::feature::Geometry::Mesh(mesh)),
            symbol: None,
        }
    }

    /// Creates a 3D box feature from min/max bounds
    pub fn create_box(
        id: impl Into<String>,
        name: impl Into<String>,
        min: Vec3,
        max: Vec3,
        color: [f32; 4],
    ) -> Self {
        let mesh = RawMeshData::create_box(min, max);
        Self::new_mesh_feature(id, name, mesh, color)
    }
}

fn default_elevation_zoom() -> i32 {
    -1
}

#[derive(Debug, Clone)]
pub struct LoadedDataset {
    pub origin: ProjectOrigin,
    pub features: Vec<GisFeature>,
    pub min_geo: GeoCoord,
    pub max_geo: GeoCoord,
}

pub fn parse_geojson_file(path: &str, custom_origin: Option<ProjectOrigin>) -> Result<LoadedDataset, String> {
    parse_geojson_file_with_progress(path, custom_origin, |_, _| {})
}

pub fn parse_geojson_file_with_progress<F>(
    path: &str,
    custom_origin: Option<ProjectOrigin>,
    progress: F,
) -> Result<LoadedDataset, String>
where
    F: Fn(f32, &str),
{
    progress(0.02, "Reading GeoJSON file from disk...");
    #[cfg(not(target_arch = "wasm32"))]
    {
        let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file {}: {}", path, e))?;
        parse_geojson_with_progress(&content, custom_origin, progress)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, custom_origin);
        Err("Direct disk file reading not supported on web".to_string())
    }
}

pub fn parse_geojson(geojson_str: &str, custom_origin: Option<ProjectOrigin>) -> Result<LoadedDataset, String> {
    parse_geojson_with_progress(geojson_str, custom_origin, |_, _| {})
}

pub fn parse_geojson_with_progress<F>(
    geojson_str: &str,
    custom_origin: Option<ProjectOrigin>,
    progress: F,
) -> Result<LoadedDataset, String>
where
    F: Fn(f32, &str),
{
    progress(0.05, "Parsing GeoJSON syntax...");
    let geojson: geojson::GeoJson = geojson_str.parse().map_err(|e| format!("Failed to parse GeoJSON: {}", e))?;

    let feature_collection = match geojson {
        geojson::GeoJson::FeatureCollection(fc) => fc,
        geojson::GeoJson::Feature(f) => geojson::FeatureCollection {
            bbox: None,
            features: vec![f],
            foreign_members: None,
        },
        geojson::GeoJson::Geometry(g) => geojson::FeatureCollection {
            bbox: None,
            features: vec![geojson::Feature {
                bbox: None,
                geometry: Some(g),
                id: None,
                properties: None,
                foreign_members: None,
            }],
            foreign_members: None,
        },
    };

    if feature_collection.features.is_empty() {
        return Err("GeoJSON contains no features".to_string());
    }

    // 1. Calculate bounding box of all points to establish project origin if not provided
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut min_lon = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;

    for feature in &feature_collection.features {
        if let Some(geom) = &feature.geometry {
            collect_bounds(&geom.value, &mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon);
        }
    }

    if min_lat > max_lat || min_lon > max_lon {
        return Err("No valid coordinate positions found in GeoJSON".to_string());
    }

    let origin = custom_origin.unwrap_or_else(|| {
        let center_lat = (min_lat + max_lat) / 2.0;
        let center_lon = (min_lon + max_lon) / 2.0;
        ProjectOrigin::new(center_lat, center_lon, 0.0)
    });

    let total_features = feature_collection.features.len();
    progress(0.20, &format!("Triangulating {} 3D features in parallel...", total_features));

    let gis_features: Vec<GisFeature> = feature_collection
        .features
        .into_par_iter()
        .enumerate()
        .map(|(index, feature)| {
            let mut props_map = HashMap::new();
            let mut height = 12.0f32; // Default 12m (approx 3-4 storeys)
            let mut min_height = 0.0f32;
            let mut name = format!("Feature {}", index + 1);
            let mut feature_type = "Building".to_string();
            let mut shadow_color = crate::gis::layer::DEFAULT_SHADOW_COLOR;

            if let Some(props) = &feature.properties {
                for (k, v) in props {
                    let v_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => v.to_string(),
                    };

                    let k_lower = k.to_lowercase();
                    if k_lower == "height" || k_lower == "extrusion" || k_lower == "render_height" || k_lower == "building:height" || k_lower == "bldg_height" {
                        if let Ok(h) = v_str.parse::<f32>() {
                            height = h;
                        }
                    } else if k_lower == "min_height" || k_lower == "building:min_height" {
                        if let Ok(mh) = v_str.parse::<f32>() {
                            min_height = mh;
                        }
                    } else if k_lower == "levels" || k_lower == "building:levels" || k_lower == "floors_above_ground" || k_lower == "num_floors" {
                        if let Ok(lvl) = v_str.parse::<f32>() {
                            height = lvl * 3.2; // 3.2m per level
                        }
                    } else if k_lower == "name" || k_lower == "address" || k_lower == "bldg_name" {
                        name = v_str.clone();
                    } else if k_lower == "development_key" && name.starts_with("Feature ") {
                        name = format!("Development {}", v_str);
                    } else if k_lower == "building" || k_lower == "type" || k_lower == "status" {
                        feature_type = v_str.clone();
                    } else if k_lower == "shadow_color" || k_lower == "shadow:color" {
                        if let Some(col) = parse_color_str(&v_str) {
                            shadow_color = col;
                        }
                    }

                    props_map.insert(k.clone(), v_str);
                }
            }

            let feature_id = feature
                .id
                .map(|id| match id {
                    geojson::feature::Id::String(s) => s,
                    geojson::feature::Id::Number(n) => n.to_string(),
                })
                .unwrap_or_else(|| format!("feat_{}", index + 1));

            let mut polygons_rings: Vec<Vec<Vec<Vec2>>> = Vec::new();
            let mut geo_polygons: Vec<Vec<Vec<[f64; 2]>>> = Vec::new();
            let mut combined_mesh = RawMeshData::new();

            if let Some(geometry) = feature.geometry {
                // Collect raw geodetic rings first (lat, lon) — origin-independent source of truth
                match geometry.value {
                    geojson::Value::Polygon(poly_coords) => {
                        let mut rings = Vec::new();
                        for ring in poly_coords {
                            let geo_ring: Vec<[f64; 2]> = ring
                                .iter()
                                .filter(|pt| pt.len() >= 2)
                                .map(|pt| [pt[1], pt[0]]) // [lat, lon]
                                .collect();
                            if geo_ring.len() >= 3 {
                                rings.push(geo_ring);
                            }
                        }
                        if !rings.is_empty() {
                            geo_polygons.push(rings);
                        }
                    }
                    geojson::Value::MultiPolygon(mp_coords) => {
                        for poly in mp_coords {
                            let mut rings = Vec::new();
                            for ring in poly {
                                let geo_ring: Vec<[f64; 2]> = ring
                                    .iter()
                                    .filter(|pt| pt.len() >= 2)
                                    .map(|pt| [pt[1], pt[0]]) // [lat, lon]
                                    .collect();
                                if geo_ring.len() >= 3 {
                                    rings.push(geo_ring);
                                }
                            }
                            if !rings.is_empty() {
                                geo_polygons.push(rings);
                            }
                        }
                    }
                    _ => {}
                }

                // Project geodetic rings to local ENU using the dataset origin
                polygons_rings = geo_polygons
                    .iter()
                    .map(|poly| {
                        poly.iter()
                            .map(|ring| {
                                ring.iter()
                                    .map(|&[lat, lon]| {
                                        let local_pos = origin.lat_lon_to_local(lat, lon, 0.0);
                                        Vec2::new(local_pos.x, -local_pos.z) // x = East, y = North = -z
                                    })
                                    .collect()
                            })
                            .collect()
                    })
                    .collect();

                // Combine extruded meshes for this feature
                for poly_rings in &polygons_rings {
                    if !poly_rings.is_empty() && poly_rings[0].len() >= 3 {
                        if let Some(mesh) = extrude_polygon(poly_rings, min_height, height - min_height) {
                            combined_mesh.append(mesh);
                        }
                    }
                }
            }

            let (center_local, center_geo) = if !combined_mesh.positions.is_empty() {
                let mut sum = Vec3::ZERO;
                for p in &combined_mesh.positions {
                    sum += Vec3::new(p[0], p[1], p[2]);
                }
                let c_loc = sum / (combined_mesh.positions.len() as f32);
                let c_geo = origin.local_to_geo(c_loc);
                (c_loc, c_geo)
            } else {
                (Vec3::ZERO, origin.origin)
            };

            let mesh_opt = if !combined_mesh.positions.is_empty() {
                Some(combined_mesh)
            } else {
                None
            };

            GisFeature {
                id: feature_id,
                name,
                feature_type,
                height,
                min_height,
                center_geo,
                center_local,
                properties: props_map,
                shadow_color,
                ground_elevation: 0.0,
                elevation_zoom: -1,
                geo_polygons,
                raw_polygons: polygons_rings,
                mesh: mesh_opt,
                ..Default::default()
            }
        })
        .collect();

    progress(0.95, &format!("Triangulated {} 3D features", total_features));

    Ok(LoadedDataset {
        origin,
        features: gis_features,
        min_geo: GeoCoord::new(min_lat, min_lon, 0.0),
        max_geo: GeoCoord::new(max_lat, max_lon, 0.0),
    })
}

fn parse_color_str(s: &str) -> Option<[f32; 4]> {
    let s = s.trim();
    if s.starts_with('#') {
        let hex = s.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
            return Some([r, g, b, 1.0]);
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0;
            return Some([r, g, b, a]);
        }
    }
    None
}

fn collect_bounds(value: &geojson::Value, min_lat: &mut f64, max_lat: &mut f64, min_lon: &mut f64, max_lon: &mut f64) {
    match value {
        geojson::Value::Point(pt) => {
            if pt.len() >= 2 {
                *min_lon = min_lon.min(pt[0]);
                *max_lon = max_lon.max(pt[0]);
                *min_lat = min_lat.min(pt[1]);
                *max_lat = max_lat.max(pt[1]);
            }
        }
        geojson::Value::MultiPoint(pts) | geojson::Value::LineString(pts) => {
            for pt in pts {
                if pt.len() >= 2 {
                    *min_lon = min_lon.min(pt[0]);
                    *max_lon = max_lon.max(pt[0]);
                    *min_lat = min_lat.min(pt[1]);
                    *max_lat = max_lat.max(pt[1]);
                }
            }
        }
        geojson::Value::MultiLineString(lines) | geojson::Value::Polygon(lines) => {
            for line in lines {
                for pt in line {
                    if pt.len() >= 2 {
                        *min_lon = min_lon.min(pt[0]);
                        *max_lon = max_lon.max(pt[0]);
                        *min_lat = min_lat.min(pt[1]);
                        *max_lat = max_lat.max(pt[1]);
                    }
                }
            }
        }
        geojson::Value::MultiPolygon(polys) => {
            for poly in polys {
                for line in poly {
                    for pt in line {
                        if pt.len() >= 2 {
                            *min_lon = min_lon.min(pt[0]);
                            *max_lon = max_lon.max(pt[0]);
                            *min_lat = min_lat.min(pt[1]);
                            *max_lat = max_lat.max(pt[1]);
                        }
                    }
                }
            }
        }
        geojson::Value::GeometryCollection(geoms) => {
            for g in geoms {
                collect_bounds(&g.value, min_lat, max_lat, min_lon, max_lon);
            }
        }
    }
}

