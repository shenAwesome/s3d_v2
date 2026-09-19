pub use crate::gis::geojson_loader::GisFeature;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::gis::renderer::Renderer;

/// Type alias reflecting standard GIS terminology (Esri FeatureLayer / SimpleFeatureLayer)
pub type SimpleFeatureLayer = Layer;

/// Default shadow color used across all layers and scene nodes — dark slate.
/// Single source of truth: any change here propagates everywhere.
pub const DEFAULT_SHADOW_COLOR: [f32; 4] = [0.10, 0.12, 0.18, 1.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElevationMode {
    /// Clamped to ground / terrain surface DEM (default for GeoJSON extrusion)
    #[serde(rename = "OnGround")]
    OnGround,
    /// Absolute height above Sea Level / Geoid (for LiDAR, CAD, and survey data)
    #[serde(rename = "Absolute")]
    Absolute,
}

impl Default for ElevationMode {
    fn default() -> Self {
        Self::OnGround
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerColorMode {
    /// Original texture from 3D Tiles / photogrammetry
    OriginalTexture,
    /// Uniform color tint applied to all features in the layer.
    SingleColor,
    /// Color ramp based on feature height — NOT YET implemented in WGSL shader.
    HeightRamp,
    /// Warm-to-cool diverging color ramp — NOT YET implemented in WGSL shader.
    WarmToCool,
    /// Random pastel color per feature — NOT YET implemented in WGSL shader.
    RandomPastel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayerType {
    Buildings,
    Terrain,
    Vectors,
    Analysis,
    SceneLayer { url: String },
    ThreeDTiles { url: String },
    ArcGIS { url: String, layer_id: u32, mode: crate::gis::arcgis::ArcGISServiceMode, min_scale: f64, max_scale: f64 },
}

impl LayerType {
    pub fn kind(&self) -> &'static str {
        match self {
            LayerType::Buildings => "buildings",
            LayerType::Terrain => "terrain",
            LayerType::Vectors => "vector",
            LayerType::Analysis => "analysis",
            LayerType::SceneLayer { .. } => "i3s",
            LayerType::ThreeDTiles { .. } => "3dtiles",
            LayerType::ArcGIS { .. } => "arcgis",
        }
    }

    pub fn from_kind(kind: &str, url: Option<String>) -> Self {
        match kind {
            "buildings" => LayerType::Buildings,
            "terrain" => LayerType::Terrain,
            "vector" | "vectors" => LayerType::Vectors,
            "analysis" => LayerType::Analysis,
            "i3s" | "scenelayer" => LayerType::SceneLayer {
                url: url.unwrap_or_default(),
            },
            "3dtiles" | "threedtiles" => LayerType::ThreeDTiles {
                url: url.unwrap_or_default(),
            },
            "arcgis" => LayerType::ArcGIS {
                url: url.unwrap_or_default(),
                layer_id: 0,
                mode: crate::gis::arcgis::ArcGISServiceMode::VectorFeature,
                min_scale: 0.0,
                max_scale: 0.0,
            },
            _ => LayerType::Vectors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayerSource {
    BuiltinSample { name: String },
    LocalFile { path: String },
    RemoteService { url: String },
    ArcGISService { url: String, layer_id: u32, mode: crate::gis::arcgis::ArcGISServiceMode },
    Empty,
}

impl Default for LayerSource {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub layer_type: LayerType,
    #[serde(default)]
    pub source: LayerSource,
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    pub color_tint: [f32; 4],
    #[serde(default = "default_stroke_color")]
    pub stroke_color: [f32; 4],
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default = "default_true")]
    pub edge_enabled: bool,
    #[serde(default = "default_shadow_color")]
    pub shadow_color: [f32; 4],
    #[serde(default = "default_height_scale")]
    pub height_scale: f32,
    #[serde(default)]
    pub base_offset: f32,
    #[serde(default)]
    pub elevation_mode: ElevationMode,
    #[serde(default = "default_true")]
    pub cast_shadows: bool,
    #[serde(default)]
    pub wireframe_overlay: bool,
    #[serde(default = "default_color_mode")]
    pub color_mode: LayerColorMode,
    /// Data-driven renderer that assigns symbols to features based on attributes.
    /// When `None`, the layer uses its built-in `color_tint` / `stroke_color` styling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<Renderer>,
    #[serde(default)]
    pub features: Vec<GisFeature>,
    /// Indicates whether the layer's CPU/GPU meshes need rebuilding.
    #[serde(skip)]
    pub dirty: bool,
}

impl Serialize for Layer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Layer", 20)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("layer_type", &self.layer_type)?;
        state.serialize_field("source", &self.source)?;
        state.serialize_field("visible", &self.visible)?;
        state.serialize_field("locked", &self.locked)?;
        state.serialize_field("opacity", &self.opacity)?;
        state.serialize_field("color_tint", &self.color_tint)?;
        state.serialize_field("stroke_color", &self.stroke_color)?;
        state.serialize_field("stroke_width", &self.stroke_width)?;
        state.serialize_field("edge_enabled", &self.edge_enabled)?;
        state.serialize_field("shadow_color", &self.shadow_color)?;
        state.serialize_field("height_scale", &self.height_scale)?;
        state.serialize_field("base_offset", &self.base_offset)?;
        state.serialize_field("elevation_mode", &self.elevation_mode)?;
        state.serialize_field("cast_shadows", &self.cast_shadows)?;
        state.serialize_field("wireframe_overlay", &self.wireframe_overlay)?;
        state.serialize_field("color_mode", &self.color_mode)?;
        if let Some(ref renderer) = self.renderer {
            state.serialize_field("renderer", renderer)?;
        }
        if self.source == LayerSource::Empty && !self.features.is_empty() {
            state.serialize_field("features", &self.features)?;
        }
        state.end()
    }
}

fn default_opacity() -> f32 { 1.0 }
fn default_stroke_color() -> [f32; 4] { [0.15, 0.16, 0.18, 1.0] }
fn default_stroke_width() -> f32 { 1.5 }
fn default_shadow_color() -> [f32; 4] { DEFAULT_SHADOW_COLOR }
fn default_height_scale() -> f32 { 1.0 }
fn default_true() -> bool { true }
fn default_color_mode() -> LayerColorMode { LayerColorMode::SingleColor }

impl Layer {
    pub fn new(id: String, name: String, layer_type: LayerType, color_tint: [f32; 4]) -> Self {
        let color_mode = if matches!(layer_type, LayerType::ThreeDTiles { .. }) {
            LayerColorMode::OriginalTexture
        } else {
            LayerColorMode::SingleColor
        };
        Self {
            id,
            name,
            layer_type,
            source: LayerSource::Empty,
            visible: true,
            locked: false,
            opacity: 1.0,
            color_tint,
            stroke_color: [0.15, 0.16, 0.18, 1.0],
            stroke_width: 1.5,
            edge_enabled: true,
            shadow_color: DEFAULT_SHADOW_COLOR,
            height_scale: 1.0,
            base_offset: 0.0,
            elevation_mode: ElevationMode::OnGround,
            cast_shadows: true,
            wireframe_overlay: false,
            color_mode,
            renderer: None,
            features: Vec::new(),
            dirty: true,
        }
    }

    /// Creates a new SimpleFeatureLayer with an ID and name (matching Esri FeatureLayer pattern).
    pub fn simple_feature_layer(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id.into(), name.into(), LayerType::Vectors, [1.0, 1.0, 1.0, 1.0])
    }

    /// Creates a new FeatureLayer with an ID and name (matching Esri FeatureLayer pattern).
    pub fn feature_layer(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::simple_feature_layer(id, name)
    }

    /// Creates a new FeatureLayer configured with a data-driven Renderer.
    pub fn with_renderer(id: impl Into<String>, name: impl Into<String>, renderer: Renderer) -> Self {
        let mut layer = Self::simple_feature_layer(id, name);
        layer.renderer = Some(renderer);
        layer
    }

    /// Sets or updates the layer's data-driven Renderer.
    pub fn set_renderer(&mut self, renderer: Renderer) {
        self.renderer = Some(renderer);
        self.dirty = true;
    }

    /// Appends a vector feature with geometry (mesh, point, or polygon) to this layer.
    pub fn add_feature(&mut self, feature: GisFeature) {
        self.features.push(feature);
        self.dirty = true;
    }

    /// Appends an Esri-aligned Graphic to this layer, resolving its symbol via the layer's
    /// `Renderer` or the graphic's explicit `Symbol3D`, and generating 3D meshes.
    pub fn add_graphic(&mut self, graphic: crate::gis::graphic::Graphic) {
        let effective_sym = graphic.symbol.as_ref().or_else(|| {
            self.renderer.as_ref().map(|r| r.get_symbol(&graphic.attributes))
        });

        let mut feature = GisFeature::default();
        feature.id = graphic.id;
        feature.name = graphic.name;
        feature.properties = graphic.attributes;

        // Resolve base color from symbol or layer
        let mut base_color = self.color_tint;
        if let Some(sym) = effective_sym {
            match sym {
                crate::gis::symbol::Symbol3D::Polygon { symbol_layers } => {
                    if let Some(l) = symbol_layers.first() {
                        match l {
                            crate::gis::symbol::PolygonSymbolLayer::Fill(f) => base_color = f.material.color,
                            crate::gis::symbol::PolygonSymbolLayer::Extrude(e) => base_color = e.material.color,
                        }
                    }
                }
                crate::gis::symbol::Symbol3D::Point { symbol_layers, .. } => {
                    if let Some(l) = symbol_layers.first() {
                        match l {
                            crate::gis::symbol::PointSymbolLayer::Icon(i) => base_color = i.material.color,
                            crate::gis::symbol::PointSymbolLayer::Object(o) => base_color = o.material.color,
                        }
                    }
                }
                crate::gis::symbol::Symbol3D::Line { symbol_layers } => {
                    if let Some(l) = symbol_layers.first() {
                        match l {
                            crate::gis::symbol::LineSymbolLayer::Line(ln) => base_color = ln.material.color,
                            crate::gis::symbol::LineSymbolLayer::Path(p) => base_color = p.material.color,
                        }
                    }
                }
                crate::gis::symbol::Symbol3D::Mesh { material } => {
                    base_color = material.material.color;
                }
            }
        }

        // Check if renderer has continuous ColorRamp visual variable
        if let Some(r) = &self.renderer {
            for vv in r.visual_variables() {
                if let crate::gis::renderer::VisualVariableType::ColorRamp { min_color, max_color, .. } = &vv.variable_type {
                    if let Some(t) = vv.compute_t(&feature.properties) {
                        base_color = crate::gis::renderer::VisualVariableType::resolve_color(t, *min_color, *max_color);
                    }
                }
            }
        }
        feature.color = Some(base_color);

        let mut callout_feature = None;

        // Generate procedural geometry mesh based on graphic.geometry
        match &graphic.geometry {
            crate::gis::geometry::Geometry::Point(pt) => {
                feature.center_local = glam::Vec3::new(pt.x as f32, pt.y as f32, pt.z as f32);
                if let Some(crate::gis::symbol::Symbol3D::Point { symbol_layers, vertical_offset, callout }) = effective_sym {
                    let base_y = pt.y as f32;
                    let offset_y = vertical_offset.as_ref().map(|o| o.screen_length).unwrap_or(0.0);
                    let elevated_y = base_y + offset_y;

                    // If callout line is requested and there is vertical offset, generate 3D callout leader line
                    if let (Some(crate::gis::symbol::Callout3D::Line(line_callout)), true) = (callout, offset_y > 0.0) {
                        let mut callout_feat = GisFeature::default();
                        callout_feat.id = format!("{}_callout", feature.id);
                        callout_feat.name = format!("{} Callout", feature.name);
                        callout_feat.color = Some(line_callout.color);
                        callout_feat.center_local = glam::Vec3::new(pt.x as f32, base_y, pt.z as f32);
                        callout_feat.height = offset_y;
                        let line_radius = (line_callout.size * 0.5).clamp(0.15, 2.0);
                        let callout_mesh = crate::gis::extrusion::RawMeshData::create_cylinder(
                            glam::Vec3::new(pt.x as f32, base_y, pt.z as f32),
                            line_radius,
                            offset_y,
                            16,
                        );
                        callout_feat.mesh = Some(callout_mesh);
                        callout_feature = Some(callout_feat);
                    }

                    feature.center_local = glam::Vec3::new(pt.x as f32, elevated_y, pt.z as f32);

                    if let Some(crate::gis::symbol::PointSymbolLayer::Object(obj)) = symbol_layers.first() {
                        let w = obj.width;
                        let h = obj.height;
                        let d = obj.depth;
                        feature.height = h;

                        let mesh = match obj.resource {
                            crate::gis::symbol::ObjectResource::Cube => {
                                let w2 = w * 0.5;
                                let d2 = d * 0.5;
                                let min = glam::Vec3::new((pt.x as f32) - w2, elevated_y, (pt.z as f32) - d2);
                                let max = glam::Vec3::new((pt.x as f32) + w2, elevated_y + h, (pt.z as f32) + d2);
                                crate::gis::extrusion::RawMeshData::create_box(min, max)
                            }
                            crate::gis::symbol::ObjectResource::Cylinder => {
                                let bottom_center = glam::Vec3::new(pt.x as f32, elevated_y, pt.z as f32);
                                crate::gis::extrusion::RawMeshData::create_cylinder(bottom_center, (w * 0.5).max(0.1), h, 24)
                            }
                            crate::gis::symbol::ObjectResource::Sphere => {
                                let center = glam::Vec3::new(pt.x as f32, elevated_y + h * 0.5, pt.z as f32);
                                crate::gis::extrusion::RawMeshData::create_sphere(center, (w * 0.5).max(0.1), 16, 24)
                            }
                            _ => {
                                let w2 = w * 0.5;
                                let d2 = d * 0.5;
                                let min = glam::Vec3::new((pt.x as f32) - w2, elevated_y, (pt.z as f32) - d2);
                                let max = glam::Vec3::new((pt.x as f32) + w2, elevated_y + h, (pt.z as f32) + d2);
                                crate::gis::extrusion::RawMeshData::create_box(min, max)
                            }
                        };
                        feature.mesh = Some(mesh);
                    }
                }
            }
            crate::gis::geometry::Geometry::Polygon(poly) => {
                let mut extrude_height = 20.0;
                if let Some(crate::gis::symbol::Symbol3D::Polygon { symbol_layers }) = effective_sym {
                    if let Some(l) = symbol_layers.first() {
                        match l {
                            crate::gis::symbol::PolygonSymbolLayer::Extrude(e) => {
                                extrude_height = e.size;
                            }
                            crate::gis::symbol::PolygonSymbolLayer::Fill(_) => {
                                extrude_height = 0.25;
                            }
                        }
                    }
                }

                // Check size visual variables
                if let Some(r) = &self.renderer {
                    for vv in r.visual_variables() {
                        if let crate::gis::renderer::VisualVariableType::SizeRange { min_size, max_size, .. } = &vv.variable_type {
                            if let Some(t) = vv.compute_t(&feature.properties) {
                                extrude_height = crate::gis::renderer::VisualVariableType::resolve_size(t, *min_size, *max_size);
                            }
                        }
                    }
                }

                feature.height = extrude_height;
                let local_rings: Vec<Vec<glam::Vec2>> = poly.rings.iter().map(|ring| {
                    ring.iter().map(|&[x, y]| glam::Vec2::new(x as f32, y as f32)).collect()
                }).collect();

                if let Some(poly_mesh) = crate::gis::extrusion::extrude_polygon(&local_rings, 0.0, extrude_height) {
                    feature.mesh = Some(poly_mesh);
                }
            }
            crate::gis::geometry::Geometry::Mesh(m) => {
                feature.mesh = Some(crate::gis::extrusion::RawMeshData::from(m.clone()));
            }
            _ => {}
        }

        if let Some(c) = callout_feature {
            self.add_feature(c);
        }
        self.add_feature(feature);
    }

    /// Creates and appends a 3D box feature to this layer with extruded volumetric mesh geometry.
    pub fn add_box_feature(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        min: glam::Vec3,
        max: glam::Vec3,
        color: [f32; 4],
    ) {
        let feature = GisFeature::create_box(id, name, min, max, color);
        self.add_feature(feature);
    }

    /// Serialisable descriptor representation of this Layer
    pub fn descriptor(&self) -> LayerDescriptor {
        LayerDescriptor {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.layer_type.kind().to_string(),
            visible: self.visible,
            locked: self.locked,
            opacity: self.opacity,
            color_tint: self.color_tint,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            edge_enabled: self.edge_enabled,
            shadow_color: self.shadow_color,
            height_scale: self.height_scale,
            base_offset: self.base_offset,
            elevation_mode: self.elevation_mode,
            cast_shadows: self.cast_shadows,
            wireframe_overlay: self.wireframe_overlay,
            color_mode: self.color_mode,
            source_id: None,
            url: match &self.layer_type {
                LayerType::SceneLayer { url } => Some(url.clone()),
                LayerType::ThreeDTiles { url } => Some(url.clone()),
                LayerType::ArcGIS { url, .. } => Some(url.clone()),
                _ => None,
            },
            custom_props: HashMap::new(),
        }
    }

    /// Reconstructs a concrete Layer from a LayerDescriptor and optional features
    pub fn from_descriptor(desc: LayerDescriptor, features: Vec<GisFeature>) -> Self {
        let layer_type = LayerType::from_kind(&desc.kind, desc.url);
        Self {
            id: desc.id,
            name: desc.name,
            layer_type,
            source: LayerSource::Empty,
            visible: desc.visible,
            locked: desc.locked,
            opacity: desc.opacity,
            color_tint: desc.color_tint,
            stroke_color: desc.stroke_color,
            stroke_width: desc.stroke_width,
            edge_enabled: desc.edge_enabled,
            shadow_color: desc.shadow_color,
            height_scale: desc.height_scale,
            base_offset: desc.base_offset,
            elevation_mode: desc.elevation_mode,
            cast_shadows: desc.cast_shadows,
            wireframe_overlay: desc.wireframe_overlay,
            color_mode: desc.color_mode,
            renderer: None,
            features,
            dirty: true,
        }
    }
}

/// Serialisable, transportable descriptor for creating and configuring layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDescriptor {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_color_tint")]
    pub color_tint: [f32; 4],
    #[serde(default = "default_stroke_color")]
    pub stroke_color: [f32; 4],
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default = "default_true")]
    pub edge_enabled: bool,
    #[serde(default = "default_shadow_color")]
    pub shadow_color: [f32; 4],
    #[serde(default = "default_height_scale")]
    pub height_scale: f32,
    #[serde(default)]
    pub base_offset: f32,
    #[serde(default)]
    pub elevation_mode: ElevationMode,
    #[serde(default = "default_true")]
    pub cast_shadows: bool,
    #[serde(default)]
    pub wireframe_overlay: bool,
    #[serde(default = "default_color_mode")]
    pub color_mode: LayerColorMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<crate::gis::source::SourceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_props: HashMap<String, String>,
}

