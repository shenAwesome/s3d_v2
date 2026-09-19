use crate::gis::crs::{GeoCoord, ProjectOrigin};
use crate::gis::extrusion::RawMeshData;
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SSpatialReference {
    pub wkid: Option<u32>,
    pub latest_wkid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SExtent {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
    pub zmin: Option<f64>,
    pub zmax: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SNodePagesConfig {
    pub nodes_per_page: Option<u32>,
    pub lod_selection_metric_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SGeometryAttributeDef {
    #[serde(rename = "type")]
    pub attr_type: Option<String>,
    pub component: Option<u32>,
    pub binding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SGeometryBufferDef {
    pub offset: Option<usize>,
    pub position: Option<I3SGeometryAttributeDef>,
    pub normal: Option<I3SGeometryAttributeDef>,
    pub uv0: Option<I3SGeometryAttributeDef>,
    pub color: Option<I3SGeometryAttributeDef>,
    pub feature_id: Option<I3SGeometryAttributeDef>,
    pub face_range: Option<I3SGeometryAttributeDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SGeometryDefinition {
    pub geometry_buffers: Vec<I3SGeometryBufferDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SPBRMetallicRoughness {
    pub base_color_factor: Option<[f32; 4]>,
    pub metallic_factor: Option<f32>,
    pub roughness_factor: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SMaterialDefinition {
    pub pbr_metallic_roughness: Option<I3SPBRMetallicRoughness>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SSceneLayer {
    pub id: Option<u32>,
    pub name: Option<String>,
    pub layer_type: Option<String>,
    pub spatial_reference: Option<I3SSpatialReference>,
    pub full_extent: Option<I3SExtent>,
    pub node_pages: Option<I3SNodePagesConfig>,
    pub geometry_definitions: Option<Vec<I3SGeometryDefinition>>,
    pub material_definitions: Option<Vec<I3SMaterialDefinition>>,
    pub drawing_info: Option<I3SDrawingInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SDrawingInfo {
    pub renderer: Option<I3SRenderer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SRenderer {
    pub r#type: Option<String>,
    pub symbol: Option<I3SMeshSymbol3D>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SMeshSymbol3D {
    pub r#type: Option<String>,
    pub symbol_layers: Option<Vec<I3SSymbolLayer>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SSymbolLayer {
    pub r#type: Option<String>,
    pub material: Option<I3SSymbolMaterial>,
    pub edges: Option<I3SSolidEdges3D>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SSymbolMaterial {
    pub color: Option<Vec<f32>>,
    pub transparency: Option<f32>,
    pub color_mix_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SSolidEdges3D {
    pub r#type: Option<String>,
    pub color: Option<Vec<f32>>,
    pub size: Option<f32>,
    pub transparency: Option<f32>,
    pub extension_length: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct I3SServerSymbol {
    pub color_tint: [f32; 4],
    pub opacity: f32,
    pub edge_enabled: bool,
    pub stroke_color: [f32; 4],
    pub stroke_width: f32,
}

impl I3SSceneLayer {
    pub fn extract_symbol(&self) -> Option<I3SServerSymbol> {
        let di = self.drawing_info.as_ref()?;
        let renderer = di.renderer.as_ref()?;
        let symbol = renderer.symbol.as_ref()?;
        let layers = symbol.symbol_layers.as_ref()?;
        let fill_layer = layers.iter().find(|l| l.r#type.as_deref() == Some("Fill"))?;

        // 1. Base Material Color
        let mut color_tint = [1.0, 1.0, 1.0, 1.0];
        let mut opacity = 1.0;
        if let Some(mat) = &fill_layer.material {
            if let Some(c) = &mat.color {
                if c.len() >= 3 {
                    color_tint[0] = (c[0] / 255.0).clamp(0.0, 1.0);
                    color_tint[1] = (c[1] / 255.0).clamp(0.0, 1.0);
                    color_tint[2] = (c[2] / 255.0).clamp(0.0, 1.0);
                }
                if c.len() >= 4 {
                    color_tint[3] = (c[3] / 255.0).clamp(0.0, 1.0);
                }
            }
            if let Some(tr) = mat.transparency {
                opacity = (1.0 - tr / 100.0).clamp(0.0, 1.0);
                color_tint[3] = opacity;
            }
        }

        // 2. SolidEdges3D
        let mut edge_enabled = false;
        let mut stroke_color = [0.15, 0.16, 0.18, 1.0];
        let mut stroke_width = 1.0;
        if let Some(edges) = &fill_layer.edges {
            edge_enabled = edges.r#type.as_deref() == Some("solid") || edges.r#type.is_some();
            if let Some(c) = &edges.color {
                if c.len() >= 3 {
                    stroke_color[0] = (c[0] / 255.0).clamp(0.0, 1.0);
                    stroke_color[1] = (c[1] / 255.0).clamp(0.0, 1.0);
                    stroke_color[2] = (c[2] / 255.0).clamp(0.0, 1.0);
                }
                if c.len() >= 4 {
                    stroke_color[3] = (c[3] / 255.0).clamp(0.0, 1.0);
                }
            }
            if let Some(tr) = edges.transparency {
                stroke_color[3] = (1.0 - tr / 100.0).clamp(0.0, 1.0);
            }
            if let Some(sz) = edges.size {
                stroke_width = sz.clamp(0.1, 10.0);
            }
        }

        Some(I3SServerSymbol {
            color_tint,
            opacity,
            edge_enabled,
            stroke_color,
            stroke_width,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct I3SOrientedBoundingBox {
    pub center: [f64; 3],
    #[serde(rename = "halfSize")]
    pub half_size: [f64; 3],
    pub quaternion: [f64; 4],
}

impl I3SOrientedBoundingBox {
    pub fn radius(&self) -> f64 {
        let (hx, hy, hz) = (self.half_size[0], self.half_size[1], self.half_size[2]);
        (hx * hx + hy * hy + hz * hz).sqrt()
    }

    /// Converts OBB parameters to Local Engine Cartesian Space (+X=East, +Y=Up, -Z=North).
    /// Returns `(center_engine, [axis_east, axis_up, axis_south], half_size)`.
    pub fn to_engine_obb(&self, origin: &ProjectOrigin) -> (Vec3, [Vec3; 3], Vec3) {
        let center_enu = origin.geo_to_local(&GeoCoord::new(self.center[1], self.center[0], self.center[2]));
        let half_size = Vec3::new(
            self.half_size[0] as f32,
            self.half_size[2] as f32, // Z in I3S is elevation/height -> Engine Y (Up)
            self.half_size[1] as f32, // Y in I3S is latitude/North -> Engine Z (North)
        );

        let q_sq = self.quaternion[0] * self.quaternion[0]
            + self.quaternion[1] * self.quaternion[1]
            + self.quaternion[2] * self.quaternion[2]
            + self.quaternion[3] * self.quaternion[3];

        if q_sq > 1e-4 {
            let q = glam::Quat::from_xyzw(
                self.quaternion[0] as f32,
                self.quaternion[1] as f32,
                self.quaternion[2] as f32,
                self.quaternion[3] as f32,
            ).normalize();

            // Transform ENU base axes by quaternion, then map ENU (East, North, Up) -> Engine (+X=East, +Y=Up, -Z=North)
            let enu_to_engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y);
            let u0 = enu_to_engine(q.mul_vec3(Vec3::X)).normalize_or_zero();
            let u1 = enu_to_engine(q.mul_vec3(Vec3::Z)).normalize_or_zero(); // Local Z is Up
            let u2 = enu_to_engine(q.mul_vec3(Vec3::Y)).normalize_or_zero(); // Local Y is North

            (center_enu, [u0, u1, u2], half_size)
        } else {
            (center_enu, [Vec3::X, Vec3::Y, Vec3::Z], half_size)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SGeometryInfo {
    pub definition: usize,
    pub resource: u32,
    pub vertex_count: u32,
    pub feature_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SMaterialInfo {
    pub definition: usize,
    pub resource: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SMeshDescriptor {
    pub geometry: Option<I3SGeometryInfo>,
    pub material: Option<I3SMaterialInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I3SNode {
    pub index: u32,
    pub parent_index: Option<u32>,
    pub lod_threshold: Option<f64>,
    pub obb: Option<I3SOrientedBoundingBox>,
    pub mesh: Option<I3SMeshDescriptor>,
    pub children: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I3SNodePage {
    pub nodes: Vec<I3SNode>,
}

// ----------------------------------------------------
// I3S Preset Service URLs
// ----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I3SPreset {
    pub name: &'static str,
    pub url: &'static str,
    pub default_color: [u8; 4],
}

pub const I3S_PRESETS: &[I3SPreset] = &[
    I3SPreset {
        name: "🏢 Melbourne CBD",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AB_Melbourne_WM/SceneServer",
        default_color: [240, 243, 246, 255],
    },
    I3SPreset {
        name: "🏗 Arden St",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/ArdenSt_189_203_WSL1/SceneServer",
        default_color: [255, 180, 100, 255],
    },
    I3SPreset {
        name: "🏙 Melbourne POC",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/sceneViewerPoc_WSL1/SceneServer",
        default_color: [180, 210, 240, 255],
    },
    I3SPreset {
        name: "🏢 AQ North",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer",
        default_color: [220, 230, 240, 255],
    },
    I3SPreset {
        name: "🏢 AQ East",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_East/SceneServer",
        default_color: [220, 240, 230, 255],
    },
];

// ----------------------------------------------------
// I3S Binary Geometry Decoder
// ----------------------------------------------------

#[derive(Debug, Clone)]
pub struct I3SFeatureMesh {
    pub feature_id: u64,
    pub node_id: u32,
    pub start_face: u32,
    pub end_face: u32,
    pub raw_mesh: RawMeshData,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub center_geo: GeoCoord,
    pub height: f32,
}

pub struct DecodedI3SNode {
    pub node_id: u32,
    pub raw_mesh: RawMeshData,
    pub features: Vec<I3SFeatureMesh>,
    pub obb_center_enu: Vec3,
    pub aabb_enu: (Vec3, Vec3),
    pub aabb_ecef: (Vec3, Vec3),
    pub base_color: [f32; 4],
}
