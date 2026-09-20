pub use crate::gis::geojson_loader::GisFeature;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::gis::renderer::Renderer;
use crate::gis::geometry::{Extent, SpatialReference};

/// Type alias reflecting standard GIS terminology (Esri FeatureLayer / SimpleFeatureLayer)
pub type SimpleFeatureLayer = FeatureLayer;

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
    /// Color ramp based on feature height.
    HeightRamp,
    /// Warm-to-cool diverging color ramp.
    WarmToCool,
    /// Random pastel color per feature.
    RandomPastel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayerType {
    Tile,
    Elevation,
    Feature,
    Buildings,
    Terrain,
    Vectors,
    Graphics,
    Group,
    Analysis,
    SceneLayer { url: String },
    ThreeDTiles { url: String },
    ArcGIS { url: String, layer_id: u32, mode: crate::gis::arcgis::ArcGISServiceMode, min_scale: f64, max_scale: f64 },
}

impl LayerType {
    pub fn kind(&self) -> &'static str {
        match self {
            LayerType::Tile => "tile",
            LayerType::Elevation | LayerType::Terrain => "terrain",
            LayerType::Feature | LayerType::Buildings => "buildings",
            LayerType::Vectors => "vector",
            LayerType::Graphics => "graphics",
            LayerType::Group => "group",
            LayerType::Analysis => "analysis",
            LayerType::SceneLayer { .. } => "i3s",
            LayerType::ThreeDTiles { .. } => "3dtiles",
            LayerType::ArcGIS { .. } => "arcgis",
        }
    }

    pub fn from_kind(kind: &str, url: Option<String>) -> Self {
        match kind {
            "tile" => LayerType::Tile,
            "buildings" => LayerType::Buildings,
            "terrain" | "elevation" => LayerType::Terrain,
            "vector" | "vectors" | "feature" => LayerType::Vectors,
            "graphics" => LayerType::Graphics,
            "group" => LayerType::Group,
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

/// Operational status of a layer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStatus {
    NotLoaded,
    Loading,
    Loaded,
    Failed(String),
}

impl Default for LoadStatus {
    fn default() -> Self {
        Self::Loaded
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

/// Context passed to each layer on every frame update.
pub struct LayerUpdateContext<'a> {
    pub camera: &'a crate::renderer::camera::Camera,
    pub origin: &'a crate::gis::crs::ProjectOrigin,
    pub projection_mode: crate::gis::crs::ProjectionMode,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub dt: f32,
}

/// Context provided to each layer for synchronizing GPU meshes and spatial collision indices.
pub struct LayerGpuContext<'a> {
    pub renderer: &'a mut crate::renderer::render_engine::RenderEngine,
    pub collider: &'a mut crate::solar::shadow_analysis::SceneCollider,
    pub origin: &'a crate::gis::crs::ProjectOrigin,
    pub projection_mode: crate::gis::crs::ProjectionMode,
}

// ─── Base Layer Trait ────────────────────────────────────────────────────────

/// The central base trait for all GIS and 3D scene layers in S3D Core.
///
/// Modeled after the Esri ArcGIS `Layer` class hierarchy.
/// Decoupled from GPU rendering and windowing: a `Layer` represents a spatial data source,
/// its styling rules, its configuration, and its own streaming/rendering implementation.
pub trait Layer: std::fmt::Debug + Send + Sync + 'static {
    /// Unique identifier for this layer instance.
    fn id(&self) -> &str;

    /// Human-readable title for display in UI, legend, or layer trees.
    fn title(&self) -> &str;

    /// Architectural layer type.
    fn layer_type(&self) -> LayerType;

    /// Current visibility state.
    fn visible(&self) -> bool;

    /// Sets layer visibility.
    fn set_visible(&mut self, visible: bool);

    /// Opacity factor in range [0.0, 1.0].
    fn opacity(&self) -> f32;

    /// Sets opacity factor clamped to [0.0, 1.0].
    fn set_opacity(&mut self, opacity: f32);

    /// Minimum scale / maximum viewing distance for LOD culling.
    fn min_scale(&self) -> f64 { 0.0 }

    /// Maximum scale / minimum viewing distance for LOD culling.
    fn max_scale(&self) -> f64 { 0.0 }

    /// Current loading status of the layer.
    fn load_status(&self) -> LoadStatus { LoadStatus::Loaded }

    /// Spatial reference system for this layer's coordinates.
    fn spatial_reference(&self) -> Option<&SpatialReference> { None }

    /// Geographic or projected bounding extent of the layer.
    fn full_extent(&self) -> Option<Extent> { None }

    /// Serialisable descriptor representation.
    fn descriptor(&self) -> LayerDescriptor;

    /// Color tint applied to features or meshes.
    fn color_tint(&self) -> [f32; 4] { [1.0, 1.0, 1.0, 1.0] }

    /// Sets color tint.
    fn set_color_tint(&mut self, _tint: [f32; 4]) {}

    /// Whether this layer casts shadows.
    fn cast_shadows(&self) -> bool { true }

    /// Sets shadow casting.
    fn set_cast_shadows(&mut self, _cast: bool) {}

    /// Per-frame update pass for LOD evaluation and streaming. The layer class decides its own implementation.
    fn update(&mut self, _ctx: &LayerUpdateContext) -> LayerStatus { LayerStatus::Idle }

    /// Synchronizes decoded geometry and textures with the GPU renderer.
    fn sync_gpu(&mut self, _ctx: &mut LayerGpuContext) {}

    /// Cleans up GPU buffers and scene collider features when the layer is removed.
    fn destroy(&mut self, _ctx: &mut LayerGpuContext) {}

    /// Notification that the local project origin has changed.
    fn on_origin_changed(&mut self, _origin: &crate::gis::crs::ProjectOrigin) {}

    /// Indicates whether the layer has active background streaming tasks in flight.
    fn is_streaming(&self) -> bool { false }

    /// Downcasting helper to Any.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Downcasting helper to mutable Any.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Backward-compatible alias for [`Layer`].
pub type LayerTrait = dyn Layer;

impl Layer for Box<dyn Layer> {
    fn id(&self) -> &str { (**self).id() }
    fn title(&self) -> &str { (**self).title() }
    fn layer_type(&self) -> LayerType { (**self).layer_type() }
    fn visible(&self) -> bool { (**self).visible() }
    fn set_visible(&mut self, visible: bool) { (**self).set_visible(visible); }
    fn opacity(&self) -> f32 { (**self).opacity() }
    fn set_opacity(&mut self, opacity: f32) { (**self).set_opacity(opacity); }
    fn min_scale(&self) -> f64 { (**self).min_scale() }
    fn max_scale(&self) -> f64 { (**self).max_scale() }
    fn load_status(&self) -> LoadStatus { (**self).load_status() }
    fn spatial_reference(&self) -> Option<&SpatialReference> { (**self).spatial_reference() }
    fn full_extent(&self) -> Option<Extent> { (**self).full_extent() }
    fn descriptor(&self) -> LayerDescriptor { (**self).descriptor() }
    fn color_tint(&self) -> [f32; 4] { (**self).color_tint() }
    fn set_color_tint(&mut self, tint: [f32; 4]) { (**self).set_color_tint(tint); }
    fn cast_shadows(&self) -> bool { (**self).cast_shadows() }
    fn set_cast_shadows(&mut self, cast: bool) { (**self).set_cast_shadows(cast); }

    fn update(&mut self, ctx: &LayerUpdateContext) -> LayerStatus { (**self).update(ctx) }
    fn sync_gpu(&mut self, ctx: &mut LayerGpuContext) { (**self).sync_gpu(ctx); }
    fn destroy(&mut self, ctx: &mut LayerGpuContext) { (**self).destroy(ctx); }
    fn on_origin_changed(&mut self, origin: &crate::gis::crs::ProjectOrigin) { (**self).on_origin_changed(origin); }
    fn is_streaming(&self) -> bool { (**self).is_streaming() }

    fn as_any(&self) -> &dyn std::any::Any { (**self).as_any() }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { (**self).as_any_mut() }
}

// ─── Concrete Layer: TileLayer (Web / Raster Tiles) ──────────────────────────

/// A layer that displays raster map tiles from web tile services (XYZ, OSM, Esri, WMTS).
///
/// Matches Esri ArcGIS `TileLayer` and `WebTileLayer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileLayer {
    pub id: String,
    pub title: String,
    pub url_template: String,
    pub subdomains: Vec<String>,
    pub tile_size: u32,
    pub min_zoom: u32,
    pub max_zoom: u32,
    pub attribution: String,
    pub visible: bool,
    pub opacity: f32,
    #[serde(skip)]
    pub load_status: LoadStatus,
}

