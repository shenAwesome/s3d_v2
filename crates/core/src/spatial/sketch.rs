use crate::gis::crs::ProjectOrigin;
use crate::gis::extrusion::{extrude_polygon_with_elevation, signed_ring_area_2d};
use crate::gis::geojson_loader::GisFeature;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointMarkerStyle {
    Beacon,
    Pin,
    Tree,
    Pillar,
    Flag,
    Diamond,
    Cone,
    Pyramid,
    Sphere,
    Cylinder,
    Cross,
    Star,
    ArrowDown,
}

impl Default for PointMarkerStyle {
    fn default() -> Self {
        Self::Beacon
    }
}

impl PointMarkerStyle {
    pub const ALL: &'static [PointMarkerStyle] = &[
        PointMarkerStyle::Beacon,
        PointMarkerStyle::Pin,
        PointMarkerStyle::Tree,
        PointMarkerStyle::Pillar,
        PointMarkerStyle::Flag,
        PointMarkerStyle::Diamond,
        PointMarkerStyle::Cone,
        PointMarkerStyle::Pyramid,
        PointMarkerStyle::Sphere,
        PointMarkerStyle::Cylinder,
        PointMarkerStyle::Cross,
        PointMarkerStyle::Star,
        PointMarkerStyle::ArrowDown,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Beacon => "Beacon",
            Self::Pin => "Pin",
            Self::Tree => "Tree",
            Self::Pillar => "Pillar",
            Self::Flag => "Flag",
            Self::Diamond => "Diamond",
            Self::Cone => "Cone",
            Self::Pyramid => "Pyramid",
            Self::Sphere => "Sphere",
            Self::Cylinder => "Cylinder",
            Self::Cross => "Cross",
            Self::Star => "Star",
            Self::ArrowDown => "ArrowDown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Beacon => "🗼 Beacon (Pillar + Sphere)",
            Self::Pin => "📍 Pin (Map Needle)",
            Self::Tree => "🌲 Tree (Canopy & Trunk)",
            Self::Pillar => "🏛 Pillar (Square Monolith)",
            Self::Flag => "🚩 Flag (Mast + Pennant)",
            Self::Diamond => "💎 Diamond (Crystal Gem)",
            Self::Cone => "🔺 Cone (Survey Spire)",
            Self::Pyramid => "▲ Pyramid (Monument)",
            Self::Sphere => "🔮 Sphere (Orb Pedestal)",
            Self::Cylinder => "🛢 Cylinder (Column Drum)",
            Self::Cross => "➕ Cross (Survey Target)",
            Self::Star => "⭐ Star (3D Star Emblem)",
            Self::ArrowDown => "⬇ Arrow (Waypoint Pointer)",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Pin" => Self::Pin,
            "Tree" => Self::Tree,
            "Pillar" => Self::Pillar,
            "Flag" => Self::Flag,
            "Diamond" => Self::Diamond,
            "Cone" => Self::Cone,
            "Pyramid" => Self::Pyramid,
            "Sphere" => Self::Sphere,
            "Cylinder" => Self::Cylinder,
            "Cross" => Self::Cross,
            "Star" => Self::Star,
            "ArrowDown" => Self::ArrowDown,
            _ => Self::Beacon,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SketchToolMode {
    Select,
    Point,
    Line,
    Polygon,
    Rectangle,
    Circle,
}

impl Default for SketchToolMode {
    fn default() -> Self {
        Self::Select
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchEngine {
    pub active: bool,
    pub mode: SketchToolMode,
    pub points: Vec<Vec3>,
    pub building_name: String,
    pub height: f32,
    pub building_type: String,
    pub color_tint: [f32; 4],
    pub shadow_color: [f32; 4],
    pub ground_elevation: f32,
    pub num_circle_segments: usize,
    // Point tool settings
    pub point_style: PointMarkerStyle,
    // Line tool settings
    pub line_width: f32,
    pub wall_height: f32,
    // Polygon surface zone toggle
    pub is_surface_zone: bool,
}

impl Default for SketchEngine {
    fn default() -> Self {
        Self {
            active: true,
            mode: SketchToolMode::Select,
            points: Vec::new(),
            building_name: String::new(),
            height: 25.0,
            building_type: "Custom 3D Object".to_string(),
            color_tint: [0.55, 0.80, 0.95, 1.0], // Modern Glass Blue
            shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
            ground_elevation: 0.0,
            num_circle_segments: 24,
            point_style: PointMarkerStyle::Beacon,
            line_width: 3.5,
            wall_height: 3.0,
            is_surface_zone: false,
        }
    }
}

impl SketchEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_mode(&mut self, mode: SketchToolMode) {
        self.mode = mode;
        self.points.clear();
        self.active = true;
        match mode {
            SketchToolMode::Select => {}
            SketchToolMode::Point => {
                if self.height <= 0.0 {
                    self.height = 12.0;
                }
            }
            SketchToolMode::Line => {
                self.ground_elevation = 0.0;
            }
            SketchToolMode::Polygon | SketchToolMode::Rectangle | SketchToolMode::Circle => {
                if self.height <= 0.0 {
                    self.height = 25.0;
                }
            }
        }
    }

    pub fn add_point(&mut self, pt: Vec3) {
        // Proximity guard: prevent duplicate vertex if double-clicked at the exact same location
        if let Some(last) = self.points.last() {
            if (last.x - pt.x).abs() < 0.25 && (last.z - pt.z).abs() < 0.25 {
                return;
            }
        }

        match self.mode {
            SketchToolMode::Select => {}
            SketchToolMode::Point => {
                self.points = vec![pt];
            }
            SketchToolMode::Line => {
                self.points.push(pt);
            }
            SketchToolMode::Polygon => {
                self.points.push(pt);
            }
            SketchToolMode::Rectangle => {
                if self.points.len() < 2 {
                    self.points.push(pt);
                } else {
                    self.points = vec![pt];
                }
            }
            SketchToolMode::Circle => {
                if self.points.len() < 2 {
                    self.points.push(pt);
                } else {
                    self.points = vec![pt];
                }
            }
        }
    }

    pub fn undo_point(&mut self) -> bool {
        if !self.points.is_empty() {
            self.points.pop();
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.building_name.clear();
    }

    pub fn is_ready_to_commit(&self) -> bool {
        match self.mode {
            SketchToolMode::Select => false,
            SketchToolMode::Point => !self.points.is_empty(),
            SketchToolMode::Line => self.points.len() >= 2,
            SketchToolMode::Polygon => self.points.len() >= 3,
            SketchToolMode::Rectangle => self.points.len() == 2,
            SketchToolMode::Circle => self.points.len() == 2,
        }
    }

    /// Calculates continuous polyline length in meters along all sketched vertices
    pub fn calculate_path_length(&self, cursor_hover: Option<Vec3>) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        let mut total = 0.0f32;
        for i in 0..self.points.len().saturating_sub(1) {
            total += (self.points[i + 1] - self.points[i]).length();
        }
        if let (Some(hover), Some(last)) = (cursor_hover, self.points.last()) {
            total += (hover - *last).length();
        }
        total
    }

    /// Generate 2D footprint ring in local meters (x = East, y = North = -z in 3D).
    /// Optional `cursor_hover` adds a live rubber-band preview segment.
    pub fn generate_footprint_polygon(&self, cursor_hover: Option<Vec3>) -> Vec<Vec2> {
        match self.mode {
            SketchToolMode::Select | SketchToolMode::Point | SketchToolMode::Line => Vec::new(),
            SketchToolMode::Polygon => {
                let mut ring: Vec<Vec2> = self.points.iter().map(|p| Vec2::new(p.x, -p.z)).collect();
                if let Some(hover) = cursor_hover {
                    if !self.points.is_empty() {
                        ring.push(Vec2::new(hover.x, -hover.z));
                    }
                }
                ring
            }
            SketchToolMode::Rectangle => {
                if self.points.is_empty() {
                    Vec::new()
                } else if self.points.len() == 1 {
                    if let Some(p1) = cursor_hover {
                        let p0 = self.points[0];
                        let (min_x, max_x) = if p0.x < p1.x { (p0.x, p1.x) } else { (p1.x, p0.x) };
                        let (min_y, max_y) = if -p0.z < -p1.z { (-p0.z, -p1.z) } else { (-p1.z, -p0.z) };
                        vec![
                            Vec2::new(min_x, min_y),
                            Vec2::new(max_x, min_y),
                            Vec2::new(max_x, max_y),
                            Vec2::new(min_x, max_y),
                        ]
                    } else {
                        vec![Vec2::new(self.points[0].x, -self.points[0].z)]
                    }
                } else {
                    let p0 = self.points[0];
                    let p1 = self.points[1];
                    let (min_x, max_x) = if p0.x < p1.x { (p0.x, p1.x) } else { (p1.x, p0.x) };
                    let (min_y, max_y) = if -p0.z < -p1.z { (-p0.z, -p1.z) } else { (-p1.z, -p0.z) };
                    vec![
                        Vec2::new(min_x, min_y),
                        Vec2::new(max_x, min_y),
                        Vec2::new(max_x, max_y),
                        Vec2::new(min_x, max_y),
                    ]
                }
            }
            SketchToolMode::Circle => {
                if self.points.is_empty() {
                    Vec::new()
                } else {
                    let center = self.points[0];
                    let center_2d = Vec2::new(center.x, -center.z);
                    let radius = if self.points.len() >= 2 {
                        let p1 = self.points[1];
                        let diff_x = p1.x - center.x;
                        let diff_z = p1.z - center.z;
                        (diff_x * diff_x + diff_z * diff_z).sqrt().max(1.0)
                    } else if let Some(hover) = cursor_hover {
                        let diff_x = hover.x - center.x;
                        let diff_z = hover.z - center.z;
                        (diff_x * diff_x + diff_z * diff_z).sqrt().max(1.0)
                    } else {
                        10.0
                    };

                    let segs = self.num_circle_segments.max(8);
                    let mut ring = Vec::with_capacity(segs);
                    for i in 0..segs {
                        let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
                        let x = center_2d.x + radius * theta.cos();
                        let y = center_2d.y + radius * theta.sin();
                        ring.push(Vec2::new(x, y));
                    }
                    ring
                }
            }
        }
    }

    /// Calculate footprint area in square meters (m²)
    pub fn calculate_footprint_area(&self, cursor_hover: Option<Vec3>) -> f32 {
        let ring = self.generate_footprint_polygon(cursor_hover);
        if ring.len() < 3 {
            return 0.0;
        }
        signed_ring_area_2d(&ring).abs() as f32
    }

    /// Calculate perimeter length in meters
    pub fn calculate_perimeter(&self, cursor_hover: Option<Vec3>) -> f32 {
        let ring = self.generate_footprint_polygon(cursor_hover);
        if ring.len() < 2 {
            return 0.0;
        }
        let mut peri = 0.0f32;
        let n = ring.len();
        for i in 0..n {
            let next = (i + 1) % n;
            peri += (ring[next] - ring[i]).length();
        }
        peri
    }

    /// Estimated Gross Floor Area (GFA = Footprint Area * Number of Storeys)
    pub fn calculate_gfa(&self, cursor_hover: Option<Vec3>) -> f32 {
        let area = self.calculate_footprint_area(cursor_hover);
        let storeys = (self.height / 3.5).round().max(1.0);
        area * storeys
    }

    /// Number of estimated storeys based on 3.5m floor-to-floor height
    pub fn estimated_storeys(&self) -> u32 {
        (self.height / 3.5).round().max(1.0) as u32
    }

    /// Builds a completed 3D GisFeature from the current sketch footprint/path/point and parameters
    pub fn build_feature(&self, origin: &ProjectOrigin, feature_id: &str) -> Option<GisFeature> {
        match self.mode {
            SketchToolMode::Select => None,
            SketchToolMode::Point => {
                if self.points.is_empty() {
                    return None;
                }
                let pt = self.points[0];
                let center_local = Vec3::new(pt.x, self.ground_elevation + self.height * 0.5, pt.z);
                let center_geo = origin.local_to_geo(center_local);
                let geo_pt = origin.local_to_geo(pt);
                let geo_polygons = vec![vec![vec![[geo_pt.latitude, geo_pt.longitude]]]];

                let mesh = self.build_point_mesh(pt);
                let custom_label = self.building_name.trim().to_string();
                let display_name = if custom_label.is_empty() {
                    "📍 Point Marker".to_string()
                } else {
                    custom_label.clone()
                };

                let mut properties = HashMap::new();
                properties.insert("Layer".to_string(), "✏️ Custom Sketches".to_string());
                properties.insert("label".to_string(), custom_label.clone());
                properties.insert("Label".to_string(), custom_label.clone());
                properties.insert("Feature Name".to_string(), display_name.clone());
                properties.insert("Feature Type".to_string(), "3D Marker".to_string());
                properties.insert("Marker Style".to_string(), format!("{:?}", self.point_style));
                properties.insert("Marker Altitude".to_string(), format!("{:.1} m", self.height));
                properties.insert("Ground Elevation".to_string(), format!("{:.1} m", self.ground_elevation));
                properties.insert("Latitude".to_string(), format!("{:.6}°", center_geo.latitude));
                properties.insert("Longitude".to_string(), format!("{:.6}°", center_geo.longitude));

                let point_raw_ring = vec![Vec2::new(pt.x, -pt.z)];

                let mut point_mesh = mesh;
                point_mesh.colors = vec![self.color_tint; point_mesh.positions.len()];

                Some(GisFeature {
                    id: feature_id.to_string(),
                    name: display_name,
                    feature_type: "3D Marker".to_string(),
                    height: self.height,
                    min_height: self.ground_elevation,
                    center_geo,
                    center_local,
                    properties,
                    shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
                    color: Some(self.color_tint),
                    edge_enabled: true,
                    stroke_color: None,
                    stroke_width: None,
                    ground_elevation: self.ground_elevation,
                    elevation_zoom: -1,
                    geo_polygons,
                    raw_polygons: vec![vec![point_raw_ring]],
                    mesh: Some(point_mesh),
                    geometry: None,
                    symbol: None,
                })
            }
            SketchToolMode::Line => {
                if self.points.len() < 2 {
                    return None;
                }
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_z = f32::INFINITY;
                let mut max_z = f32::NEG_INFINITY;
                for p in &self.points {
                    min_x = min_x.min(p.x);
                    max_x = max_x.max(p.x);
                    min_z = min_z.min(p.z);
                    max_z = max_z.max(p.z);
                }
                let total_len = self.calculate_path_length(None);
                let eff_wall_h = if self.wall_height <= 0.05 { 0.08 } else { self.wall_height };
                let center_local = Vec3::new((min_x + max_x) * 0.5, self.ground_elevation + eff_wall_h * 0.5, (min_z + max_z) * 0.5);
                let center_geo = origin.local_to_geo(center_local);

                let geo_points: Vec<[f64; 2]> = self.points.iter().map(|p| {
                    let geo = origin.local_to_geo(*p);
                    [geo.latitude, geo.longitude]
                }).collect();
                let geo_polygons = vec![vec![geo_points]];

                let mesh = self.build_line_mesh();
                let custom_label = self.building_name.trim().to_string();
                let display_name = if custom_label.is_empty() {
                    "〰 Path Corridor".to_string()
                } else {
                    custom_label.clone()
                };

                let path_type = "3D Path Corridor".to_string();

                let mut properties = HashMap::new();
                properties.insert("Layer".to_string(), "✏️ Custom Sketches".to_string());
                properties.insert("label".to_string(), custom_label.clone());
                properties.insert("Label".to_string(), custom_label.clone());
                properties.insert("Path Name".to_string(), display_name.clone());
                properties.insert("Path Type".to_string(), path_type.clone());
                properties.insert("Path Length".to_string(), format!("{:.1} m", total_len));
                properties.insert("Path Width".to_string(), format!("{:.1} m", self.line_width));
                properties.insert("Wall Height".to_string(), format!("{:.1} m", eff_wall_h));
                properties.insert("Vertices Count".to_string(), format!("{}", self.points.len()));
                properties.insert("Ground Elevation".to_string(), format!("{:.1} m", self.ground_elevation));
                properties.insert("Vertex Elevations".to_string(), self.points.iter().map(|p| format!("{:.3}", p.y)).collect::<Vec<_>>().join(","));
                properties.insert("Latitude".to_string(), format!("{:.6}°", center_geo.latitude));
                properties.insert("Longitude".to_string(), format!("{:.6}°", center_geo.longitude));

                let line_raw_ring: Vec<Vec2> = self.points.iter().map(|p| Vec2::new(p.x, -p.z)).collect();

                let mut line_mesh = mesh;
                line_mesh.colors = vec![self.color_tint; line_mesh.positions.len()];

                Some(GisFeature {
                    id: feature_id.to_string(),
                    name: display_name,
                    feature_type: path_type,
                    height: eff_wall_h,
                    min_height: self.ground_elevation,
                    center_geo,
                    center_local,
                    properties,
                    shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
                    color: Some(self.color_tint),
                    edge_enabled: true,
                    stroke_color: None,
                    stroke_width: None,
                    ground_elevation: self.ground_elevation,
                    elevation_zoom: -1,
                    geo_polygons,
                    raw_polygons: vec![vec![line_raw_ring]],
                    mesh: Some(line_mesh),
                    geometry: None,
                    symbol: None,
                })
            }
            SketchToolMode::Polygon | SketchToolMode::Rectangle | SketchToolMode::Circle => {
                let ring = self.generate_footprint_polygon(None);
                if ring.len() < 3 {
                    return None;
                }

                let effective_height = self.height.max(1.0);
                let rings = vec![ring.clone()];

                // Determine per-vertex base elevations from sketch.points
                let mut ring_elevs = Vec::with_capacity(ring.len());
                for p in &ring {
                    let mut closest_elev = self.ground_elevation;
                    let mut min_dist_sq = f32::INFINITY;
                    for pt3d in &self.points {
                        let d2 = (pt3d.x - p.x).powi(2) + (pt3d.z - (-p.y)).powi(2);
                        if d2 < min_dist_sq {
                            min_dist_sq = d2;
                            closest_elev = pt3d.y;
                        }
                    }
                    ring_elevs.push(closest_elev);
                }

                let min_elev = ring_elevs.iter().copied().fold(f32::INFINITY, f32::min);
                let mesh = extrude_polygon_with_elevation(&rings, Some(&[ring_elevs.clone()]), min_elev, effective_height)?;

                // Convert local 2D ring to geodetic coordinates [lat, lon]
                let mut geo_ring: Vec<[f64; 2]> = Vec::with_capacity(ring.len());
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_z = f32::INFINITY;
                let mut max_z = f32::NEG_INFINITY;
                for p in &ring {
                    let local_3d = Vec3::new(p.x, min_elev, -p.y);
                    let geo = origin.local_to_geo(local_3d);
                    geo_ring.push([geo.latitude, geo.longitude]);
                    min_x = min_x.min(p.x);
                    max_x = max_x.max(p.x);
                    min_z = min_z.min(-p.y);
                    max_z = max_z.max(-p.y);
                }

                let center_local = Vec3::new(
                    (min_x + max_x) * 0.5,
                    min_elev + effective_height * 0.5,
                    (min_z + max_z) * 0.5,
                );
                let center_geo = origin.local_to_geo(center_local);

                let area_m2 = signed_ring_area_2d(&ring).abs() as f32;
                let storeys = self.estimated_storeys();
                let gfa = area_m2 * storeys as f32;

                let custom_label = self.building_name.trim().to_string();
                let display_name = if custom_label.is_empty() {
                    "🏢 3D Structure".to_string()
                } else {
                    custom_label.clone()
                };

                let mut properties = HashMap::new();
                properties.insert("Layer".to_string(), "✏️ Custom Sketches".to_string());
                properties.insert("label".to_string(), custom_label.clone());
                properties.insert("Label".to_string(), custom_label.clone());
                properties.insert("Building Name".to_string(), display_name.clone());
                properties.insert("Building Type".to_string(), self.building_type.clone());
                properties.insert("Building Height".to_string(), format!("{:.1} m", effective_height));
                properties.insert("Storeys".to_string(), format!("{}", storeys));
                properties.insert("Footprint Area".to_string(), format!("{:.1} m² ({:.3} ha)", area_m2, area_m2 / 10000.0));
                properties.insert("Gross Floor Area (GFA)".to_string(), format!("{:.1} m²", gfa));
                properties.insert("Ground Elevation".to_string(), format!("{:.1} m", min_elev));
                properties.insert("Vertex Elevations".to_string(), ring_elevs.iter().map(|h| format!("{:.3}", h)).collect::<Vec<_>>().join(","));
                properties.insert("Latitude".to_string(), format!("{:.6}°", center_geo.latitude));
                properties.insert("Longitude".to_string(), format!("{:.6}°", center_geo.longitude));

                let mut poly_mesh = mesh;
                poly_mesh.colors = vec![self.color_tint; poly_mesh.positions.len()];

                Some(GisFeature {
                    id: feature_id.to_string(),
                    name: display_name,
                    feature_type: self.building_type.clone(),
                    height: effective_height,
                    min_height: min_elev,
                    center_geo,
                    center_local,
                    properties,
                    shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
                    color: Some(self.color_tint),
                    edge_enabled: true,
                    stroke_color: None,
                    stroke_width: None,
                    ground_elevation: min_elev,
                    elevation_zoom: -1,
                    geo_polygons: vec![vec![geo_ring]],
                    raw_polygons: vec![rings.clone()],
                    mesh: Some(poly_mesh),
                    geometry: None,
                    symbol: None,
                })
            }
        }
    }

    /// Builds a 3D procedural mesh for a point landmark/marker
    pub fn build_point_mesh_for_style(pt: Vec3, height: f32, style: PointMarkerStyle) -> crate::gis::extrusion::RawMeshData {
        let mut mesh = crate::gis::extrusion::RawMeshData::new();
        let base_y = pt.y;
        let h = height.max(3.0);

        match style {
            PointMarkerStyle::Beacon => {
                // Base ground target disc (radius 2.0m, height 0.25m)
                let base_disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 2.0, 0.25, 16);
                mesh.append(base_disc);

                // Vertical column stalk (radius 0.35m, height h - 1.2m)
                let stalk = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.25, pt.z), 0.35, (h - 1.2).max(1.0), 12);
                mesh.append(stalk);

                // Top glowing beacon sphere (radius 1.2m at peak)
                let sphere = generate_sphere_mesh(Vec3::new(pt.x, base_y + h, pt.z), 1.2, 10, 16);
                mesh.append(sphere);
            }
            PointMarkerStyle::Pin => {
                // Ground disc (radius 1.5m, height 0.15m)
                let disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.5, 0.15, 16);
                mesh.append(disc);

