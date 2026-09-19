use glam::Vec3;
use serde::{Deserialize, Serialize};

/// WGS84 Ellipsoid constants
pub const WGS84_A: f64 = 6378137.0; // Semi-major axis in meters
pub const WGS84_F: f64 = 1.0 / 298.257223563; // Flattening
pub const WGS84_E_SQ: f64 = 2.0 * WGS84_F - WGS84_F * WGS84_F; // First eccentricity squared

/// Geographic coordinates in WGS84
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct GeoCoord {
    /// Latitude in decimal degrees [-90, +90]
    pub latitude: f64,
    /// Longitude in decimal degrees [-180, +180]
    pub longitude: f64,
    /// Elevation above ellipsoid in meters
    pub elevation: f64,
}

impl GeoCoord {
    pub fn new(latitude: f64, longitude: f64, elevation: f64) -> Self {
        Self {
            latitude,
            longitude,
            elevation,
        }
    }

    /// Convert to degrees, minutes, seconds string (e.g., 37°48'50"S 144°57'47"E)
    pub fn to_dms_string(&self) -> String {
        fn to_dms(deg: f64) -> (u32, u32, f64) {
            let d = deg.abs();
            let degrees = d.floor() as u32;
            let minutes = ((d - degrees as f64) * 60.0).floor() as u32;
            let seconds = (d - degrees as f64 - minutes as f64 / 60.0) * 3600.0;
            (degrees, minutes, seconds)
        }

        let (lat_d, lat_m, lat_s) = to_dms(self.latitude);
        let lat_h = if self.latitude >= 0.0 { 'N' } else { 'S' };
        let (lon_d, lon_m, lon_s) = to_dms(self.longitude);
        let lon_h = if self.longitude >= 0.0 { 'E' } else { 'W' };

        format!(
            "{}°{:02}'{:04.1}\"{} {}°{:02}'{:04.1}\"{} ({:+.1}m)",
            lat_d, lat_m, lat_s, lat_h, lon_d, lon_m, lon_s, lon_h, self.elevation
        )
    }

    /// Calculate geodesic distance on the WGS84 ellipsoid to another point in meters using Haversine formula
    pub fn distance_to(&self, other: &GeoCoord) -> f64 {
        let lat1 = self.latitude.to_radians();
        let lon1 = self.longitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let lon2 = other.longitude.to_radians();

        let dlat = lat2 - lat1;
        let dlon = lon2 - lon1;

        let a = (dlat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        WGS84_A * c
    }
}

/// Project Coordinate Reference System with Local Project Origin
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProjectOrigin {
    pub origin: GeoCoord,
    pub re: f64, // Radius of curvature in prime vertical
    pub rn: f64, // Meridional radius of curvature
}

impl ProjectOrigin {
    pub fn from_geo(coord: GeoCoord) -> Self {
        Self::new(coord.latitude, coord.longitude, coord.elevation)
    }

    pub fn new(latitude: f64, longitude: f64, elevation: f64) -> Self {
        let origin = GeoCoord::new(latitude, longitude, elevation);
        let lat_rad = latitude.to_radians();
        let sin_lat = lat_rad.sin();
        let sin2_lat = sin_lat * sin_lat;

        let re = WGS84_A / (1.0 - WGS84_E_SQ * sin2_lat).sqrt();
        let rn = WGS84_A * (1.0 - WGS84_E_SQ) / (1.0 - WGS84_E_SQ * sin2_lat).powf(1.5);

        Self { origin, re, rn }
    }