impl TileLayer {
    /// Creates a new TileLayer with a URL template (e.g. `https://tile.openstreetmap.org/{z}/{x}/{y}.png`).
    pub fn new(id: impl Into<String>, url_template: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: "Web Tile Layer".to_string(),
            url_template: url_template.into(),
            subdomains: vec!["a".into(), "b".into(), "c".into()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 19,
            attribution: String::new(),
            visible: true,
            opacity: 1.0,
            load_status: LoadStatus::Loaded,
        }
    }

    /// OpenStreetMap standard raster tiles preset.
    pub fn osm() -> Self {
        let mut l = Self::new("osm_tiles", "https://tile.openstreetmap.org/{z}/{x}/{y}.png");
        l.title = "OpenStreetMap".into();
        l.attribution = "© OpenStreetMap contributors".into();
        l
    }

    /// Esri World Imagery (Satellite) raster tiles preset.
    pub fn esri_imagery() -> Self {
        let mut l = Self::new(
            "esri_imagery_tiles",
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        );
        l.title = "Esri World Imagery (Satellite)".into();
        l.attribution = "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics".into();
        l
    }

    /// Esri World Streets raster tiles preset.
    pub fn esri_streets() -> Self {
        let mut l = Self::new(
            "esri_streets_tiles",
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
        );
        l.title = "Esri World Streets".into();
        l.attribution = "Tiles © Esri — Source: Esri".into();
        l
    }

