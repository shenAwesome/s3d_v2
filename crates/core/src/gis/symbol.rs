use serde::{Deserialize, Serialize};

fn default_one() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

/// Material properties for symbols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolMaterial {
    /// Color as RGBA values, each from 0.0 to 1.0.
    pub color: [f32; 4],
    /// Opacity from 0.0 to 1.0.
    #[serde(default = "default_one")]
    pub opacity: f32,
}

impl SymbolMaterial {
    /// Creates a new `SymbolMaterial` with the given color and default full opacity (1.0).
    pub fn new(color: [f32; 4]) -> Self {
        Self { color, opacity: 1.0 }
    }

    /// Creates a new `SymbolMaterial` with the given color and opacity.
    pub fn with_opacity(color: [f32; 4], opacity: f32) -> Self {
        Self { color, opacity }
    }
}

/// The resource used by an `IconSymbol3DLayer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconResource {
    Circle,
    Square,
    Triangle,
    Cross,
    Diamond,
    Custom { url: String },
}

impl Default for IconResource {
    fn default() -> Self {
        Self::Circle
    }
}

/// A 2D billboard icon at a point (screen-space).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconSymbol3DLayer {
    /// Size in pixels.
    pub size: f32,
    /// The icon resource.
    pub resource: IconResource,
    /// The material of the icon.
    pub material: SymbolMaterial,
}

/// The geometry primitive used by an `ObjectSymbol3DLayer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ObjectResource {
    #[default]
    Cube,
    Sphere,
    Cylinder,
    Cone,
    Tetrahedron,
}

/// Anchor positioning for a 3D object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Anchor3D {
    #[default]
    Bottom,
    Center,
    Top,
    Origin,
}

/// A 3D volumetric object at a point (world-space, meters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSymbol3DLayer {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub resource: ObjectResource,
    pub material: SymbolMaterial,
    #[serde(default)]
    pub anchor: Anchor3D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// A flat line along a polyline (screen-space width).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSymbol3DLayer {
    /// Size/width of the line in pixels.
    pub size: f32,
    pub material: SymbolMaterial,
    #[serde(default)]
    pub cap: LineCap,
    #[serde(default)]
    pub join: LineJoin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PathProfile {
    #[default]
    Circle,
    Quad,
}

/// A 3D tube/wall along a polyline (world-space, meters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSymbol3DLayer {
    pub width: f32,
    pub height: f32,
    pub material: SymbolMaterial,
    #[serde(default)]
    pub profile: PathProfile,
}

/// A vertical extrusion of a polygon (world-space, meters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtrudeSymbol3DLayer {
    /// Extrusion height in meters.
    pub size: f32,
    pub material: SymbolMaterial,
    #[serde(default = "default_true")]
    pub cast_shadows: bool,
}

/// A flat fill for polygon surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillSymbol3DLayer {
    pub material: SymbolMaterial,
    pub outline: Option<LineSymbol3DLayer>,
}

/// Material and texture control for mesh geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMaterial3DLayer {
    pub material: SymbolMaterial,
}

/// Allowed symbol layers for Point geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PointSymbolLayer {
    Icon(IconSymbol3DLayer),
    Object(ObjectSymbol3DLayer),
}

/// Allowed symbol layers for Line geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LineSymbolLayer {
    Line(LineSymbol3DLayer),
    Path(PathSymbol3DLayer),
}

/// Allowed symbol layers for Polygon geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PolygonSymbolLayer {
    Fill(FillSymbol3DLayer),
    Extrude(ExtrudeSymbol3DLayer),
}

/// Vertical offset settings for elevating 3D symbols above their ground anchor point.
/// Matches Esri ArcGIS Maps SDK `Symbol3DVerticalOffset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol3DVerticalOffset {
    /// Vertical offset length in world meters (or screen pixels).
    pub screen_length: f32,
    /// Maximum vertical offset in world units (meters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_world_length: Option<f32>,
    /// Minimum vertical offset in world units (meters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_world_length: Option<f32>,
}

impl Symbol3DVerticalOffset {
    pub fn new(length: f32) -> Self {
        Self {
            screen_length: length,
            max_world_length: None,
            min_world_length: None,
        }
    }

    pub fn with_bounds(length: f32, min_world: f32, max_world: f32) -> Self {
        Self {
            screen_length: length,
            max_world_length: Some(max_world),
            min_world_length: Some(min_world),
        }
    }
}

/// Border styling for a 3D callout line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Callout3DBorder {
    pub color: [f32; 4],
    #[serde(default = "default_one")]
    pub size: f32,
}