fn default_color_tint() -> [f32; 4] { [0.85, 0.88, 0.92, 1.0] }

impl LayerDescriptor {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            visible: true,
            locked: false,
            opacity: 1.0,
            color_tint: default_color_tint(),
            stroke_color: default_stroke_color(),
            stroke_width: default_stroke_width(),
            edge_enabled: true,
            shadow_color: default_shadow_color(),
            height_scale: default_height_scale(),
            base_offset: 0.0,
            elevation_mode: ElevationMode::OnGround,
            cast_shadows: true,
            wireframe_overlay: false,
            color_mode: LayerColorMode::SingleColor,
            source_id: None,
            url: None,
            custom_props: HashMap::new(),
        }
    }
}

/// Operational status of a layer after an update pass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerStatus {
    /// Layer is idle, all resources resident
    Idle,
    /// Layer is actively streaming/downloading data
    Streaming { pending_requests: usize },
    /// Layer encountered an error
    Error,
}

/// Extensible Layer trait per ARCHITECTURE_v2.md §3.2
pub trait LayerTrait: Send + Sync + 'static {
    /// Unique identifier for this layer instance
    fn id(&self) -> &str;
    /// Architectural kind (e.g. "buildings", "terrain", "vector", "3dtiles")
    fn kind(&self) -> &'static str;
    /// Serialisable descriptor representation
    fn descriptor(&self) -> LayerDescriptor;
    /// Returns current visibility state
    fn is_visible(&self) -> bool;
    /// Sets visibility state
    fn set_visible(&mut self, visible: bool);
    /// Returns opacity in [0.0, 1.0]
    fn opacity(&self) -> f32;
    /// Sets opacity in [0.0, 1.0]
    fn set_opacity(&mut self, opacity: f32);
    /// Returns color tint [r, g, b, a]
    fn color_tint(&self) -> [f32; 4];
    /// Sets color tint [r, g, b, a]
    fn set_color_tint(&mut self, tint: [f32; 4]);
    /// Returns cast shadows flag
    fn cast_shadows(&self) -> bool;
    /// Sets cast shadows flag
    fn set_cast_shadows(&mut self, cast: bool);
    /// Update pass: streaming, LODs, clamping. Never touches GPU.
    fn update(&mut self) -> LayerStatus {
        LayerStatus::Idle
    }
}