    /// Esri World Topographic raster tiles preset.
    pub fn esri_topo() -> Self {
        let mut l = Self::new(
            "esri_topo_tiles",
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}",
        );
        l.title = "Esri World Topo".into();
        l.attribution = "Tiles © Esri — Source: Esri".into();
        l
    }

    /// Formats a tile URL for the given (z, x, y) slippy map coordinates.
    pub fn tile_url(&self, z: u32, x: u32, y: u32) -> String {
        self.url_template
            .replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string())
    }
}

impl Layer for TileLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.title }
    fn layer_type(&self) -> LayerType { LayerType::Tile }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) { self.opacity = opacity.clamp(0.0, 1.0); }
    fn load_status(&self) -> LoadStatus { self.load_status.clone() }
    fn descriptor(&self) -> LayerDescriptor {
        let mut desc = LayerDescriptor::new(&self.id, &self.title, "tile");
        desc.visible = self.visible;
        desc.opacity = self.opacity;
        desc.url = Some(self.url_template.clone());
        desc
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── Concrete Layer: ElevationLayer (DEM / Terrain) ──────────────────────────

/// Format of the digital elevation model raster encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElevationFormat {
    /// Mapbox Terrain-RGB: elevation = -10000 + ((R * 256 * 256 + G * 256 + B) * 0.1)
    MapboxTerrainRgb,
    /// Mapzen Terrarium: elevation = (R * 256 + G + B / 256) - 32768
    Terrarium,
    /// Esri Limited Error Raster Compression (LERC)
    EsriLerc,
}

/// A layer that provides digital elevation models (DEM) for 3D terrain surfaces.
///
/// Matches Esri ArcGIS `ElevationLayer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationLayer {
    pub id: String,
    pub title: String,
    pub url_template: String,
    pub format: ElevationFormat,
    pub elevation_exaggeration: f32,
    pub visible: bool,
    pub opacity: f32,
    #[serde(skip)]
    pub load_status: LoadStatus,
}

