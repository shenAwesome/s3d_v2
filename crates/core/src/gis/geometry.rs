use serde::{Deserialize, Serialize};

// ─── Spatial Reference ───────────────────────────────────────────────────────

/// Spatial reference system for coordinate interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialReference {
    /// WGS 84 geographic (EPSG:4326)
    Wgs84,
    /// Web Mercator (EPSG:3857)
    WebMercator,
    /// MGA Zone 55 / GDA2020 (EPSG:7855)
    Mga55,
    /// VicGrid94 / GDA94 (EPSG:3111)
    VicGrid94,
    /// Scene-local East-North-Up meters
    LocalEnu,
    /// Arbitrary EPSG code
    Custom(u32),
}

impl Default for SpatialReference {
    fn default() -> Self {
        Self::Wgs84
    }
}

// ─── Point ───────────────────────────────────────────────────────────────────

/// 3D point geometry.
///
/// Coordinates follow standard GIS convention:
/// - `x`: easting / longitude
/// - `y`: northing / latitude  
/// - `z`: elevation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
}

impl Default for Point {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
}

impl Point {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Creates a Point from geographic coordinates (latitude, longitude, elevation).
    pub fn from_geo(lat: f64, lon: f64, elev: f64) -> Self {
        Self { x: lon, y: lat, z: elev }
    }

    /// Creates a 2D point (z = 0).
    pub fn xy(x: f64, y: f64) -> Self {
        Self { x, y, z: 0.0 }
    }
}

// ─── Polyline ────────────────────────────────────────────────────────────────

/// Multi-path polyline geometry.
///
/// Each path is an ordered sequence of 3D vertices `[x, y, z]`.
/// Multiple paths allow for disjoint line segments in a single geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Polyline {
    pub paths: Vec<Vec<[f64; 3]>>,
}

impl Polyline {
    pub fn new(paths: Vec<Vec<[f64; 3]>>) -> Self {
        Self { paths }
    }

    /// Creates a single-path polyline from a list of 3D vertices.
    pub fn from_path(path: Vec<[f64; 3]>) -> Self {
        Self { paths: vec![path] }
    }

    /// Total number of vertices across all paths.
    pub fn vertex_count(&self) -> usize {
        self.paths.iter().map(|p| p.len()).sum()
    }
}

// ─── Polygon ─────────────────────────────────────────────────────────────────

/// Closed polygon with exterior ring and optional holes.
///
/// Each ring is a list of `[x, y]` coordinate pairs. The first ring is the
/// exterior boundary; subsequent rings are interior holes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Polygon {
    pub rings: Vec<Vec<[f64; 2]>>,
    /// Whether this polygon carries per-vertex Z values.
    #[serde(default)]
    pub has_z: bool,
    /// Per-vertex Z values, parallel to `rings` structure.
    #[serde(default)]
    pub z_values: Vec<Vec<f64>>,
}

impl Polygon {
    /// Creates a polygon from rings (first = exterior, rest = holes).
    pub fn from_rings(rings: Vec<Vec<[f64; 2]>>) -> Self {
        Self { rings, has_z: false, z_values: Vec::new() }
    }

    /// Creates a single-ring polygon.
    pub fn from_ring(ring: Vec<[f64; 2]>) -> Self {
        Self { rings: vec![ring], has_z: false, z_values: Vec::new() }
    }

    /// Returns the number of rings (exterior + holes).
    pub fn ring_count(&self) -> usize {
        self.rings.len()
    }
}

// ─── Extent ──────────────────────────────────────────────────────────────────

/// Axis-aligned 3D bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Extent {
    pub xmin: f64,
    pub ymin: f64,
    pub zmin: f64,
    pub xmax: f64,
    pub ymax: f64,
    pub zmax: f64,
}

impl Default for Extent {
    fn default() -> Self {
        Self {
            xmin: f64::INFINITY, ymin: f64::INFINITY, zmin: f64::INFINITY,
            xmax: f64::NEG_INFINITY, ymax: f64::NEG_INFINITY, zmax: f64::NEG_INFINITY,
        }
    }
}

impl Extent {
    pub fn new(xmin: f64, ymin: f64, zmin: f64, xmax: f64, ymax: f64, zmax: f64) -> Self {
        Self { xmin, ymin, zmin, xmax, ymax, zmax }
    }

    /// Returns the center point of this extent.
    pub fn center(&self) -> Point {
        Point::new(
            (self.xmin + self.xmax) * 0.5,
            (self.ymin + self.ymax) * 0.5,
            (self.zmin + self.zmax) * 0.5,
        )
    }