                // Thin needle stalk (radius 0.2m, height h)
                let needle = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.15, pt.z), 0.2, (h - 1.5).max(1.0), 8);
                mesh.append(needle);

                // Pin head diamond / sphere (radius 1.4m)
                let head = generate_sphere_mesh(Vec3::new(pt.x, base_y + h, pt.z), 1.4, 8, 12);
                mesh.append(head);
            }
            PointMarkerStyle::Tree => {
                // Trunk (radius 0.45m)
                let trunk_h = (h * 0.35).min(3.5);
                let trunk = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 0.45, trunk_h, 10);
                mesh.append(trunk);

                // Conical canopy foliage (radius 2.8m, height h - trunk_h)
                let canopy = generate_cone_mesh(Vec3::new(pt.x, base_y + trunk_h, pt.z), 2.8, (h - trunk_h).max(2.0), 14);
                mesh.append(canopy);
            }
            PointMarkerStyle::Pillar => {
                // Square monolith pillar (1.5m x 1.5m, height h)
                let pillar = generate_box_mesh(Vec3::new(pt.x, base_y + h * 0.5, pt.z), Vec3::new(1.5, h, 1.5));
                mesh.append(pillar);
            }
            PointMarkerStyle::Flag => {
                // Ground disc
                let disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.2, 0.2, 16);
                mesh.append(disc);
                // Flagpole mast
                let mast = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.2, pt.z), 0.16, (h - 0.2).max(1.0), 10);
                mesh.append(mast);
                // Finial sphere
                let finial = generate_sphere_mesh(Vec3::new(pt.x, base_y + h, pt.z), 0.3, 8, 10);
                mesh.append(finial);
                // Pennant / banner box extending along +X
                let banner_w = 2.4;
                let banner_h = (h * 0.35).clamp(1.2, 3.5);
                let banner_th = 0.08;
                let banner = generate_box_mesh(
                    Vec3::new(pt.x + banner_w * 0.5, base_y + h - banner_h * 0.5 - 0.1, pt.z),
                    Vec3::new(banner_w, banner_h, banner_th),
                );
                mesh.append(banner);
            }
            PointMarkerStyle::Diamond => {
                // Base pedestal disc
                let disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.5, 0.2, 16);
                mesh.append(disc);
                // Stalk
                let gem_hh = 1.4;
                let stalk_h = (h - gem_hh * 2.0 - 0.2).max(0.5);
                let stalk = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.2, pt.z), 0.25, stalk_h, 10);
                mesh.append(stalk);
                // Octahedral diamond crystal
                let gem = generate_octahedron_mesh(Vec3::new(pt.x, base_y + h - gem_hh, pt.z), 1.6, gem_hh);
                mesh.append(gem);
            }
            PointMarkerStyle::Cone => {
                // Circular base flange
                let flange = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 2.0, 0.2, 16);
                mesh.append(flange);
                // Survey cone spire
                let cone = generate_cone_mesh(Vec3::new(pt.x, base_y + 0.2, pt.z), 1.5, (h - 0.2).max(1.0), 16);
                mesh.append(cone);
            }
            PointMarkerStyle::Pyramid => {
                // Tiered plinth base
                let plinth1 = generate_box_mesh(Vec3::new(pt.x, base_y + 0.15, pt.z), Vec3::new(2.8, 0.3, 2.8));
                mesh.append(plinth1);
                let plinth2 = generate_box_mesh(Vec3::new(pt.x, base_y + 0.45, pt.z), Vec3::new(2.2, 0.3, 2.2));
                mesh.append(plinth2);
                // Obelisk pyramid
                let pyr_h = (h - 0.6).max(1.5);
                let pyr = generate_pyramid_mesh(Vec3::new(pt.x, base_y + 0.6, pt.z), 1.8, pyr_h);
                mesh.append(pyr);
            }
            PointMarkerStyle::Sphere => {
                // Pedestal base disc
                let disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.8, 0.25, 16);
                mesh.append(disc);
                // Column pedestal
                let orb_r = (h * 0.26).clamp(1.1, 2.8);
                let col_h = (h - orb_r * 2.0 - 0.25).max(0.5);
                let col = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.25, pt.z), 0.55, col_h, 14);
                mesh.append(col);
                // Top sphere orb
                let orb = generate_sphere_mesh(Vec3::new(pt.x, base_y + h - orb_r, pt.z), orb_r, 12, 18);
                mesh.append(orb);
            }
            PointMarkerStyle::Cylinder => {
                // Lower base rim
                let rim_b = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.8, 0.3, 16);
                mesh.append(rim_b);
                // Central drum
                let drum_h = (h - 0.6).max(1.0);
                let drum = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.3, pt.z), 1.3, drum_h, 16);
                mesh.append(drum);
                // Top capital rim
                let rim_t = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.3 + drum_h, pt.z), 1.6, 0.3, 16);
                mesh.append(rim_t);
            }
            PointMarkerStyle::Cross => {
                // Base plate
                let disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.6, 0.2, 16);
                mesh.append(disc);
                // Vertical column
                let post_w = 0.45;
                let post = generate_box_mesh(Vec3::new(pt.x, base_y + h * 0.5, pt.z), Vec3::new(post_w, h, post_w));
                mesh.append(post);
                // Horizontal crossbar X
                let arm_len = (h * 0.65).clamp(1.8, 5.0);
                let arm_y = base_y + h * 0.75;
                let arm_x = generate_box_mesh(Vec3::new(pt.x, arm_y, pt.z), Vec3::new(arm_len, post_w, post_w));
                mesh.append(arm_x);
                // Horizontal crossbar Z
                let arm_z = generate_box_mesh(Vec3::new(pt.x, arm_y, pt.z), Vec3::new(post_w, post_w, arm_len));
                mesh.append(arm_z);
            }
            PointMarkerStyle::Star => {
                // Base plate
                let disc = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.5, 0.2, 16);
                mesh.append(disc);
                // Stem
                let star_r = (h * 0.28).clamp(1.4, 3.2);
                let stem_h = (h - star_r * 2.0 - 0.2).max(0.5);
                let stem = generate_cylinder_mesh(Vec3::new(pt.x, base_y + 0.2, pt.z), 0.25, stem_h, 10);
                mesh.append(stem);
                // Vertical 5-pointed star facing camera / Z
                let star = generate_star_mesh(Vec3::new(pt.x, base_y + h - star_r, pt.z), star_r, star_r * 0.45, 0.4, 5);
                mesh.append(star);
            }
            PointMarkerStyle::ArrowDown => {
                // Ground target ring
                let target = generate_cylinder_mesh(Vec3::new(pt.x, base_y, pt.z), 1.8, 0.15, 20);
                mesh.append(target);
                // Downward pointing inverted cone (tip hovering 0.8m above ground)
                let cone_h = (h * 0.35).clamp(1.5, 3.5);
                let tip_y = base_y + 0.8;
                let inv_cone = generate_inverted_cone_mesh(Vec3::new(pt.x, tip_y, pt.z), 1.5, cone_h, 16);
                mesh.append(inv_cone);
                // Shaft extending up from cone base
                let shaft_base_y = tip_y + cone_h;
                let shaft_h = (h - (shaft_base_y - base_y)).max(0.8);
                let shaft = generate_cylinder_mesh(Vec3::new(pt.x, shaft_base_y, pt.z), 0.5, shaft_h, 12);
                mesh.append(shaft);
                // Top cap sphere
                let cap = generate_sphere_mesh(Vec3::new(pt.x, base_y + h, pt.z), 0.7, 8, 12);
                mesh.append(cap);
            }
        }
        mesh
    }

    pub fn build_point_mesh(&self, pt: Vec3) -> crate::gis::extrusion::RawMeshData {
        Self::build_point_mesh_for_style(pt, self.height, self.point_style)
    }

    /// Builds a continuous 3D corridor ribbon or extruded wall mesh along path vertices with smooth miter joints and end caps
    /// Builds a continuous 3D corridor ribbon or extruded wall mesh along path vertices with smooth miter joints and end caps
    pub fn build_line_mesh_from_points(points: &[Vec3], line_width: f32, wall_height: f32) -> crate::gis::extrusion::RawMeshData {
        let mut mesh = crate::gis::extrusion::RawMeshData::new();
        if points.len() < 2 {
            return mesh;
        }

        // Deduplicate adjacent points closer than 2cm
        let mut clean_pts: Vec<Vec3> = Vec::with_capacity(points.len());
        for &pt in points {
            if let Some(&last) = clean_pts.last() {
                let dx = pt.x - last.x;
                let dz = pt.z - last.z;
                if (dx * dx + dz * dz).sqrt() < 0.02 {
                    continue;
                }
            }
            clean_pts.push(pt);
        }

        let mut n = clean_pts.len();
        if n < 2 {
            return mesh;
        }

        // Detect if path forms a closed loop (start and end within 0.5m)
        let is_closed = n >= 3 && {
            let p_first = clean_pts[0];
            let p_last = clean_pts[n - 1];
            (p_first.x - p_last.x).hypot(p_first.z - p_last.z) < 0.5
        };

        // If closed, drop the duplicate closing point so the loop is represented by unique vertices
        if is_closed {
            clean_pts.pop();
            n = clean_pts.len();
            if n < 3 {
                return mesh;
            }
        }

        let half_w = (line_width.max(0.1)) * 0.5;
        let eff_h = if wall_height <= 0.05 {
            0.08 // 8cm curb profile
        } else {
            wall_height
        };
        let skirt_depth = 0.5;

        // 1. Compute segment 2D unit direction vectors and normal vectors (left-facing)
        let num_segs = if is_closed { n } else { n - 1 };
        let mut seg_dirs = Vec::with_capacity(num_segs);
        let mut seg_norms = Vec::with_capacity(num_segs);

        for i in 0..num_segs {
            let p0 = clean_pts[i];
            let p1 = clean_pts[(i + 1) % n];
            let dx = p1.x - p0.x;
            let dz = p1.z - p0.z;
            let len = (dx * dx + dz * dz).sqrt().max(1e-5);
            let dir = Vec3::new(dx / len, 0.0, dz / len);
            // Unit normal pointing to the left of path forward direction (-Z, X)
            let norm = Vec3::new(-dir.z, 0.0, dir.x);
            seg_dirs.push(dir);
            seg_norms.push(norm);
        }

        // 2. Compute robust miter vectors at each vertex
        let mut miter_offsets = Vec::with_capacity(n);
        const MITER_LIMIT: f32 = 2.5;

        for i in 0..n {
            if !is_closed && i == 0 {
                miter_offsets.push(seg_norms[0] * half_w);
            } else if !is_closed && i == n - 1 {
                miter_offsets.push(seg_norms[num_segs - 1] * half_w);
            } else {
                let prev_idx = if i == 0 { num_segs - 1 } else { i - 1 };
                let next_idx = i % num_segs;
                let d_prev = seg_dirs[prev_idx];
                let d_next = seg_dirs[next_idx];
                let n_prev = seg_norms[prev_idx];

                let tangent = d_prev + d_next;
                if tangent.length_squared() < 1e-4 {
                    // Path reversed 180 degrees
                    miter_offsets.push(n_prev * half_w);
                } else {
                    let tangent_dir = tangent.normalize();
                    // Normal to tangent pointing left
                    let mut miter_dir = Vec3::new(-tangent_dir.z, 0.0, tangent_dir.x);
                    let mut cos_alpha = miter_dir.dot(n_prev);
                    if cos_alpha < 0.0 {
                        miter_dir = -miter_dir;
                        cos_alpha = -cos_alpha;
                    }
                    let miter_len = if cos_alpha > 0.15 {
                        (half_w / cos_alpha).min(MITER_LIMIT * half_w)
                    } else {
                        MITER_LIMIT * half_w
                    };
                    miter_offsets.push(miter_dir * miter_len);
                }
            }
        }

        // 3. Generate vertices for left/right base and top at each path vertex
        let mut b_left = Vec::with_capacity(n);
        let mut b_right = Vec::with_capacity(n);
        let mut t_left = Vec::with_capacity(n);
        let mut t_right = Vec::with_capacity(n);

        for i in 0..n {
            let p = clean_pts[i];
            let offset = miter_offsets[i];

            let bl = Vec3::new(p.x - offset.x, p.y - skirt_depth, p.z - offset.z);
            let br = Vec3::new(p.x + offset.x, p.y - skirt_depth, p.z + offset.z);
            let tl = Vec3::new(p.x - offset.x, p.y + eff_h, p.z - offset.z);
            let tr = Vec3::new(p.x + offset.x, p.y + eff_h, p.z + offset.z);

            b_left.push(bl);
            b_right.push(br);
            t_left.push(tl);
            t_right.push(tr);
        }

        // 4. Build continuous ribbon quads for each segment
        for i in 0..num_segs {
            let next_i = (i + 1) % n;
            let tl0 = t_left[i];
            let tr0 = t_right[i];
            let tl1 = t_left[next_i];
            let tr1 = t_right[next_i];

            let bl0 = b_left[i];
            let br0 = b_right[i];
            let bl1 = b_left[next_i];
            let br1 = b_right[next_i];

            // Top surface: CCW winding seen from above: tl0 -> tr0 -> tr1 -> tl1 (Normal: +Y)
            add_quad(&mut mesh, tl0, tr0, tr1, tl1, Vec3::Y);

            // Bottom surface: CCW winding seen from below: bl0 -> bl1 -> br1 -> br0 (Normal: -Y)
            add_quad(&mut mesh, bl0, bl1, br1, br0, -Vec3::Y);

            // Left wall (Normal: -seg_norms[i])
            let left_n = -seg_norms[i];
            add_quad(&mut mesh, bl0, tl0, tl1, bl1, left_n);

            // Right wall (Normal: seg_norms[i])
            let right_n = seg_norms[i];
            add_quad(&mut mesh, br1, tr1, tr0, br0, right_n);
        }

        // 5. If path is open, seal the start and end with end caps
        if !is_closed {
            let start_dir = -seg_dirs[0];
            let bl0 = b_left[0];
            let br0 = b_right[0];
            let tl0 = t_left[0];
            let tr0 = t_right[0];
            add_quad(&mut mesh, br0, tr0, tl0, bl0, start_dir);

            let last = n - 1;
            let end_dir = seg_dirs[num_segs - 1];
            let bl_last = b_left[last];
            let br_last = b_right[last];
            let tl_last = t_left[last];
            let tr_last = t_right[last];
            add_quad(&mut mesh, bl_last, tl_last, tr_last, br_last, end_dir);
        }

        mesh
    }

    pub fn build_line_mesh(&self) -> crate::gis::extrusion::RawMeshData {
        Self::build_line_mesh_from_points(&self.points, self.line_width, self.wall_height)
    }
}

