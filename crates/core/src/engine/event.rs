use glam::Vec3;
use crate::gis::crs::GeoCoord;
use crate::gis::geojson_loader::GisFeature;

/// Events emitted by the MapEngine during user interaction or data loading
#[derive(Debug, Clone)]
pub enum MapEvent {
    /// User clicked at geographic and local 3D coordinates
    CoordinatesClicked {
        geo: GeoCoord,
        local: Vec3,
    },
    /// A vector feature was selected or deselected
    FeatureSelected(Option<GisFeature>),
    /// Camera view, position, or orientation changed
    CameraMoved,
    /// Active basemap provider changed
    BasemapProviderChanged(crate::gis::basemap::BasemapProvider),
    /// 3D terrain elevation toggled
    TerrainToggled(bool),
    /// Project geographic origin changed
    OriginChanged(crate::gis::crs::ProjectOrigin),
    /// Status message updated
    StatusChanged(String),
}