impl ElevationLayer {
    /// Creates a new ElevationLayer with a URL template and elevation encoding format.
    pub fn new(id: impl Into<String>, url_template: impl Into<String>, format: ElevationFormat) -> Self {
        Self {
            id: id.into(),
            title: "Elevation Layer".to_string(),
            url_template: url_template.into(),
            format,
            elevation_exaggeration: 1.0,
            visible: true,
            opacity: 1.0,
            load_status: LoadStatus::Loaded,
        }
    }

    /// Mapbox Terrain-RGB preset.
    pub fn mapbox_terrain_rgb(id: impl Into<String>, url_template: impl Into<String>) -> Self {
        Self::new(id, url_template, ElevationFormat::MapboxTerrainRgb)
    }

    /// Mapzen Terrarium preset.
    pub fn terrarium(id: impl Into<String>, url_template: impl Into<String>) -> Self {
        Self::new(id, url_template, ElevationFormat::Terrarium)
    }
}

impl Layer for ElevationLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.title }
    fn layer_type(&self) -> LayerType { LayerType::Elevation }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) { self.opacity = opacity.clamp(0.0, 1.0); }
    fn load_status(&self) -> LoadStatus { self.load_status.clone() }
    fn descriptor(&self) -> LayerDescriptor {
        let mut desc = LayerDescriptor::new(&self.id, &self.title, "terrain");
        desc.visible = self.visible;
        desc.opacity = self.opacity;
        desc.url = Some(self.url_template.clone());
        desc.height_scale = self.elevation_exaggeration;
        desc
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── Concrete Layer: GraphicsLayer (Client-side Graphics) ────────────────────

/// A client-side layer that displays individual graphics (geometries + symbols + attributes).
///
/// Unlike `FeatureLayer`, which uses a shared `Renderer`, each `Graphic` in a `GraphicsLayer`
/// can carry its own individual `Symbol3D`. Ideal for temporary annotations, measurements,
/// pins, and sketch drawing.
///
/// Matches Esri ArcGIS `GraphicsLayer`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphicsLayer {
    pub id: String,
    pub title: String,
    pub graphics: Vec<crate::gis::graphic::Graphic>,
    pub visible: bool,
    pub opacity: f32,
    #[serde(skip)]
    pub dirty: bool,
}

impl GraphicsLayer {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            graphics: Vec::new(),
            visible: true,
            opacity: 1.0,
            dirty: true,
        }
    }

    /// Appends a graphic to this layer.
    pub fn add(&mut self, graphic: crate::gis::graphic::Graphic) {
        self.graphics.push(graphic);
        self.dirty = true;
    }

    /// Removes a graphic by its ID.
    pub fn remove(&mut self, id: &str) -> Option<crate::gis::graphic::Graphic> {
        if let Some(pos) = self.graphics.iter().position(|g| g.id == id) {
            self.dirty = true;
            Some(self.graphics.remove(pos))
        } else {
            None
        }
    }

    /// Clears all graphics from this layer.
    pub fn clear(&mut self) {
        self.graphics.clear();
        self.dirty = true;
    }

    /// Returns a slice of all graphics in this layer.
    pub fn graphics(&self) -> &[crate::gis::graphic::Graphic] {
        &self.graphics
    }
}

impl Layer for GraphicsLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.title }
    fn layer_type(&self) -> LayerType { LayerType::Graphics }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) { self.opacity = opacity.clamp(0.0, 1.0); }
    fn descriptor(&self) -> LayerDescriptor {
        let mut desc = LayerDescriptor::new(&self.id, &self.title, "graphics");
        desc.visible = self.visible;
        desc.opacity = self.opacity;
        desc
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── Concrete Layer: SceneLayer (Esri I3S 3D Objects) ─────────────────────────

/// A layer that displays 3D scene objects streamed via the Esri I3S standard.
///
/// Encapsulates its own `I3SManager` to handle hierarchical LOD nodepage index
/// traversal, worker thread streaming, and GPU buffer uploads.
pub struct SceneLayer {
    pub id: String,
    pub title: String,
    pub url: String,
    pub layer_id: u32,
    pub visible: bool,
    pub opacity: f32,
    pub lod_threshold_scale: f32,
    pub manager: crate::gis::i3s::I3SManager,
}

impl std::fmt::Debug for SceneLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneLayer")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("url", &self.url)
            .field("visible", &self.visible)
            .field("opacity", &self.opacity)
            .finish()
    }
}