fn add_quad(mesh: &mut crate::gis::extrusion::RawMeshData, v0: Vec3, v1: Vec3, v2: Vec3, v3: Vec3, normal: Vec3) {
    let base = mesh.positions.len() as u32;
    let n = normal.to_array();
    let col = [1.0, 1.0, 1.0, 1.0];
    mesh.positions.push(v0.to_array());
    mesh.positions.push(v1.to_array());
    mesh.positions.push(v2.to_array());
    mesh.positions.push(v3.to_array());

    mesh.normals.push(n);
    mesh.normals.push(n);
    mesh.normals.push(n);
    mesh.normals.push(n);

    mesh.uvs.push([0.0, 0.0]);
    mesh.uvs.push([1.0, 0.0]);
    mesh.uvs.push([1.0, 1.0]);
    mesh.uvs.push([0.0, 1.0]);

    mesh.colors.push(col);
    mesh.colors.push(col);
    mesh.colors.push(col);
    mesh.colors.push(col);

    mesh.indices.push(base);
    mesh.indices.push(base + 1);
    mesh.indices.push(base + 2);
    mesh.indices.push(base);
    mesh.indices.push(base + 2);
    mesh.indices.push(base + 3);
}

fn add_triangle(mesh: &mut crate::gis::extrusion::RawMeshData, v0: Vec3, v1: Vec3, v2: Vec3, normal: Vec3) {
    let base = mesh.positions.len() as u32;
    let n = normal.to_array();
    let col = [1.0, 1.0, 1.0, 1.0];
    mesh.positions.push(v0.to_array());
    mesh.positions.push(v1.to_array());
    mesh.positions.push(v2.to_array());

    mesh.normals.push(n);
    mesh.normals.push(n);
    mesh.normals.push(n);

    mesh.uvs.push([0.0, 0.0]);
    mesh.uvs.push([1.0, 0.0]);
    mesh.uvs.push([0.5, 1.0]);

    mesh.colors.push(col);
    mesh.colors.push(col);
    mesh.colors.push(col);

    mesh.indices.push(base);
    mesh.indices.push(base + 1);
    mesh.indices.push(base + 2);
}

