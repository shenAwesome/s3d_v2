//! Esri ArcGIS-inspired Map document model and layer collections.
//!
//! Re-exports the unified [`Map`] engine, [`Basemap`], and collections.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::gis::crs::GeoCoord;
use crate::gis::layer::Layer;

// Re-export primary types
pub use crate::engine::map_engine::Map;
pub use crate::gis::basemap::Basemap;

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

// ─── Ground ──────────────────────────────────────────────────────────────────

/// Represents the 3D elevation surface of a Map.
///
/// Contains one or more elevation layers defining digital elevation models
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
    pub fn query_elevation(&self, _coord: &GeoCoord) -> Option<f64> {
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