impl LayerTrait for Layer {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> &'static str {
        self.layer_type.kind()
    }
    fn descriptor(&self) -> LayerDescriptor {
        self.descriptor()
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    fn opacity(&self) -> f32 {
        self.opacity
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
    fn color_tint(&self) -> [f32; 4] {
        self.color_tint
    }
    fn set_color_tint(&mut self, tint: [f32; 4]) {
        self.color_tint = tint;
        self.dirty = true;
    }
    fn cast_shadows(&self) -> bool {
        self.cast_shadows
    }
    fn set_cast_shadows(&mut self, cast: bool) {
        self.cast_shadows = cast;
    }
    fn update(&mut self) -> LayerStatus {
        LayerStatus::Idle
    }
}

/// Factory trait for constructing layer instances from serialisable descriptors
pub trait LayerFactory: Send + Sync + 'static {
    /// The layer kind string this factory produces (e.g. "buildings", "3dtiles")
    fn kind(&self) -> &'static str;
    /// Constructs a concrete Layer from the given descriptor
    fn create(&self, descriptor: &LayerDescriptor) -> Result<Layer, String>;
}

/// Default built-in layer factory producing standard `Layer` instances
pub struct BuiltinLayerFactory {
    kind: &'static str,
}

impl BuiltinLayerFactory {
    pub fn new(kind: &'static str) -> Self {
        Self { kind }
    }
}