fn add_triangle_outward(mesh: &mut crate::gis::extrusion::RawMeshData, a: Vec3, b: Vec3, c: Vec3, center: Vec3) {
    let n = (b - a).cross(c - a).normalize_or_zero();
    let centroid = (a + b + c) * (1.0 / 3.0);
    if n.dot(centroid - center) < 0.0 {
        add_triangle(mesh, a, c, b, -n);
    } else {
        add_triangle(mesh, a, b, c, n);
    }
}

fn add_quad_outward(mesh: &mut crate::gis::extrusion::RawMeshData, v0: Vec3, v1: Vec3, v2: Vec3, v3: Vec3, center: Vec3) {
    let n = (v1 - v0).cross(v2 - v0).normalize_or_zero();
    let centroid = (v0 + v1 + v2 + v3) * 0.25;
    if n.dot(centroid - center) < 0.0 {
        add_quad(mesh, v3, v2, v1, v0, -n);
    } else {
        add_quad(mesh, v0, v1, v2, v3, n);
    }
}

fn generate_pyramid_mesh(base_center: Vec3, base_size: f32, height: f32) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let hs = base_size * 0.5;
    let apex = Vec3::new(base_center.x, base_center.y + height, base_center.z);
    let center = Vec3::new(base_center.x, base_center.y + height * 0.25, base_center.z);

    let p0 = Vec3::new(base_center.x - hs, base_center.y, base_center.z - hs);
    let p1 = Vec3::new(base_center.x + hs, base_center.y, base_center.z - hs);
    let p2 = Vec3::new(base_center.x + hs, base_center.y, base_center.z + hs);
    let p3 = Vec3::new(base_center.x - hs, base_center.y, base_center.z + hs);

    // Base
    add_quad_outward(&mut mesh, p0, p1, p2, p3, center);

    // Sides
    add_triangle_outward(&mut mesh, p0, p1, apex, center);
    add_triangle_outward(&mut mesh, p1, p2, apex, center);
    add_triangle_outward(&mut mesh, p2, p3, apex, center);
    add_triangle_outward(&mut mesh, p3, p0, apex, center);

    mesh
}