impl SceneLayer {
    pub fn new(id: impl Into<String>, title: impl Into<String>, url: impl Into<String>) -> Self {
        let u = url.into();
        let mut manager = crate::gis::i3s::I3SManager::new(crate::gis::crs::ProjectOrigin::default());
        manager.set_service_url(&u);
        manager.is_enabled = true;
        Self {
            id: id.into(),
            title: title.into(),
            url: u,
            layer_id: 0,
            visible: true,
            opacity: 1.0,
            lod_threshold_scale: 1.0,
            manager,
        }
    }
}

impl Layer for SceneLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.title }
    fn layer_type(&self) -> LayerType { LayerType::SceneLayer { url: self.url.clone() } }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.manager.is_enabled = visible;
    }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
        self.manager.opacity = self.opacity;
    }
    fn descriptor(&self) -> LayerDescriptor {
        let mut desc = LayerDescriptor::new(&self.id, &self.title, "i3s");
        desc.visible = self.visible;
        desc.opacity = self.opacity;
        desc.url = Some(self.url.clone());
        desc
    }

    fn update(&mut self, ctx: &LayerUpdateContext) -> LayerStatus {
        if !self.visible {
            return LayerStatus::Idle;
        }
        self.manager.opacity = self.opacity;
        self.manager.lod_threshold_scale = self.lod_threshold_scale;
        self.manager.is_enabled = true;

        let visible_nodes = self.manager.calculate_visible_nodes(
            ctx.camera,
            ctx.viewport_width,
            ctx.viewport_height,
            ctx.origin,
        );
        self.manager.request_nodes(&visible_nodes);

        if self.manager.is_streaming() {
            LayerStatus::Streaming { pending_requests: self.manager.pending_task_count() }
        } else {
            LayerStatus::Idle
        }
    }

    fn sync_gpu(&mut self, ctx: &mut LayerGpuContext) {
        if !self.visible {
            return;
        }
        let new_i3s_nodes = self.manager.drain_completed();
        for node in new_i3s_nodes {
            for f in &node.features {
                ctx.collider.add_mesh(&f.raw_mesh, &format!("{}_feat_{}_{}", self.id, node.node_id, f.feature_id));
            }
            ctx.renderer.add_i3s_tile(node, [1.0, 1.0, 1.0], self.opacity, true);
        }
        let evicted = ctx.renderer.prune_unneeded_i3s_tiles(&self.manager.cached_visible_nodes);
        for ev in evicted {
            self.manager.unmark_loaded(ev);
            ctx.collider.remove_features_with_prefix(&format!("{}_feat_{}_", self.id, ev));
        }
    }

    fn destroy(&mut self, ctx: &mut LayerGpuContext) {
        ctx.renderer.clear_i3s_tiles();
        ctx.collider.remove_features_with_prefix(&format!("{}_feat_", self.id));
    }

    fn on_origin_changed(&mut self, origin: &crate::gis::crs::ProjectOrigin) {
        self.manager.set_origin(*origin);
    }

    fn is_streaming(&self) -> bool {
        self.manager.is_streaming()
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── Concrete Layer: IntegratedMeshLayer (3D Tiles) ──────────────────────────

/// A layer that displays continuous 3D reality meshes streamed via OGC 3D Tiles.
///
/// Encapsulates its own `Tiles3DManager` to handle bounding-volume hierarchy
/// traversal, Screen-Space Error (SSE) refinement, and b3dm parsing.
pub struct IntegratedMeshLayer {
    pub id: String,
    pub title: String,
    pub url: String,
    pub visible: bool,
    pub opacity: f32,
    pub maximum_screen_space_error: f32,
    pub height_offset: f32,
    pub tint: [f32; 3],
    pub manager: crate::gis::threedtiles::Tiles3DManager,
}

impl std::fmt::Debug for IntegratedMeshLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegratedMeshLayer")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("url", &self.url)
            .field("visible", &self.visible)
            .field("opacity", &self.opacity)
            .finish()
    }
}