impl LayerFactory for BuiltinLayerFactory {
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn create(&self, descriptor: &LayerDescriptor) -> Result<Layer, String> {
        Ok(Layer::from_descriptor(descriptor.clone(), Vec::new()))
    }
}

/// Registry of layer factories allowing extensible plugin-based layer types
pub struct LayerRegistry {
    factories: HashMap<String, Box<dyn LayerFactory>>,
}

impl Default for LayerRegistry {
    fn default() -> Self {
        let mut reg = Self {
            factories: HashMap::new(),
        };
        // Register standard built-in factories
        for kind in &["buildings", "terrain", "vector", "3dtiles", "i3s", "arcgis", "analysis"] {
            reg.register_factory(Box::new(BuiltinLayerFactory::new(kind)));
        }
        reg
    }
}

impl LayerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a custom layer factory
    pub fn register_factory(&mut self, factory: Box<dyn LayerFactory>) {
        self.factories.insert(factory.kind().to_string(), factory);
    }

    /// Creates a layer instance from a descriptor using registered factories
    pub fn create_layer(&self, descriptor: &LayerDescriptor) -> Result<Layer, String> {
        if let Some(factory) = self.factories.get(&descriptor.kind) {
            factory.create(descriptor)
        } else {
            // Fallback to creating a standard layer from descriptor directly
            Ok(Layer::from_descriptor(descriptor.clone(), Vec::new()))
        }
    }

    /// Returns list of supported layer kinds
    pub fn supported_kinds(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }
}