fn generate_octahedron_mesh(center: Vec3, radius: f32, half_height: f32) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let top_apex = center + Vec3::new(0.0, half_height, 0.0);
    let bot_apex = center - Vec3::new(0.0, half_height, 0.0);
    let eq = [
        center + Vec3::new(radius, 0.0, 0.0),
        center + Vec3::new(0.0, 0.0, radius),
        center + Vec3::new(-radius, 0.0, 0.0),
        center + Vec3::new(0.0, 0.0, -radius),
    ];
    for i in 0..4 {
        let next = (i + 1) % 4;
        add_triangle_outward(&mut mesh, eq[i], eq[next], top_apex, center);
        add_triangle_outward(&mut mesh, eq[i], eq[next], bot_apex, center);
    }
    mesh
}

fn generate_inverted_cone_mesh(tip_center: Vec3, radius: f32, height: f32, segments: usize) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let segs = segments.max(6);
    let top_y = tip_center.y + height;
    let top_center = Vec3::new(tip_center.x, top_y, tip_center.z);
    let center = tip_center + Vec3::new(0.0, height * 0.6, 0.0);

    let mut top_pts = Vec::with_capacity(segs);
    for i in 0..segs {
        let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
        top_pts.push(Vec3::new(tip_center.x + radius * theta.cos(), top_y, tip_center.z + radius * theta.sin()));
    }

    // Top cap fan (pointing up)
    for i in 0..segs {
        let next = (i + 1) % segs;
        add_triangle(&mut mesh, top_center, top_pts[i], top_pts[next], Vec3::Y);
    }

    // Inverted sides
    for i in 0..segs {
        let next = (i + 1) % segs;
        add_triangle_outward(&mut mesh, top_pts[i], top_pts[next], tip_center, center);
    }

    mesh
}

