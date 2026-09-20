//! Esri ArcGIS-inspired Map document model and layer collections.
//!
//! In S3D Core, [`Map`] is the central headless GIS document model.
//! It is completely decoupled from GPU rendering, windowing, and interaction state.
//!
//! A [`Map`] contains:
//! - [`Basemap`]: Backdrop raster/vector tile layers (satellite, streets, topo).
//! - [`Ground`]: Planetary elevation surface with digital elevation models (DEM).
//! - [`LayerCollection`]: Operational spatial layers ([`FeatureLayer`], [`SceneLayer`], [`IntegratedMeshLayer`], [`GraphicsLayer`]).
//! - [`TableCollection`]: Non-spatial tabular datasets.
//!
//! Modeled after the Esri ArcGIS `Map` and `Scene` classes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::gis::basemap::BasemapProvider;
use crate::gis::crs::GeoCoord;
use crate::gis::geometry::SpatialReference;
use crate::gis::layer::{Layer, TileLayer};

// ─── LayerCollection ─────────────────────────────────────────────────────────

/// Thread-safe, observable collection of layers.
///
/// Provides O(1) indexed access, ID-based lookup, reordering, and
/// change revision tracking.
#[derive(Debug, Clone, Default)]
pub struct LayerCollection {
    layers: Arc<RwLock<Vec<Arc<dyn Layer>>>>,
    revision: Arc<AtomicU64>,
}

