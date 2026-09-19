pub mod arcgis;
pub mod basemap;
pub mod cache;
pub mod crs;
pub mod extrusion;
pub mod feature;
pub mod geojson_loader;
pub mod geometry;
pub mod graphic;
pub mod i3s;
pub mod layer;
pub mod platform;
pub mod rasterizer;
pub mod renderer;
pub mod source;
pub mod symbol;
pub mod terrain;
pub mod threedtiles;

pub use cache::ResourceBudget;

// Legacy feature/geometry re-exports (backward compatibility)
pub use feature::{
    Feature, Geometry, ObjectSymbol3DLayer, ObjectSymbol3DResource, Point, PointGeometry,
    PointSymbol3D, PolygonGeometry,
};

// New API re-exports
pub use geometry::{
    Extent,
    Mesh,
    Point as GeoPoint,
    Polygon as GeoPolygon,
    Polyline as GeoPolyline,
    SpatialReference,
};
pub use graphic::Graphic;
pub use renderer::{ClassBreakInfo, Renderer, UniqueValueInfo, VisualVariable};
pub use symbol::{
    Anchor3D, ExtrudeSymbol3DLayer, FillSymbol3DLayer, IconSymbol3DLayer,
    LineSymbol3DLayer, MeshMaterial3DLayer, ObjectResource,
    PathSymbol3DLayer, PolygonSymbolLayer, Symbol3D, SymbolMaterial,
};

pub use layer::{
    Layer, LayerDescriptor, LayerFactory, LayerRegistry, LayerStatus, LayerTrait,
    SimpleFeatureLayer,
};
pub use source::{
    RequestHandle, Source, SourceCaps, SourceId, SourceKind, SourceRegistry, TileKey, TileScheme,
    TileSource, XyzRasterSource,
};

