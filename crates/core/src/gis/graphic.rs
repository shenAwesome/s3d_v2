use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::gis::geometry::{Geometry, Point, Polygon, Polyline};
use crate::gis::symbol::Symbol3D;

/// A Graphic is the unit of geographic data in S3D.
///
/// It combines a [`Geometry`] (shape) with `attributes` (key-value data)
/// and an optional per-feature [`Symbol3D`] override.
///
/// In a **FeatureLayer**, the layer's [`Renderer`](crate::gis::renderer::Renderer)
/// assigns symbols to graphics based on their attributes. The per-graphic symbol
/// is an optional override.
///
/// In a **GraphicsLayer**, each graphic must define its own symbol since there
/// is no shared renderer.
///
/// Matches the Esri ArcGIS `Graphic` class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graphic {
    /// Unique identifier for this graphic.
    pub id: String,
    /// Optional human-readable name.
    #[serde(default)]
    pub name: String,
    /// The geometry (shape and location) of this graphic.
    pub geometry: Geometry,
    /// Key-value attribute data (properties).
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    /// Per-feature symbol override. When `None`, the layer's Renderer determines appearance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<Symbol3D>,
}

/// Backward-compatible alias matching standard GIS terminology.
pub type Feature = Graphic;

impl Graphic {
    /// Creates a minimal Graphic with just an id and geometry.
    pub fn new(id: impl Into<String>, geometry: Geometry) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            geometry,
            attributes: HashMap::new(),
            symbol: None,
        }
    }

    /// Creates a Graphic with a symbol override.
    pub fn with_symbol(id: impl Into<String>, geometry: Geometry, symbol: Symbol3D) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            geometry,
            attributes: HashMap::new(),
            symbol: Some(symbol),
        }
    }

    /// Creates a Graphic with attributes.
    pub fn with_attributes(
        id: impl Into<String>,
        geometry: Geometry,
        attributes: HashMap<String, String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            geometry,
            attributes,
            symbol: None,
        }
    }

    /// Convenience constructor for a Point graphic.
    pub fn point(id: impl Into<String>, x: f64, y: f64, z: f64) -> Self {
        Self::new(id, Geometry::Point(Point::new(x, y, z)))
    }

    /// Convenience constructor for a Polygon graphic.
    pub fn polygon(id: impl Into<String>, rings: Vec<Vec<[f64; 2]>>) -> Self {
        Self::new(id, Geometry::Polygon(Polygon::from_rings(rings)))
    }

    /// Convenience constructor for a Polyline graphic.
    pub fn polyline(id: impl Into<String>, paths: Vec<Vec<[f64; 3]>>) -> Self {
        Self::new(id, Geometry::Polyline(Polyline::new(paths)))
    }

    /// Returns the geometry type string (delegates to `Geometry::geometry_type`).
    pub fn geometry_type(&self) -> &'static str {
        self.geometry.geometry_type()
    }

    /// Looks up an attribute value by key.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|s| s.as_str())
    }

    /// Sets an attribute key-value pair.
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// Returns `true` if this graphic has a per-feature symbol override.
    pub fn has_symbol(&self) -> bool {
        self.symbol.is_some()
    }
}

