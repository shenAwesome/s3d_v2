//! # S3D Core
//!
//! Core 3D GIS and map rendering engine for Desktop and WebAssembly (WASM).
//!
//! This crate provides:
//! - High-precision Geodesy and Coordinate Reference Systems (`crs`)
//! - GIS vector geometries and GeoJSON loading (`geometry`, `feature`, `geojson_loader`)
//! - 3D building polygon triangulation and extrusion (`extrusion`)
//! - Slippy map raster tile streaming (`basemap`)
//! - 3D DEM elevation streaming (`terrain`)
//! - OGC 3D Tiles and Esri I3S scene layer decoders (`threedtiles`, `i3s`)
//! - Solar calculation and shadow analysis (`solar`)
//! - Spatial queries and raycast picking (`spatial`)
//! - WGPU 3D rendering pipeline and shaders (`renderer`)
//! - Unified map engine facade (`engine::MapEngine`, `engine::MapCommand`, `engine::MapEvent`)

pub mod compute;
pub mod engine;
pub mod gis;
pub mod renderer;
pub mod scene;
pub mod solar;
pub mod spatial;

// Convenient top-level re-exports
pub use engine::{
    GoToOptions, GoToTarget, IntoGoToOptions, MapCommand, MapEngine, MapEvent, MapView,
};
#[cfg(feature = "egui")]
pub use engine::{MapResponse, MapWidget};

pub use gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
pub use gis::layer::{
    ElevationFormat, ElevationLayer, ElevationMode, FeatureLayer, GraphicsLayer,
    GroupLayer, IntegratedMeshLayer, Layer, LayerTrait, LayerType, LoadStatus,
    SceneLayer, SimpleFeatureLayer, TileLayer,
};
pub use gis::map::{Basemap, Ground, LayerCollection, Map, MapBuilder, Table, TableCollection, ViewingMode};
pub use gis::graphic::Graphic;
pub use gis::renderer::Renderer;
pub use gis::basemap::{BasemapManager, BasemapProvider};
pub use gis::terrain::{
    AwsTerrain, AwsTerrariumTerrain, EsriTerrain, EsriTerrain3D,
    TerrariumTerrain, Terrain, TerrainManager, TerrainProvider, TerrainShadingMode,
};
pub use gis::geojson_loader::GisFeature;
pub use renderer::camera::{Camera, CameraTarget};
pub use solar::datetime_state::SolarDateTimeState;
pub use solar::sun_calc::{calculate_solar_position, SolarPosition};
pub use spatial::picking::{screen_to_ray, Ray};
