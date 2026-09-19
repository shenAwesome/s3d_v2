use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};

pub const WGS84_A: f64 = 6378137.0;
pub const WGS84_F: f64 = 1.0 / 298.257223563;
pub const WGS84_B: f64 = WGS84_A * (1.0 - WGS84_F);
pub const WGS84_E2: f64 = 2.0 * WGS84_F - WGS84_F * WGS84_F;
pub const WGS84_EP2: f64 = (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_B * WGS84_B);

// ----------------------------------------------------
// OGC 3D Tiles JSON Specification Types
// ----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefinementMode {
    Add,
    Replace,
}

impl Default for RefinementMode {
    fn default() -> Self {
        Self::Replace
    }
}

// Custom deserialization to handle field name "box" in JSON
impl<'de> Deserialize<'de> for BoundingVolumeInternal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawBV {
            #[serde(rename = "box")]
            box_obb: Option<[f64; 12]>,
            region: Option<[f64; 6]>,
            sphere: Option<[f64; 4]>,
        }
        let raw = RawBV::deserialize(deserializer)?;
        Ok(BoundingVolumeInternal {
            box_obb: raw.box_obb,
            region: raw.region,
            sphere: raw.sphere,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "BoundingVolumeInternal", into = "BoundingVolumeInternal")]