fn generate_star_mesh(center: Vec3, outer_r: f32, inner_r: f32, thickness: f32, num_points: usize) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let pts = num_points.max(3);
    let total_vertices = pts * 2;
    let ht = thickness * 0.5;

    let mut front_pts = Vec::with_capacity(total_vertices);
    let mut back_pts = Vec::with_capacity(total_vertices);

    for i in 0..total_vertices {
        let angle = (i as f32 / total_vertices as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        let x = center.x + r * angle.cos();
        let y = center.y + r * angle.sin();
        front_pts.push(Vec3::new(x, y, center.z + ht));
        back_pts.push(Vec3::new(x, y, center.z - ht));
    }

    let c_front = Vec3::new(center.x, center.y, center.z + ht);
    let c_back = Vec3::new(center.x, center.y, center.z - ht);

    // Front & Back fans
    for i in 0..total_vertices {
        let next = (i + 1) % total_vertices;
        add_triangle(&mut mesh, c_front, front_pts[i], front_pts[next], Vec3::Z);
        add_triangle(&mut mesh, c_back, back_pts[next], back_pts[i], -Vec3::Z);
    }

    // Side edge quads
    for i in 0..total_vertices {
        let next = (i + 1) % total_vertices;
        add_quad_outward(&mut mesh, back_pts[i], front_pts[i], front_pts[next], back_pts[next], center);
    }

    mesh
}