/// 3D line callout connecting an elevated symbol back to its ground anchor coordinate.
/// Matches Esri ArcGIS Maps SDK `LineCallout3D`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineCallout3D {
    /// Width/thickness of the callout leader line in pixels or meters.
    pub size: f32,
    /// RGBA color of the callout line.
    pub color: [f32; 4],
    /// Optional contrasting border outline for enhanced visibility in 3D scenes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Callout3DBorder>,
}

impl LineCallout3D {
    pub fn new(color: [f32; 4], size: f32) -> Self {
        Self {
            size,
            color,
            border: None,
        }
    }

    pub fn with_border(color: [f32; 4], size: f32, border_color: [f32; 4]) -> Self {
        Self {
            size,
            color,
            border: Some(Callout3DBorder {
                color: border_color,
                size: 1.0,
            }),
        }
    }
}

/// Supported 3D callout types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Callout3D {
    Line(LineCallout3D),
}

/// The top-level 3D symbol definition, utilizing a composite pattern of layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Symbol3D {
    Point {
        symbol_layers: Vec<PointSymbolLayer>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        vertical_offset: Option<Symbol3DVerticalOffset>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        callout: Option<Callout3D>,
    },
    Line { symbol_layers: Vec<LineSymbolLayer> },
    Polygon { symbol_layers: Vec<PolygonSymbolLayer> },
    Mesh { material: MeshMaterial3DLayer },
}

impl Symbol3D {
    /// Creates a Point symbol with one `ObjectSymbol3DLayer` Cube.
    pub fn simple_point(color: [f32; 4], width: f32, height: f32, depth: f32) -> Self {
        Self::Point {
            symbol_layers: vec![PointSymbolLayer::Object(ObjectSymbol3DLayer {
                width,
                height,
                depth,
                resource: ObjectResource::Cube,
                material: SymbolMaterial::new(color),
                anchor: Anchor3D::Bottom,
            })],
            vertical_offset: None,
            callout: None,
        }
    }

    /// Creates a Point symbol with one `ObjectSymbol3DLayer` Sphere and a 3D line callout with vertical offset.
    pub fn callout_pin(
        pin_color: [f32; 4],
        pin_radius: f32,
        offset_height: f32,
        callout_color: [f32; 4],
        callout_width: f32,
    ) -> Self {
        Self::Point {
            symbol_layers: vec![PointSymbolLayer::Object(ObjectSymbol3DLayer {
                width: pin_radius * 2.0,
                height: pin_radius * 2.0,
                depth: pin_radius * 2.0,
                resource: ObjectResource::Sphere,
                material: SymbolMaterial::new(pin_color),
                anchor: Anchor3D::Center,
            })],
            vertical_offset: Some(Symbol3DVerticalOffset::new(offset_height)),
            callout: Some(Callout3D::Line(LineCallout3D::new(callout_color, callout_width))),
        }
    }

    /// Creates a Point symbol with one `IconSymbol3DLayer` Circle.
    pub fn simple_marker(color: [f32; 4], size: f32) -> Self {
        Self::Point {
            symbol_layers: vec![PointSymbolLayer::Icon(IconSymbol3DLayer {
                size,
                resource: IconResource::Circle,
                material: SymbolMaterial::new(color),
            })],
            vertical_offset: None,
            callout: None,
        }
    }

    /// Creates a Line symbol with one `LineSymbol3DLayer`.
    pub fn simple_line(color: [f32; 4], width: f32) -> Self {
        Self::Line {
            symbol_layers: vec![LineSymbolLayer::Line(LineSymbol3DLayer {
                size: width,
                material: SymbolMaterial::new(color),
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            })],
        }
    }

    /// Creates a Polygon symbol with one `FillSymbol3DLayer`.
    pub fn simple_fill(color: [f32; 4]) -> Self {
        Self::Polygon {
            symbol_layers: vec![PolygonSymbolLayer::Fill(FillSymbol3DLayer {
                material: SymbolMaterial::new(color),
                outline: None,
            })],
        }
    }

    /// Creates a Polygon symbol with one `ExtrudeSymbol3DLayer`.
    pub fn simple_extrude(color: [f32; 4], height: f32) -> Self {
        Self::Polygon {
            symbol_layers: vec![PolygonSymbolLayer::Extrude(ExtrudeSymbol3DLayer {
                size: height,
                material: SymbolMaterial::new(color),
                cast_shadows: true,
            })],
        }
    }
}