    /// Convert WGS84 geographic coordinate to Local 3D Cartesian coordinates (Rigorous Topocentric ENU)
    /// In Engine Space (Right-Handed):
    /// +X = East
    /// +Y = Up (Elevation / Local Vertical)
    /// -Z = North (+Z = South)
    pub fn geo_to_local(&self, geo: &GeoCoord) -> Vec3 {
        let lat_rad = geo.latitude.to_radians();
        let lon_rad = geo.longitude.to_radians();
        let sin_lat = lat_rad.sin();
        let cos_lat = lat_rad.cos();
        let sin_lon = lon_rad.sin();
        let cos_lon = lon_rad.cos();

        let n = WGS84_A / (1.0 - WGS84_E_SQ * sin_lat * sin_lat).sqrt();
        let p_x = (n + geo.elevation) * cos_lat * cos_lon;
        let p_y = (n + geo.elevation) * cos_lat * sin_lon;
        let p_z = (n * (1.0 - WGS84_E_SQ) + geo.elevation) * sin_lat;

        let lat0_rad = self.origin.latitude.to_radians();
        let lon0_rad = self.origin.longitude.to_radians();
        let sin_lat0 = lat0_rad.sin();
        let cos_lat0 = lat0_rad.cos();
        let sin_lon0 = lon0_rad.sin();
        let cos_lon0 = lon0_rad.cos();

        let n0 = WGS84_A / (1.0 - WGS84_E_SQ * sin_lat0 * sin_lat0).sqrt();
        let o_x = (n0 + self.origin.elevation) * cos_lat0 * cos_lon0;
        let o_y = (n0 + self.origin.elevation) * cos_lat0 * sin_lon0;
        let o_z = (n0 * (1.0 - WGS84_E_SQ) + self.origin.elevation) * sin_lat0;

        let dx = p_x - o_x;
        let dy = p_y - o_y;
        let dz = p_z - o_z;

        let east = -sin_lon0 * dx + cos_lon0 * dy;
        let north = -sin_lat0 * cos_lon0 * dx - sin_lat0 * sin_lon0 * dy + cos_lat0 * dz;
        let up = cos_lat0 * cos_lon0 * dx + cos_lat0 * sin_lon0 * dy + sin_lat0 * dz;

        Vec3::new(east as f32, up as f32, -north as f32)
    }

    /// Convert 2D (lat, lon) at base elevation to Local 3D vector
    pub fn lat_lon_to_local(&self, lat: f64, lon: f64, elev: f64) -> Vec3 {
        self.geo_to_local(&GeoCoord::new(lat, lon, elev))
    }

    /// Convert Local 3D coordinates back to WGS84 Geodetic coordinate (Rigorous Inverse ENU -> ECEF -> Geodetic)
    pub fn local_to_geo(&self, local: Vec3) -> GeoCoord {
        let east = local.x as f64;
        let up = local.y as f64;
        let north = -(local.z as f64);

        let lat0_rad = self.origin.latitude.to_radians();
        let lon0_rad = self.origin.longitude.to_radians();
        let sin_lat0 = lat0_rad.sin();
        let cos_lat0 = lat0_rad.cos();
        let sin_lon0 = lon0_rad.sin();
        let cos_lon0 = lon0_rad.cos();

        let n0 = WGS84_A / (1.0 - WGS84_E_SQ * sin_lat0 * sin_lat0).sqrt();
        let o_x = (n0 + self.origin.elevation) * cos_lat0 * cos_lon0;
        let o_y = (n0 + self.origin.elevation) * cos_lat0 * sin_lon0;
        let o_z = (n0 * (1.0 - WGS84_E_SQ) + self.origin.elevation) * sin_lat0;

        // Inverse rotation: transpose of ECEF-to-ENU rotation matrix
        let dx = -sin_lon0 * east - sin_lat0 * cos_lon0 * north + cos_lat0 * cos_lon0 * up;
        let dy = cos_lon0 * east - sin_lat0 * sin_lon0 * north + cos_lat0 * sin_lon0 * up;
        let dz = cos_lat0 * north + sin_lat0 * up;

        let x_std = o_x + dx;
        let y_std = o_y + dy;
        let z_std = o_z + dz;

        // Convert standard ECEF [x_std, y_std, z_std] to Engine ECEF [x_std, z_std, -y_std]
        ecef_to_geodetic_d64([x_std, z_std, -y_std])
    }
}

/// Projection coordinate system mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProjectionMode {
    /// Local Topocentric East-North-Up (Planar Architectural Mode)
    #[default]
    PlanarENU,
    /// 3D Earth-Centered Earth-Fixed WGS84 Ellipsoid (3D Globe Mode)
    GlobeECEF,
}

impl ProjectionMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            ProjectionMode::PlanarENU => "🗺 Planar (Local ENU)",
            ProjectionMode::GlobeECEF => "🌐 3D Globe (ECEF)",
        }
    }
}

/// Convert WGS84 geodetic coordinate to 3D Cartesian ECEF (double-precision f64) in Engine Space (Y-Up convention)
/// +Y = North Pole
/// +X = Equator, Prime Meridian (0°N, 0°E)
/// -Z = Equator, 90°E
pub fn geodetic_to_ecef_d64(geo: &GeoCoord) -> [f64; 3] {
    let lat_rad = geo.latitude.to_radians();
    let lon_rad = geo.longitude.to_radians();

    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let sin_lon = lon_rad.sin();
    let cos_lon = lon_rad.cos();

    let n = WGS84_A / (1.0 - WGS84_E_SQ * sin_lat * sin_lat).sqrt();

    let x = (n + geo.elevation) * cos_lat * cos_lon;
    let y = (n * (1.0 - WGS84_E_SQ) + geo.elevation) * sin_lat;
    let z = -((n + geo.elevation) * cos_lat * sin_lon);

    [x, y, z]
}