fn generate_cylinder_mesh(center: Vec3, radius: f32, height: f32, segments: usize) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let segs = segments.max(6);
    let top_y = center.y + height;
    let bot_y = center.y;

    let mut bottom_pts = Vec::with_capacity(segs);
    let mut top_pts = Vec::with_capacity(segs);

    for i in 0..segs {
        let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
        let x = center.x + radius * theta.cos();
        let z = center.z + radius * theta.sin();
        bottom_pts.push(Vec3::new(x, bot_y, z));
        top_pts.push(Vec3::new(x, top_y, z));
    }

    // Side quads
    for i in 0..segs {
        let next = (i + 1) % segs;
        let theta_mid = ((i as f32 + 0.5) / segs as f32) * std::f32::consts::TAU;
        let n = Vec3::new(theta_mid.cos(), 0.0, theta_mid.sin());
        add_quad(&mut mesh, bottom_pts[i], bottom_pts[next], top_pts[next], top_pts[i], n);
    }

    // Top cap fan
    let top_center = Vec3::new(center.x, top_y, center.z);
    for i in 0..segs {
        let next = (i + 1) % segs;
        let base = mesh.positions.len() as u32;
        mesh.positions.push(top_center.to_array());
        mesh.positions.push(top_pts[i].to_array());
        mesh.positions.push(top_pts[next].to_array());

        mesh.normals.push(Vec3::Y.to_array());
        mesh.normals.push(Vec3::Y.to_array());
        mesh.normals.push(Vec3::Y.to_array());

        mesh.uvs.push([0.5, 0.5]);
        mesh.uvs.push([0.0, 0.0]);
        mesh.uvs.push([1.0, 0.0]);

        mesh.colors.push([1.0, 1.0, 1.0, 1.0]);
        mesh.colors.push([1.0, 1.0, 1.0, 1.0]);
        mesh.colors.push([1.0, 1.0, 1.0, 1.0]);

        mesh.indices.push(base);
        mesh.indices.push(base + 1);
        mesh.indices.push(base + 2);
    }

    mesh
}

fn generate_sphere_mesh(center: Vec3, radius: f32, rings: usize, sectors: usize) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let r_count = rings.max(4);
    let s_count = sectors.max(6);

    for r in 0..=r_count {
        let phi = std::f32::consts::PI * (r as f32 / r_count as f32);
        let y = center.y + radius * phi.cos();
        let ring_r = radius * phi.sin();

        for s in 0..=s_count {
            let theta = std::f32::consts::TAU * (s as f32 / s_count as f32);
            let x = center.x + ring_r * theta.cos();
            let z = center.z + ring_r * theta.sin();

            let p = Vec3::new(x, y, z);
            let n = (p - center).normalize_or_zero();

            mesh.positions.push(p.to_array());
            mesh.normals.push(n.to_array());
            mesh.uvs.push([s as f32 / s_count as f32, r as f32 / r_count as f32]);
            mesh.colors.push([1.0, 1.0, 1.0, 1.0]);
        }
    }

    for r in 0..r_count {
        for s in 0..s_count {
            let first = (r * (s_count + 1) + s) as u32;
            let second = first + s_count as u32 + 1;

            mesh.indices.push(first);
            mesh.indices.push(first + 1);
            mesh.indices.push(second);

            mesh.indices.push(second);
            mesh.indices.push(first + 1);
            mesh.indices.push(second + 1);
        }
    }

    mesh
}

fn generate_cone_mesh(base_center: Vec3, radius: f32, height: f32, segments: usize) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let segs = segments.max(6);
    let apex = Vec3::new(base_center.x, base_center.y + height, base_center.z);

    let mut base_pts = Vec::with_capacity(segs);
    for i in 0..segs {
        let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
        base_pts.push(Vec3::new(base_center.x + radius * theta.cos(), base_center.y, base_center.z + radius * theta.sin()));
    }

    // Cone side triangles
    for i in 0..segs {
        let next = (i + 1) % segs;
        let base = mesh.positions.len() as u32;
        let p0 = base_pts[i];
        let p1 = base_pts[next];
        let edge1 = p1 - p0;
        let edge2 = apex - p0;
        let normal = edge1.cross(edge2).normalize_or_zero();

        mesh.positions.push(apex.to_array());
        mesh.positions.push(p0.to_array());
        mesh.positions.push(p1.to_array());

        mesh.normals.push(normal.to_array());
        mesh.normals.push(normal.to_array());
        mesh.normals.push(normal.to_array());

        mesh.uvs.push([0.5, 1.0]);
        mesh.uvs.push([0.0, 0.0]);
        mesh.uvs.push([1.0, 0.0]);

        mesh.colors.push([1.0, 1.0, 1.0, 1.0]);
        mesh.colors.push([1.0, 1.0, 1.0, 1.0]);
        mesh.colors.push([1.0, 1.0, 1.0, 1.0]);

        mesh.indices.push(base);
        mesh.indices.push(base + 1);
        mesh.indices.push(base + 2);
    }

    mesh
}

fn generate_box_mesh(center: Vec3, size: Vec3) -> crate::gis::extrusion::RawMeshData {
    let mut mesh = crate::gis::extrusion::RawMeshData::new();
    let hw = size.x * 0.5;
    let hh = size.y * 0.5;
    let hd = size.z * 0.5;

    let c = center;
    let p0 = Vec3::new(c.x - hw, c.y - hh, c.z - hd);
    let p1 = Vec3::new(c.x + hw, c.y - hh, c.z - hd);
    let p2 = Vec3::new(c.x + hw, c.y + hh, c.z - hd);
    let p3 = Vec3::new(c.x - hw, c.y + hh, c.z - hd);

    let p4 = Vec3::new(c.x - hw, c.y - hh, c.z + hd);
    let p5 = Vec3::new(c.x + hw, c.y - hh, c.z + hd);
    let p6 = Vec3::new(c.x + hw, c.y + hh, c.z + hd);
    let p7 = Vec3::new(c.x - hw, c.y + hh, c.z + hd);

    // Front, Back, Top, Bottom, Left, Right
    add_quad(&mut mesh, p4, p5, p6, p7, Vec3::Z);
    add_quad(&mut mesh, p1, p0, p3, p2, -Vec3::Z);
    add_quad(&mut mesh, p3, p2, p6, p7, Vec3::Y);
    add_quad(&mut mesh, p0, p1, p5, p4, -Vec3::Y);
    add_quad(&mut mesh, p0, p4, p7, p3, -Vec3::X);
    add_quad(&mut mesh, p5, p1, p2, p6, Vec3::X);

    mesh
}