impl IntegratedMeshLayer {
    pub fn new(id: impl Into<String>, title: impl Into<String>, url: impl Into<String>) -> Self {
        let u = url.into();
        let mut manager = crate::gis::threedtiles::Tiles3DManager::new();
        manager.set_service_url(&u);
        manager.is_enabled = true;
        Self {
            id: id.into(),
            title: title.into(),
            url: u,
            visible: true,
            opacity: 1.0,
            maximum_screen_space_error: 16.0,
            height_offset: 0.0,
            tint: [1.0, 1.0, 1.0],
            manager,
        }
    }

    pub fn set_height_offset(&mut self, offset: f32) {
        self.height_offset = offset;
        self.manager.height_offset = offset;
        self.manager.set_height_offset(offset);
    }
}

impl Layer for IntegratedMeshLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.title }
    fn layer_type(&self) -> LayerType { LayerType::ThreeDTiles { url: self.url.clone() } }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.manager.is_enabled = visible;
    }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
        self.manager.opacity = self.opacity;
    }
    fn descriptor(&self) -> LayerDescriptor {
        let mut desc = LayerDescriptor::new(&self.id, &self.title, "3dtiles");
        desc.visible = self.visible;
        desc.opacity = self.opacity;
        desc.url = Some(self.url.clone());
        desc
    }

    fn update(&mut self, ctx: &LayerUpdateContext) -> LayerStatus {
        if !self.visible {
            return LayerStatus::Idle;
        }
        self.manager.opacity = self.opacity;
        self.manager.maximum_screen_space_error = self.maximum_screen_space_error;
        self.manager.height_offset = self.height_offset;
        self.manager.tint = self.tint;
        self.manager.is_enabled = true;

        let (_active_3d_tiles, _newly_loaded) = self.manager.update_streaming(
            ctx.origin,
            ctx.camera,
            ctx.viewport_width,
            ctx.viewport_height,
            ctx.projection_mode,
        );

        if self.manager.is_streaming() {
            LayerStatus::Streaming { pending_requests: self.manager.pending_requests.len() }
        } else {
            LayerStatus::Idle
        }
    }

    fn sync_gpu(&mut self, ctx: &mut LayerGpuContext) {
        if !self.visible {
            return;
        }
        let tint = self.tint;
        let opacity = self.opacity;

        for (tile_id, mesh) in &self.manager.loaded_tiles {
            let feat_id = format!("{}_threedtile_{}", self.id, tile_id);
            if !ctx.renderer.threedtiles_gpu_tiles.contains_key(tile_id) {
                ctx.collider.remove_features_with_prefix(&feat_id);
                ctx.collider.add_threedtile_mesh(mesh, &feat_id);
                ctx.renderer.add_threedtile(mesh.clone(), tint, opacity, self.manager.replace_texture);
            }
        }
    }

    fn destroy(&mut self, ctx: &mut LayerGpuContext) {
        ctx.renderer.clear_threedtiles();
        ctx.collider.remove_features_with_prefix(&format!("{}_threedtile_", self.id));
    }

    fn on_origin_changed(&mut self, origin: &crate::gis::crs::ProjectOrigin) {
        self.manager.current_origin = Some(*origin);
        self.manager.reproject_all(origin);
    }

    fn is_streaming(&self) -> bool {
        self.manager.is_streaming()
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── Concrete Layer: GroupLayer (Hierarchical Composite) ─────────────────────

/// A composite layer containing child layers.
///
/// Toggling visibility or changing opacity on a GroupLayer cascades down to its children.
///
/// Matches Esri ArcGIS `GroupLayer`.
#[derive(Debug, Clone)]
pub struct GroupLayer {
    pub id: String,
    pub title: String,
    pub layers: crate::gis::map::LayerCollection,
    pub visible: bool,
    pub opacity: f32,
}

impl GroupLayer {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            layers: crate::gis::map::LayerCollection::new(),
            visible: true,
            opacity: 1.0,
        }
    }
}