    /// Width along X axis.
    pub fn width(&self) -> f64 {
        self.xmax - self.xmin
    }

    /// Height along Y axis.
    pub fn height(&self) -> f64 {
        self.ymax - self.ymin
    }

    /// Depth along Z axis.
    pub fn depth(&self) -> f64 {
        self.zmax - self.zmin
    }

    /// Tests whether a point falls inside this extent.
    pub fn contains_point(&self, p: &Point) -> bool {
        p.x >= self.xmin && p.x <= self.xmax
            && p.y >= self.ymin && p.y <= self.ymax
            && p.z >= self.zmin && p.z <= self.zmax
    }

    /// Tests whether two extents overlap.
    pub fn intersects(&self, other: &Extent) -> bool {
        self.xmin <= other.xmax && self.xmax >= other.xmin
            && self.ymin <= other.ymax && self.ymax >= other.ymin
            && self.zmin <= other.zmax && self.zmax >= other.zmin
    }

    /// Expands this extent to include a point.
    pub fn expand_to_point(&mut self, p: &Point) {
        self.xmin = self.xmin.min(p.x);
        self.ymin = self.ymin.min(p.y);
        self.zmin = self.zmin.min(p.z);
        self.xmax = self.xmax.max(p.x);
        self.ymax = self.ymax.max(p.y);
        self.zmax = self.zmax.max(p.z);
    }
}

// ─── Mesh ────────────────────────────────────────────────────────────────────

/// Triangle mesh geometry for 3D objects (I3S, 3D Tiles, custom meshes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    #[serde(default)]
    pub uv: Vec<[f32; 2]>,
    #[serde(default)]
    pub vertex_colors: Vec<[f32; 4]>,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            uv: Vec::new(),
            vertex_colors: Vec::new(),
        }
    }
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Computes the AABB extent of the mesh positions.
    pub fn extent(&self) -> Extent {
        let mut ext = Extent::default();
        for p in &self.positions {
            ext.expand_to_point(&Point::new(p[0] as f64, p[1] as f64, p[2] as f64));
        }
        ext
    }
}

/// Convert from legacy `RawMeshData` to new `Mesh`.
impl From<crate::gis::extrusion::RawMeshData> for Mesh {
    fn from(raw: crate::gis::extrusion::RawMeshData) -> Self {
        Self {
            positions: raw.positions,
            normals: raw.normals,
            indices: raw.indices,
            uv: raw.uvs,
            vertex_colors: raw.colors,
        }
    }
}

/// Convert from new `Mesh` back to legacy `RawMeshData`.
impl From<Mesh> for crate::gis::extrusion::RawMeshData {
    fn from(mesh: Mesh) -> Self {
        Self {
            positions: mesh.positions,
            normals: mesh.normals,
            indices: mesh.indices,
            uvs: mesh.uv,
            colors: mesh.vertex_colors,
        }
    }
}

// ─── Unified Geometry Enum ───────────────────────────────────────────────────

/// Unified geometry enum supporting all S3D geometry types.
///
/// Matches the Esri ArcGIS pattern of geometry types:
/// `Point`, `Polyline`, `Polygon`, `Mesh`, `Extent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Geometry {
    #[serde(rename = "point")]
    Point(Point),
    #[serde(rename = "polyline")]
    Polyline(Polyline),
    #[serde(rename = "polygon")]
    Polygon(Polygon),
    #[serde(rename = "mesh")]
    Mesh(Mesh),
    #[serde(rename = "extent")]
    Extent(Extent),
}

impl Geometry {
    /// Returns the geometry type as a static string.
    pub fn geometry_type(&self) -> &'static str {
        match self {
            Self::Point(_) => "point",
            Self::Polyline(_) => "polyline",
            Self::Polygon(_) => "polygon",
            Self::Mesh(_) => "mesh",
            Self::Extent(_) => "extent",
        }
    }

    /// Computes the axis-aligned bounding box of this geometry.
    pub fn extent(&self) -> Extent {
        match self {
            Self::Point(p) => Extent::new(p.x, p.y, p.z, p.x, p.y, p.z),
            Self::Polyline(pl) => {
                let mut ext = Extent::default();
                for path in &pl.paths {
                    for v in path {
                        ext.expand_to_point(&Point::new(v[0], v[1], v[2]));
                    }
                }
                ext
            }
            Self::Polygon(pg) => {
                let mut ext = Extent::default();
                for ring in &pg.rings {
                    for v in ring {
                        ext.expand_to_point(&Point::new(v[0], v[1], 0.0));
                    }
                }
                ext
            }
            Self::Mesh(m) => m.extent(),
            Self::Extent(e) => *e,
        }
    }
}