/// Re-extrudes the 3D mesh, updates area, storeys, GFA, and geodetic coords for a custom sketched feature
pub fn rebuild_sketch_feature_mesh(
    origin: &crate::gis::crs::ProjectOrigin,
    terrain: &crate::gis::terrain::TerrainManager,
    feat: &mut crate::gis::geojson_loader::GisFeature,
) {
    if feat.feature_type == "3D Marker" || feat.properties.contains_key("Marker Style") {
        let style = feat.properties.get("Marker Style")
            .map(|s| PointMarkerStyle::from_str(s))
            .unwrap_or(PointMarkerStyle::Beacon);
        let base_pt = if let Some(ring) = feat.raw_polygons.first().and_then(|p| p.first()) {
            if let Some(v) = ring.first() {
                Vec3::new(v.x, feat.ground_elevation, -v.y)
            } else {
                Vec3::new(feat.center_local.x, feat.ground_elevation, feat.center_local.z)
            }
        } else {
            Vec3::new(feat.center_local.x, feat.ground_elevation, feat.center_local.z)
        };
        feat.center_local = Vec3::new(base_pt.x, feat.ground_elevation + feat.height * 0.5, base_pt.z);
        let mut point_mesh = SketchEngine::build_point_mesh_for_style(
            base_pt,
            feat.height,
            style,
        );

        // Rotate point marker symbol vertices around base_pt according to Heading property
        let heading_deg = feat.properties.get("Heading")
            .and_then(|s| s.trim_end_matches('°').parse::<f32>().ok())
            .unwrap_or(0.0);
        if heading_deg.abs() > 1e-3 {
            let rad = heading_deg.to_radians();
            let (sin_a, cos_a) = rad.sin_cos();
            for pos in point_mesh.positions.iter_mut() {
                let dx = pos[0] - base_pt.x;
                let dz = pos[2] - base_pt.z;
                pos[0] = base_pt.x + dx * cos_a - dz * sin_a;
                pos[2] = base_pt.z + dx * sin_a + dz * cos_a;
            }
            for norm in point_mesh.normals.iter_mut() {
                let nx = norm[0];
                let nz = norm[2];
                norm[0] = nx * cos_a - nz * sin_a;
                norm[2] = nx * sin_a + nz * cos_a;
            }
        }

        feat.mesh = Some(point_mesh);
    } else if feat.feature_type == "3D Path Corridor" || (feat.properties.contains_key("Path Name") && !feat.raw_polygons.is_empty()) {
        if let Some(ring) = feat.raw_polygons.first().and_then(|p| p.first()) {
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_z = f32::INFINITY;
            let mut max_z = f32::NEG_INFINITY;
            let parsed_elevs: Option<Vec<f32>> = feat.properties.get("Vertex Elevations").map(|s| {
                s.split(',').filter_map(|v| v.trim().parse::<f32>().ok()).collect()
            });
            let line_pts: Vec<Vec3> = ring.iter().enumerate().map(|(idx, v)| {
                min_x = min_x.min(v.x);
                max_x = max_x.max(v.x);
                min_z = min_z.min(-v.y);
                max_z = max_z.max(-v.y);
                let elev = if let Some(ref elevs) = parsed_elevs {
                    if let Some(&e) = elevs.get(idx) {
                        e
                    } else {
                        feat.ground_elevation
                    }
                } else if terrain.is_enabled {
                    let geo = origin.local_to_geo(Vec3::new(v.x, 0.0, -v.y));
                    let geodetic_elev = terrain.sample_elevation(geo.latitude, geo.longitude)
                        .unwrap_or(origin.origin.elevation as f32) as f64;
                    origin.lat_lon_to_local(geo.latitude, geo.longitude, geodetic_elev).y + feat.ground_elevation
                } else {
                    feat.ground_elevation
                };
                Vec3::new(v.x, elev, -v.y)
            }).collect();
            if min_x.is_finite() && max_x.is_finite() && feat.center_local.x == 0.0 && feat.center_local.z == 0.0 {
                feat.center_local.x = (min_x + max_x) * 0.5;
                feat.center_local.z = (min_z + max_z) * 0.5;
                feat.center_geo = origin.local_to_geo(feat.center_local);
            }
            feat.center_local.y = feat.ground_elevation + feat.height * 0.5;
            let path_w = feat.properties.get("Path Width")
                .and_then(|s| s.trim_end_matches(" m").parse::<f32>().ok())
                .unwrap_or(3.5);
            feat.mesh = Some(SketchEngine::build_line_mesh_from_points(&line_pts, path_w, feat.height));
            let total_len: f32 = line_pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
            feat.properties.insert("Path Length".to_string(), format!("{:.1} m", total_len));
            feat.properties.insert("Vertices Count".to_string(), format!("{}", ring.len()));
        }
    } else if let Some(rings) = feat.raw_polygons.first() {
        let parsed_elevs: Option<Vec<Vec<f32>>> = feat.properties.get("Vertex Elevations").map(|s| {
            let evs: Vec<f32> = s.split(',').filter_map(|v| v.trim().parse::<f32>().ok()).collect();
            vec![evs]
        });
        feat.mesh = crate::gis::extrusion::extrude_polygon_with_elevation(
            rings,
            parsed_elevs.as_deref(),
            feat.ground_elevation,
            feat.height,
        );
        if let Some(outer) = rings.first() {
            let area_m2 = crate::gis::extrusion::signed_ring_area_2d(outer).abs() as f32;
            let storeys = (feat.height / 3.2).round().max(1.0) as u32;
            let gfa = area_m2 * storeys as f32;
            feat.properties.insert("Footprint Area".to_string(), format!("{:.1} m² ({:.3} ha)", area_m2, area_m2 / 10000.0));
            feat.properties.insert("Gross Floor Area (GFA)".to_string(), format!("{:.1} m²", gfa));
        }
    }

    // Apply diffuse vertex color if feature has custom color
    if let Some(col) = feat.color {
        if let Some(mesh) = &mut feat.mesh {
            mesh.colors = vec![col; mesh.positions.len()];
        }
    }

    // Re-calculate geodetic polygon rings (-v.y is 3D Z)
    if !feat.raw_polygons.is_empty() {
        feat.geo_polygons = feat.raw_polygons.iter().map(|poly| {
            poly.iter().map(|ring| {
                ring.iter().map(|v| {
                    let geo = origin.local_to_geo(Vec3::new(v.x, 0.0, -v.y));
                    [geo.latitude, geo.longitude]
                }).collect()
            }).collect()
        }).collect();
    }
}