impl LayerCollection {
    /// Creates a new empty layer collection.
    pub fn new() -> Self {
        Self {
            layers: Arc::new(RwLock::new(Vec::new())),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Creates a layer collection initialized with the given layers.
    pub fn with_layers(layers: Vec<Arc<dyn Layer>>) -> Self {
        Self {
            layers: Arc::new(RwLock::new(layers)),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns the number of layers in this collection.
    pub fn len(&self) -> usize {
        self.layers.read().map(|l| l.len()).unwrap_or(0)
    }

    /// Returns `true` if the collection contains no layers.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the layer at the specified index, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<Arc<dyn Layer>> {
        self.layers.read().ok()?.get(index).cloned()
    }

    /// Finds a layer by its unique ID.
    pub fn find_by_id(&self, id: &str) -> Option<Arc<dyn Layer>> {
        let list = self.layers.read().ok()?;
        list.iter().find(|l| l.id() == id).cloned()
    }

    /// Appends a layer to the top of the collection.
    pub fn add(&self, layer: Arc<dyn Layer>) {
        if let Ok(mut list) = self.layers.write() {
            list.push(layer);
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Inserts a layer at the specified index.
    pub fn insert(&self, index: usize, layer: Arc<dyn Layer>) {
        if let Ok(mut list) = self.layers.write() {
            let clamped = index.min(list.len());
            list.insert(clamped, layer);
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Removes a layer by its unique ID. Returns the removed layer if found.
    pub fn remove_by_id(&self, id: &str) -> Option<Arc<dyn Layer>> {
        if let Ok(mut list) = self.layers.write() {
            if let Some(pos) = list.iter().position(|l| l.id() == id) {
                let removed = list.remove(pos);
                self.revision.fetch_add(1, Ordering::SeqCst);
                return Some(removed);
            }
        }
        None
    }

    /// Removes a layer at the given index. Returns the removed layer if found.
    pub fn remove_at(&self, index: usize) -> Option<Arc<dyn Layer>> {
        if let Ok(mut list) = self.layers.write() {
            if index < list.len() {
                let removed = list.remove(index);
                self.revision.fetch_add(1, Ordering::SeqCst);
                return Some(removed);
            }
        }
        None
    }

    /// Reorders a layer from `from_index` to `to_index`.
    pub fn reorder(&self, from_index: usize, to_index: usize) -> bool {
        if let Ok(mut list) = self.layers.write() {
            if from_index < list.len() && to_index < list.len() && from_index != to_index {
                let item = list.remove(from_index);
                list.insert(to_index, item);
                self.revision.fetch_add(1, Ordering::SeqCst);
                return true;
            }
        }
        false
    }

    /// Clears all layers from this collection.
    pub fn clear(&self) {
        if let Ok(mut list) = self.layers.write() {
            list.clear();
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Returns a snapshot of all layers in this collection as a `Vec`.
    pub fn to_vec(&self) -> Vec<Arc<dyn Layer>> {
        self.layers.read().map(|l| l.clone()).unwrap_or_default()
    }

    /// Current revision counter of this collection.
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}

// ─── Basemap ─────────────────────────────────────────────────────────────────

/// Represents the backdrop / reference layers of a Map.
///
/// In Esri ArcGIS, a Basemap is composed of:
/// - `base_layers`: Background raster or vector tile layers (satellite, streets, topo).
/// - `reference_layers`: Foreground label or boundary layers drawn on top of operational layers.
#[derive(Debug, Clone)]
pub struct Basemap {
    /// Unique identifier for this basemap.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Backdrop layers rendered underneath operational layers.
    pub base_layers: LayerCollection,
    /// Overlay layers (e.g. labels, boundaries) rendered above operational layers.
    pub reference_layers: LayerCollection,
}

impl Basemap {
    /// Creates a new custom basemap with the given ID and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            base_layers: LayerCollection::new(),
            reference_layers: LayerCollection::new(),
        }
    }

    /// Convenience builder: appends a base layer.
    pub fn with_base_layer(self, layer: Arc<dyn Layer>) -> Self {
        self.base_layers.add(layer);
        self
    }

    /// OpenStreetMap standard raster tiles preset.
    pub fn osm() -> Self {
        let base = Self::new("osm", "OpenStreetMap");
        base.base_layers.add(Arc::new(TileLayer::osm()));
        base
    }

    /// Esri World Imagery (Satellite) raster tiles preset.
    pub fn esri_imagery() -> Self {
        let base = Self::new("esri_imagery", "Esri World Imagery (Satellite)");
        base.base_layers.add(Arc::new(TileLayer::esri_imagery()));
        base
    }

    /// Esri World Streets raster tiles preset.
    pub fn esri_streets() -> Self {
        let base = Self::new("esri_streets", "Esri World Streets");
        base.base_layers.add(Arc::new(TileLayer::esri_streets()));
        base
    }

    /// Esri World Topographic raster tiles preset.
    pub fn esri_topo() -> Self {
        let base = Self::new("esri_topo", "Esri World Topo");
        base.base_layers.add(Arc::new(TileLayer::esri_topo()));
        base
    }

    /// Blank / no basemap preset (useful for CAD grids or custom background colors).
    pub fn none() -> Self {
        Self::new("none", "None (Blank Grid)")
    }

    /// Constructs a Basemap matching a legacy [`BasemapProvider`] enum value.
    pub fn from_provider(provider: BasemapProvider) -> Self {
        match provider {
            BasemapProvider::OpenStreetMap => Self::osm(),
            BasemapProvider::EsriImagery => Self::esri_imagery(),
            BasemapProvider::EsriStreet => Self::esri_streets(),
            BasemapProvider::EsriTopo => Self::esri_topo(),
            BasemapProvider::None => Self::none(),
        }
    }
}

// ─── Ground ──────────────────────────────────────────────────────────────────

/// Represents the 3D elevation surface of a Map.
///
/// Contains one or more [`ElevationLayer`]s defining digital elevation models
/// (e.g. Mapbox Terrain-RGB, Terrarium, Esri LERC) and global surface properties.
#[derive(Debug, Clone)]
pub struct Ground {
    /// Elevation layers defining surface terrain geometry.
    pub layers: LayerCollection,
    /// Solid base surface color when no elevation tiles are resident.
    pub surface_color: [f32; 4],
    /// Global vertical height exaggeration multiplier (default: 1.0).
    pub elevation_exaggeration: f32,
}

impl Default for Ground {
    fn default() -> Self {
        Self::new()
    }
}

impl Ground {
    /// Creates a new default Ground surface.
    pub fn new() -> Self {
        Self {
            layers: LayerCollection::new(),
            surface_color: [0.18, 0.20, 0.24, 1.0],
            elevation_exaggeration: 1.0,
        }
    }

    /// Convenience constructor with a single primary elevation layer.
    pub fn with_elevation_layer(layer: Arc<dyn Layer>) -> Self {
        let ground = Self::new();
        ground.layers.add(layer);
        ground
    }

    /// Adds an elevation layer to the ground surface.
    pub fn add_layer(&self, layer: Arc<dyn Layer>) {
        self.layers.add(layer);
    }

    /// Removes an elevation layer by its ID.
    pub fn remove_layer(&self, id: &str) -> Option<Arc<dyn Layer>> {
        self.layers.remove_by_id(id)
    }

    /// Queries the elevation in meters at the given geographic coordinate.
    /// Returns `None` if no elevation layer covers this coordinate or data is pending.
    pub fn query_elevation(&self, _coord: &GeoCoord) -> Option<f64> {
        // Will delegate to active ElevationLayer query sampling
        None
    }
}

// ─── Tables ──────────────────────────────────────────────────────────────────

/// Non-spatial tabular dataset (attributes without direct geometries).
#[derive(Debug, Clone)]
pub struct Table {
    pub id: String,
    pub title: String,
    pub fields: Vec<String>,
    pub rows: Vec<std::collections::HashMap<String, String>>,
}

/// Collection of non-spatial tabular datasets.
#[derive(Debug, Clone, Default)]
pub struct TableCollection {
    tables: Arc<RwLock<Vec<Table>>>,
}

impl TableCollection {
    pub fn new() -> Self {
        Self {
            tables: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn len(&self) -> usize {
        self.tables.read().map(|t| t.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn add(&self, table: Table) {
        if let Ok(mut list) = self.tables.write() {
            list.push(table);
        }
    }
}

// ─── Viewing Mode ────────────────────────────────────────────────────────────

/// Specifies the viewing mode policy for the map scene.
///
/// Modeled after Esri ArcGIS `WebScene.viewingMode`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewingMode {
    /// Whole-Earth 3D Globe (ECEF coordinates).
    Globe,
    /// Flat Local Planar (ENU coordinates).
    Planar,
    /// Automatically switches between Globe and Planar based on camera altitude above Mean Sea Level (MSL).
    Auto {
        /// Distance to Mean Sea Level (Ground 0) in meters where transition occurs.
        /// Default: 50,000.0 meters (50 km).
        threshold_altitude: f64,
    },
}

impl Default for ViewingMode {
    fn default() -> Self {
        Self::Auto {
            threshold_altitude: 50_000.0,
        }
    }
}

// ─── MapBuilder ──────────────────────────────────────────────────────────────

/// Fluent builder for constructing a [`Map`].
#[derive(Debug, Clone, Default)]
pub struct MapBuilder {
    title: Option<String>,
    spatial_reference: Option<SpatialReference>,
    basemap: Option<Basemap>,
    ground: Option<Ground>,
    origin: Option<GeoCoord>,
    viewing_mode: Option<ViewingMode>,
}

impl MapBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn spatial_reference(mut self, sr: SpatialReference) -> Self {
        self.spatial_reference = Some(sr);
        self
    }

    pub fn basemap(mut self, basemap: Basemap) -> Self {
        self.basemap = Some(basemap);
        self
    }

    pub fn ground(mut self, ground: Ground) -> Self {
        self.ground = Some(ground);
        self
    }

    /// Sets the geographic project origin using `[x, y, z]` (`[lon, lat, elev]`), `[x, y]`, or `GeoCoord`.
    pub fn origin(mut self, origin: impl Into<GeoCoord>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    pub fn viewing_mode(mut self, mode: ViewingMode) -> Self {
        self.viewing_mode = Some(mode);
        self
    }

    pub fn build(self) -> Map {
        let mut map = Map::new();
        if let Some(title) = self.title {
            map.title = title;
        }
        if let Some(sr) = self.spatial_reference {
            map.spatial_reference = sr;
        }
        if let Some(bm) = self.basemap {
            map.basemap = Some(bm);
        }
        if let Some(ground) = self.ground {
            map.ground = ground;
        }
        if let Some(origin) = self.origin {
            map.origin = origin;
        }
        if let Some(mode) = self.viewing_mode {
            map.viewing_mode = mode;
        }
        map
    }
}

// ─── Map ─────────────────────────────────────────────────────────────────────

/// The central headless GIS document model.
///
/// Encapsulates all spatial layers, backdrop basemap, 3D ground surface, and
/// coordinate reference system.
///
/// Decoupled from GPU rendering and windowing: a `Map` can be constructed,
/// loaded, analyzed, or serialized completely headless.
#[derive(Debug, Clone)]
pub struct Map {
    /// Unique identifier for this map document.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Spatial reference system for the map (default: WGS 84 / EPSG:4326).
    pub spatial_reference: SpatialReference,
    /// Backdrop basemap (imagery, street maps, topographic maps).
    pub basemap: Option<Basemap>,
    /// 3D ground surface and elevation models.
    pub ground: Ground,
    /// Operational spatial layers (vector features, 3D tiles, point clouds, markup).
    pub layers: LayerCollection,
    /// Non-spatial tabular datasets.
    pub tables: TableCollection,
    /// Geographic project origin [x: lon, y: lat, z: elev]. Default: Melbourne CBD.
    pub origin: GeoCoord,
    /// Viewing mode policy (Globe, Planar, or Auto-switch based on MSL altitude).
    pub viewing_mode: ViewingMode,
    /// Global revision counter incremented on structural changes.
    revision: Arc<AtomicU64>,
}

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

static MAP_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

impl Map {
    /// Creates a new empty GIS map.
    pub fn new() -> Self {
        Self {
            id: format!("map_{}", MAP_ID_COUNTER.fetch_add(1, Ordering::Relaxed)),
            title: "Untitled Map".to_string(),
            spatial_reference: SpatialReference::Wgs84,
            basemap: Some(Basemap::osm()),
            ground: Ground::new(),
            layers: LayerCollection::new(),
            tables: TableCollection::new(),
            origin: GeoCoord::new(-37.8136, 144.9631, 0.0),
            viewing_mode: ViewingMode::default(),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a new fluent [`MapBuilder`].
    pub fn builder() -> MapBuilder {
        MapBuilder::new()
    }

    /// Creates a new map configured with a specific basemap.
    pub fn with_basemap(basemap: Basemap) -> Self {
        let mut map = Self::new();
        map.basemap = Some(basemap);
        map
    }

    /// Sets the human-readable title of this map.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the spatial reference system of this map.
    pub fn with_spatial_reference(mut self, sr: SpatialReference) -> Self {
        self.spatial_reference = sr;
        self
    }

    /// Sets the geographic project origin using `[x, y, z]` (`[lon, lat, elev]`), `[x, y]`, or `GeoCoord`.
    pub fn with_origin(mut self, origin: impl Into<GeoCoord>) -> Self {
        self.origin = origin.into();
        self
    }

    /// Sets or replaces the geographic project origin.
    pub fn set_origin(&mut self, origin: impl Into<GeoCoord>) {
        self.origin = origin.into();
        self.bump_revision();
    }

    /// Sets the viewing mode policy for this map.
    pub fn with_viewing_mode(mut self, mode: ViewingMode) -> Self {
        self.viewing_mode = mode;
        self
    }

    /// Sets or replaces the viewing mode policy.
    pub fn set_viewing_mode(&mut self, mode: ViewingMode) {
        self.viewing_mode = mode;
        self.bump_revision();
    }

    /// Sets or replaces the active basemap.
    pub fn set_basemap(&mut self, basemap: Basemap) {
        self.basemap = Some(basemap);
        self.bump_revision();
    }

    /// Appends an operational layer to the map.
    pub fn add_layer(&self, layer: Arc<dyn Layer>) {
        self.layers.add(layer);
        self.bump_revision();
    }

    /// Removes an operational layer by its unique ID.
    pub fn remove_layer(&self, id: &str) -> Option<Arc<dyn Layer>> {
        let removed = self.layers.remove_by_id(id);
        if removed.is_some() {
            self.bump_revision();
        }
        removed
    }

    /// Finds an operational layer by its unique ID.
    pub fn find_layer(&self, id: &str) -> Option<Arc<dyn Layer>> {
        self.layers.find_by_id(id)
    }

    /// Returns a flattened list of all active layers in bottom-to-top visual stacking order:
    /// 1. `basemap.base_layers` (rendered at the bottom)
    /// 2. `ground.layers` (elevation DEMs)
    /// 3. `layers` (operational layers: features, 3D tiles, graphics)
    /// 4. `basemap.reference_layers` (overlay labels and boundaries rendered on top)
    pub fn all_layers(&self) -> Vec<Arc<dyn Layer>> {
        let mut result = Vec::new();

        if let Some(basemap) = &self.basemap {
            result.extend(basemap.base_layers.to_vec());
        }

        result.extend(self.ground.layers.to_vec());
        result.extend(self.layers.to_vec());

        if let Some(basemap) = &self.basemap {
            result.extend(basemap.reference_layers.to_vec());
        }

        result
    }

    /// Increments the map revision counter to notify views of structural updates.
    pub fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// Returns current revision counter.
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}
