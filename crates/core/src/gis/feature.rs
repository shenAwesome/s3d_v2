use crate::gis::extrusion::RawMeshData;
use crate::gis::geojson_loader::GisFeature;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Point geometry in 3D GIS space (x: East/Lon, y: Up/Elevation, z: North/Lat).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PointGeometry {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
}

impl PointGeometry {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Convenience alias matching Esri Point geometry
pub type Point = PointGeometry;

/// Polygon rings in geodetic or planar coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PolygonGeometry {
    pub rings: Vec<Vec<[f64; 2]>>,
}

/// Feature geometry supporting Points, Polygons, or 3D Meshes (matching Esri Geometry types).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Geometry {
    #[serde(rename = "point")]
    Point(PointGeometry),
    #[serde(rename = "polygon")]
    Polygon(PolygonGeometry),
    #[serde(rename = "mesh")]
    Mesh(RawMeshData),
}

/// 3D object primitive shape resource (cube, sphere, cylinder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectSymbol3DResource {
    #[serde(rename = "cube")]
    Cube,
    #[serde(rename = "sphere")]
    Sphere,
    #[serde(rename = "cylinder")]
    Cylinder,
}

/// 3D Volumetric Object Symbol Layer representing a 3D shape (e.g. cube/box) with real-world dimensions in meters.
/// Reference: https://developers.arcgis.com/javascript/latest/references/core/symbols/ObjectSymbol3DLayer/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSymbol3DLayer {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub resource: ObjectSymbol3DResource,
    pub material_color: [f32; 4],
}

/// 3D Point Symbol containing one or more 3D symbol layers.
/// Reference: https://developers.arcgis.com/javascript/latest/references/core/symbols/PointSymbol3D/
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PointSymbol3D {
    pub symbol_layers: Vec<ObjectSymbol3DLayer>,
}

impl PointSymbol3D {
    pub fn new(symbol_layers: Vec<ObjectSymbol3DLayer>) -> Self {
        Self { symbol_layers }
    }

    /// Convenience constructor creating a 3D box/cube symbol with dimensions in meters and RGBA color.
    pub fn cube(width: f32, height: f32, depth: f32, color: [f32; 4]) -> Self {
        Self {
            symbol_layers: vec![ObjectSymbol3DLayer {
                width,
                height,
                depth,
                resource: ObjectSymbol3DResource::Cube,
                material_color: color,
            }],
        }
    }
}

/// Alias reflecting standard GIS & Esri terminology (Graphic / Feature).
pub type Feature = GisFeature;

impl GisFeature {
    /// Creates an Esri-aligned Feature containing Geometry and an optional 3D Symbol.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        geometry: Geometry,
        symbol: Option<PointSymbol3D>,
    ) -> Self {
        let id_str = id.into();
        let name_str = name.into();

        match &geometry {
            Geometry::Point(pt) => {
                if let Some(sym) = &symbol {
                    if let Some(obj_layer) = sym.symbol_layers.first() {
                        let w2 = obj_layer.width * 0.5;
                        let d2 = obj_layer.depth * 0.5;
                        // Bottom-anchored 3D volumetric box at point (x, y, z)
                        let min = Vec3::new(
                            (pt.x as f32) - w2,
                            pt.y as f32,
                            (pt.z as f32) - d2,
                        );
                        let max = Vec3::new(
                            (pt.x as f32) + w2,
                            (pt.y as f32) + obj_layer.height,
                            (pt.z as f32) + d2,
                        );
                        let mut feat = GisFeature::create_box(
                            id_str,
                            name_str,
                            min,
                            max,
                            obj_layer.material_color,
                        );
                        feat.geometry = Some(geometry);
                        feat.symbol = symbol;
                        return feat;
                    }
                }
                // Default point without symbol layer
                let mut feat = GisFeature::default();
                feat.id = id_str;
                feat.name = name_str;
                feat.center_local = Vec3::new(pt.x as f32, pt.y as f32, pt.z as f32);
                feat.geometry = Some(geometry);
                feat.symbol = symbol;
                feat
            }
            Geometry::Mesh(mesh) => {
                let color = symbol
                    .as_ref()
                    .and_then(|s| s.symbol_layers.first())
                    .map(|l| l.material_color)
                    .unwrap_or([0.22, 0.58, 0.92, 1.0]);
                let mut feat = GisFeature::new_mesh_feature(id_str, name_str, mesh.clone(), color);
                feat.geometry = Some(geometry);
                feat.symbol = symbol;
                feat
            }
            Geometry::Polygon(poly) => {
                let mut feat = GisFeature::default();
                feat.id = id_str;
                feat.name = name_str;
                feat.geo_polygons = vec![poly.rings.clone()];
                feat.geometry = Some(geometry);
                feat.symbol = symbol;
                feat
            }
        }
    }
}