pub struct BoundingVolumeDef {
    pub box_obb: Option<[f64; 12]>,
    pub region: Option<[f64; 6]>,
    pub sphere: Option<[f64; 4]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoundingVolumeInternal {
    #[serde(rename = "box", skip_serializing_if = "Option::is_none")]
    pub box_obb: Option<[f64; 12]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<[f64; 6]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sphere: Option<[f64; 4]>,
}

impl From<BoundingVolumeInternal> for BoundingVolumeDef {
    fn from(i: BoundingVolumeInternal) -> Self {
        Self {
            box_obb: i.box_obb,
            region: i.region,
            sphere: i.sphere,
        }
    }
}

impl From<BoundingVolumeDef> for BoundingVolumeInternal {
    fn from(b: BoundingVolumeDef) -> Self {
        Self {
            box_obb: b.box_obb,
            region: b.region,
            sphere: b.sphere,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile3DContent {
    pub uri: Option<String>,
    pub url: Option<String>,
    pub bounding_volume: Option<BoundingVolumeDef>,
}

impl Tile3DContent {
    pub fn content_uri(&self) -> Option<&str> {
        self.uri.as_deref().or(self.url.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile3DNode {
    pub bounding_volume: BoundingVolumeDef,
    pub geometric_error: f64,
    #[serde(default)]
    pub refine: RefinementMode,
    pub transform: Option<[f64; 16]>,
    pub content: Option<Tile3DContent>,
    #[serde(default)]
    pub children: Vec<Tile3DNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TilesetAsset {
    pub version: String,
    pub tileset_version: Option<String>,
    pub gltf_up_axis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TilesetJson {
    pub asset: TilesetAsset,
    pub geometric_error: f64,
    pub root: Tile3DNode,
    pub properties: Option<serde_json::Value>,
    pub extensions_used: Option<Vec<String>>,
    pub extensions_required: Option<Vec<String>>,
}

impl TilesetJson {
    pub fn from_json_str(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse tileset.json: {}", e))
    }
}

// ----------------------------------------------------
// Coordinate Conversions & Bounding Volume Math
// ----------------------------------------------------

/// Converts Geodetic coordinates (Lat, Lon in deg, Alt in meters) to standard WGS84 ECEF coordinates (X, Y, Z in meters, Z is North Pole).
pub fn lat_lon_alt_to_standard_ecef(lat_deg: f64, lon_deg: f64, alt: f64) -> DVec3 {
    let lat_rad = lat_deg.to_radians();
    let lon_rad = lon_deg.to_radians();
    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let sin_lon = lon_rad.sin();
    let cos_lon = lon_rad.cos();
    let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    let x = (n + alt) * cos_lat * cos_lon;
    let y = (n + alt) * cos_lat * sin_lon;
    let z = (n * (1.0 - WGS84_E2) + alt) * sin_lat;
    DVec3::new(x, y, z)
}

/// Converts WGS84 ECEF coordinates (X, Y, Z in meters) to Geodetic coordinates (Lat, Lon in deg, Alt in meters).
/// Uses Bowring's method for sub-millimeter precision.
pub fn ecef_to_lat_lon_alt(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let p = (x * x + y * y).sqrt();
    if p < 1e-6 {
        let lat = if z >= 0.0 { 90.0 } else { -90.0 };
        let lon = 0.0;
        let alt = z.abs() - WGS84_B;
        return (lat, lon, alt);
    }

    let theta = (z * WGS84_A).atan2(p * WGS84_B);
    let sin_t = theta.sin();
    let cos_t = theta.cos();

    let lat_rad = (z + WGS84_EP2 * WGS84_B * sin_t * sin_t * sin_t)
        .atan2(p - WGS84_E2 * WGS84_A * cos_t * cos_t * cos_t);
    let lon_rad = y.atan2(x);

    let sin_lat = lat_rad.sin();
    let n = WGS84_A / (1.0f64 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    let alt = p / lat_rad.cos() - n;

    (lat_rad.to_degrees(), lon_rad.to_degrees(), alt)
}

/// Computes the center of a bounding volume in WGS84 ECEF coordinates [x, y, z] in meters.
pub fn get_bounding_volume_center_ecef(
    bv: &BoundingVolumeDef,
    transform: Option<&DMat4>,
) -> DVec3 {
    if let Some(reg) = &bv.region {
        // [west, south, east, north, minHeight, maxHeight] in radians/meters
        let lon_rad = (reg[0] + reg[2]) * 0.5;
        let lat_rad = (reg[1] + reg[3]) * 0.5;
        let alt = (reg[4] + reg[5]) * 0.5;
        return lat_lon_alt_to_standard_ecef(lat_rad.to_degrees(), lon_rad.to_degrees(), alt);
    }

    if let Some(b) = &bv.box_obb {
        let raw_center = DVec3::new(b[0], b[1], b[2]);
        return if let Some(m) = transform {
            m.transform_point3(raw_center)
        } else {
            raw_center
        };
    }

    if let Some(s) = &bv.sphere {
        let raw_center = DVec3::new(s[0], s[1], s[2]);
        return if let Some(m) = transform {
            m.transform_point3(raw_center)
        } else {
            raw_center
        };
    }

    DVec3::ZERO
}

/// Computes the bounding radius in meters for a bounding volume.
pub fn get_bounding_volume_radius(
    bv: &BoundingVolumeDef,
    transform: Option<&DMat4>,
) -> f64 {
    if let Some(reg) = &bv.region {
        let lon1 = reg[0].to_degrees();
        let lat1 = reg[1].to_degrees();
        let lon2 = reg[2].to_degrees();
        let lat2 = reg[3].to_degrees();
        let p1 = lat_lon_alt_to_standard_ecef(lat1, lon1, reg[4]);
        let p2 = lat_lon_alt_to_standard_ecef(lat2, lon2, reg[5]);
        return p1.distance(p2) * 0.5;
    }

    if let Some(b) = &bv.box_obb {
        let half_x = DVec3::new(b[3], b[4], b[5]);
        let half_y = DVec3::new(b[6], b[7], b[8]);
        let half_z = DVec3::new(b[9], b[10], b[11]);
        let scale = if let Some(m) = transform {
            let col0 = DVec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
            let col1 = DVec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
            let col2 = DVec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
            col0.max(col1).max(col2)
        } else {
            1.0
        };
        return (half_x.length_squared() + half_y.length_squared() + half_z.length_squared())
            .sqrt()
            * scale;
    }

    if let Some(s) = &bv.sphere {
        let scale = if let Some(m) = transform {
            let col0 = DVec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
            col0
        } else {
            1.0
        };
        return s[3] * scale;
    }

    10.0
}

/// Calculates Screen-Space Error (SSE) in pixels for a given tile node given camera parameters.
pub fn calculate_screen_space_error(
    geometric_error: f64,
    distance_to_camera: f64,
    screen_height_px: f32,
    fov_y_rad: f32,
) -> f32 {
    if distance_to_camera <= 1.0 || geometric_error <= 0.0 {
        return 0.0;
    }
    let denom = 2.0 * distance_to_camera * (fov_y_rad as f64 * 0.5).tan();
    if denom <= 1e-6 {
        return 0.0;
    }
    ((geometric_error * screen_height_px as f64) / denom) as f32
}