/// Convert 3D Cartesian ECEF (double-precision f64) in Engine Space back to WGS84 Geodetic coordinate (Bowring's algorithm)
pub fn ecef_to_geodetic_d64(ecef: [f64; 3]) -> GeoCoord {
    let x_std = ecef[0];
    let y_std = -ecef[2];
    let z_std = ecef[1];

    let b = WGS84_A * (1.0 - WGS84_F);
    let e_prime_sq = (WGS84_A * WGS84_A - b * b) / (b * b);

    let p = (x_std * x_std + y_std * y_std).sqrt();
    let theta = (z_std * WGS84_A).atan2(p * b);

    let sin_theta = theta.sin();
    let cos_theta = theta.cos();

    let lat = (z_std + e_prime_sq * b * sin_theta.powi(3))
        .atan2(p - WGS84_E_SQ * WGS84_A * cos_theta.powi(3));
    let lon = y_std.atan2(x_std);

    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let n = WGS84_A / (1.0 - WGS84_E_SQ * sin_lat * sin_lat).sqrt();

    // Singularity-free elevation formula across entire ellipsoid including poles
    let alt = p * cos_lat + (z_std + WGS84_E_SQ * n * sin_lat) * sin_lat - n;

    GeoCoord::new(lat.to_degrees(), lon.to_degrees(), alt)
}

/// Convert WGS84 geodetic coordinate to 3D Cartesian ECEF Vec3 in Engine Space (Y-Up convention)
pub fn geodetic_to_ecef(geo: &GeoCoord) -> Vec3 {
    let [x, y, z] = geodetic_to_ecef_d64(geo);
    Vec3::new(x as f32, y as f32, z as f32)
}

/// Convert 3D Cartesian ECEF Vec3 in Engine Space back to WGS84 Geodetic coordinate
pub fn ecef_to_geodetic(ecef: Vec3) -> GeoCoord {
    ecef_to_geodetic_d64([ecef.x as f64, ecef.y as f64, ecef.z as f64])
}

/// Computes the outward geodetic unit surface normal at any given geographic coordinate
pub fn geodetic_surface_normal(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let lat_rad = lat_deg.to_radians();
    let lon_rad = lon_deg.to_radians();

    let cos_lat = lat_rad.cos() as f32;
    let sin_lat = lat_rad.sin() as f32;
    let cos_lon = lon_rad.cos() as f32;
    let sin_lon = lon_rad.sin() as f32;

    Vec3::new(cos_lat * cos_lon, sin_lat, -cos_lat * sin_lon).normalize()
}

/// Computes the exact camera (pitch, yaw) angles in radians to look directly down at a geographic coordinate on the Globe
pub fn geo_to_globe_camera_angles(geo: &GeoCoord) -> (f32, f32) {
    let ecef = geodetic_to_ecef(geo);
    let len = ecef.length();
    let pitch = (ecef.y / len).clamp(-0.9999, 0.9999).asin();
    let yaw = ecef.x.atan2(ecef.z);
    (pitch, yaw)
}

/// Web Mercator maximum valid latitude (where projection reaches singularity)
pub const WEB_MERCATOR_MAX_LAT: f64 = 85.0511287798066;

/// Convert WGS84 (latitude, longitude) in decimal degrees to Web Mercator (EPSG:3857) meters (x, y)
pub fn wgs84_to_web_mercator(lat_deg: f64, lon_deg: f64) -> (f64, f64) {
    let lat = lat_deg.clamp(-WEB_MERCATOR_MAX_LAT, WEB_MERCATOR_MAX_LAT);
    let x = WGS84_A * lon_deg.to_radians();
    let y = WGS84_A * ((std::f64::consts::PI / 4.0) + (lat.to_radians() / 2.0)).tan().ln();
    (x, y)
}

/// Convert Web Mercator (EPSG:3857) meters (x, y) to WGS84 (latitude, longitude) in decimal degrees
pub fn web_mercator_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    let lon = (x / WGS84_A).to_degrees();
    let lat = (2.0 * (y / WGS84_A).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
    (lat, lon)
}

/// Calculate geodesic distance between two WGS84 coordinates in meters using the Haversine formula
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    GeoCoord::new(lat1, lon1, 0.0).distance_to(&GeoCoord::new(lat2, lon2, 0.0))
}

