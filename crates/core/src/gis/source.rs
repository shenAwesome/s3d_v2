use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::gis::basemap::BasemapProvider;
use crate::gis::platform::http::fetch_bytes_cancellable;

/// Generational unique identifier for sources
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceId {
    pub index: u32,
    pub generation: u32,
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "src-{}:{}", self.index, self.generation)
    }
}

/// Architectural kind/protocol of the spatial data source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    XyzRaster,
    TerrariumDem,
    EsriLercDem,
    ThreeDTiles,
    I3S,
    GeoJson,
    ArcGisFeature,
    Custom,
}

/// Tiling coordinate scheme used by the source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileScheme {
    WebMercatorQuad,
    Wgs84Geographic,
    Implicit3DTiles,
    I3SIndexTree,
}

/// Standardized coordinates identifying a single spatial tile in a pyramid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileKey {
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn new(z: u32, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }
}

/// Capabilities and metadata exposed by a spatial source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCaps {
    pub attribution: String,
    pub min_zoom: u32,
    pub max_zoom: u32,
    pub tile_size: u32,
    pub scheme: TileScheme,
}

impl Default for SourceCaps {
    fn default() -> Self {
        Self {
            attribution: String::new(),
            min_zoom: 0,
            max_zoom: 19,
            tile_size: 256,
            scheme: TileScheme::WebMercatorQuad,
        }
    }
}

/// Cancellable handle for in-flight tile network requests and CPU decodes.
///
/// Dropping or calling `.cancel()` halts processing, preventing stale tile
/// responses from wasting CPU, GPU upload bandwidth, or network socket queues.
#[derive(Clone, Default)]
pub struct RequestHandle {
    cancelled: Arc<AtomicBool>,
}

impl RequestHandle {
    pub fn new() -> (Self, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(false));
        (
            Self {
                cancelled: flag.clone(),
            },
            flag,
        )
    }

    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self { cancelled: flag }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Core trait representing an origin of spatial data or geometry streams.
pub trait Source: Send + Sync + 'static {
    fn id(&self) -> SourceId;
    fn kind(&self) -> SourceKind;
    fn name(&self) -> &str;
    fn capabilities(&self) -> &SourceCaps;
}

/// Trait for sources that stream discrete LOD tiles on demand (raster, elevation DEM, 3D Tiles)
pub trait TileSource: Source {
    /// Dispatches a request for tile payload bytes, returning a cancellation handle.
    fn request_tile(
        &self,
        key: TileKey,
        on_done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send + 'static>,
    ) -> RequestHandle;
}

/// Built-in XYZ Raster imagery tile source (OpenStreetMap, Esri Imagery)
pub struct XyzRasterSource {
    id: SourceId,
    name: String,
    url_template: String,
    caps: SourceCaps,
}

impl XyzRasterSource {
    pub fn new(id: SourceId, name: impl Into<String>, url_template: impl Into<String>, attribution: impl Into<String>) -> Self {
        let mut caps = SourceCaps::default();
        caps.attribution = attribution.into();
        Self {
            id,
            name: name.into(),
            url_template: url_template.into(),
            caps,
        }
    }

    pub fn from_provider(id: SourceId, provider: BasemapProvider) -> Self {
        let name = provider.display_name().to_string();
        let attribution = provider.attribution().to_string();
        let generic_template = provider
            .tile_url(0, 0, 0)
            .map(|url| {
                url.replace("/0/0/0.", "/{z}/{x}/{y}.")
                    .replace("/0/0/0?", "/{z}/{x}/{y}?")
            })
            .unwrap_or_default();

        Self::new(id, name, generic_template, attribution)
    }

    pub fn format_url(&self, key: TileKey) -> String {
        self.url_template
            .replace("{z}", &key.z.to_string())
            .replace("{x}", &key.x.to_string())
            .replace("{y}", &key.y.to_string())
    }
}

impl Source for XyzRasterSource {
    fn id(&self) -> SourceId {
        self.id
    }

    fn kind(&self) -> SourceKind {
        SourceKind::XyzRaster
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &SourceCaps {
        &self.caps
    }
}

impl TileSource for XyzRasterSource {
    fn request_tile(
        &self,
        key: TileKey,
        on_done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send + 'static>,
    ) -> RequestHandle {
        let url = self.format_url(key);
        fetch_bytes_cancellable(&url, move |res| {
            on_done(res);
        })
    }
}

/// Central registry managing spatial data sources and tile streaming subscriptions
pub struct SourceRegistry {
    sources: HashMap<SourceId, Arc<dyn Source>>,
    tile_sources: HashMap<SourceId, Arc<dyn TileSource>>,
    next_index: u32,
    generations: HashMap<u32, u32>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            tile_sources: HashMap::new(),
            next_index: 1,
            generations: HashMap::new(),
        }
    }

    /// Allocates a new generational SourceId
    pub fn allocate_id(&mut self) -> SourceId {
        let idx = self.next_index;
        self.next_index += 1;
        let gen = self.generations.entry(idx).or_insert(1);
        SourceId {
            index: idx,
            generation: *gen,
        }
    }

    /// Registers a generic source
    pub fn register(&mut self, source: Arc<dyn Source>) -> SourceId {
        let id = source.id();
        self.sources.insert(id, source);
        id
    }

    /// Registers a tile streaming source
    pub fn register_tile_source(&mut self, source: Arc<dyn TileSource>) -> SourceId {
        let id = source.id();
        self.sources.insert(id, source.clone());
        self.tile_sources.insert(id, source);
        id
    }

    /// Look up a source by SourceId
    pub fn get(&self, id: SourceId) -> Option<Arc<dyn Source>> {
        self.sources.get(&id).cloned()
    }

    /// Look up a tile streaming source by SourceId
    pub fn get_tile_source(&self, id: SourceId) -> Option<Arc<dyn TileSource>> {
        self.tile_sources.get(&id).cloned()
    }

    /// Unregisters a source by SourceId, bumping generation to prevent stale reuse
    pub fn unregister(&mut self, id: SourceId) -> bool {
        let removed = self.sources.remove(&id).is_some();
        self.tile_sources.remove(&id);
        if removed {
            if let Some(gen) = self.generations.get_mut(&id.index) {
                *gen = gen.wrapping_add(1);
            }
        }
        removed
    }

    /// Returns all registered sources
    pub fn all(&self) -> Vec<Arc<dyn Source>> {
        self.sources.values().cloned().collect()
    }

    /// Aggregates distinct attributions from all currently registered sources
    pub fn attributions(&self) -> Vec<String> {
        let mut list: Vec<String> = self
            .sources
            .values()
            .map(|s| s.capabilities().attribution.clone())
            .filter(|a| !a.is_empty())
            .collect();
        list.sort();
        list.dedup();
        list
    }
}