impl Layer for GroupLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.title }
    fn layer_type(&self) -> LayerType { LayerType::Group }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) { self.opacity = opacity.clamp(0.0, 1.0); }
    fn descriptor(&self) -> LayerDescriptor {
        let mut desc = LayerDescriptor::new(&self.id, &self.title, "group");
        desc.visible = self.visible;
        desc.opacity = self.opacity;
        desc
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── Concrete Layer: FeatureLayer (Vector Features & 3D Extrusions) ──────────

/// A layer of vector features with geometries, attributes, and data-driven symbology.
///
/// Matches Esri ArcGIS `FeatureLayer`.
#[derive(Debug, Clone, Deserialize)]
pub struct FeatureLayer {
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

impl Serialize for FeatureLayer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("FeatureLayer", 20)?;
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

impl FeatureLayer {
    pub fn new(id: impl Into<String>, name: impl Into<String>, layer_type: LayerType, color_tint: [f32; 4]) -> Self {
        let color_mode = if matches!(layer_type, LayerType::ThreeDTiles { .. }) {
            LayerColorMode::OriginalTexture
        } else {
            LayerColorMode::SingleColor
        };
        Self {
            id: id.into(),
            name: name.into(),
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

    /// Reconstructs a concrete FeatureLayer from a LayerDescriptor and optional features
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

impl Layer for FeatureLayer {
    fn id(&self) -> &str { &self.id }
    fn title(&self) -> &str { &self.name }
    fn layer_type(&self) -> LayerType { self.layer_type.clone() }
    fn visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }
    fn opacity(&self) -> f32 { self.opacity }
    fn set_opacity(&mut self, opacity: f32) { self.opacity = opacity.clamp(0.0, 1.0); }
    fn color_tint(&self) -> [f32; 4] { self.color_tint }
    fn set_color_tint(&mut self, tint: [f32; 4]) { self.color_tint = tint; self.dirty = true; }
    fn cast_shadows(&self) -> bool { self.cast_shadows }
    fn set_cast_shadows(&mut self, cast: bool) { self.cast_shadows = cast; }
    fn descriptor(&self) -> LayerDescriptor { self.descriptor() }
    fn on_origin_changed(&mut self, _origin: &crate::gis::crs::ProjectOrigin) {
        self.dirty = true;
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ─── LayerDescriptor & Registry ──────────────────────────────────────────────

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

/// Factory trait for constructing layer instances from serialisable descriptors
pub trait LayerFactory: Send + Sync + 'static {
    /// The layer kind string this factory produces (e.g. "buildings", "3dtiles")
    fn kind(&self) -> &'static str;
    /// Constructs a concrete FeatureLayer from the given descriptor
    fn create(&self, descriptor: &LayerDescriptor) -> Result<FeatureLayer, String>;
}

/// Default built-in layer factory producing standard `FeatureLayer` instances
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
    fn create(&self, descriptor: &LayerDescriptor) -> Result<FeatureLayer, String> {
        Ok(FeatureLayer::from_descriptor(descriptor.clone(), Vec::new()))
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
    pub fn create_layer(&self, descriptor: &LayerDescriptor) -> Result<FeatureLayer, String> {
        if let Some(factory) = self.factories.get(&descriptor.kind) {
            factory.create(descriptor)
        } else {
            // Fallback to creating a standard layer from descriptor directly
            Ok(FeatureLayer::from_descriptor(descriptor.clone(), Vec::new()))
        }
    }

    /// Returns list of supported layer kinds
    pub fn supported_kinds(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }
}
