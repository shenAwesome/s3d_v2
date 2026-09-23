use crate::gis::basemap::BasemapManager;
use crate::gis::cache::ResourceBudget;
use crate::gis::crs::ProjectOrigin;
use crate::gis::layer::LayerRegistry;
use crate::gis::source::SourceRegistry;
use crate::gis::terrain::{Terrain, TerrainManager};
use crate::renderer::camera::Camera;
use crate::renderer::edges::EdgeConfig;
use crate::solar::datetime_state::SolarDateTimeState;
use crate::solar::sun_calc::SolarPosition;

/// Immutable read facade over MapEngine state for controls, inspectors, and tools.
///
/// Prevents borrow-checker contention by guaranteeing that UI chrome
/// can only inspect state, emitting `MapCommand`s to mutate it sequentially.
pub struct MapView<'a> {
    pub origin: &'a ProjectOrigin,
    pub camera: &'a Camera,
    pub layers: &'a [Box<dyn crate::gis::layer::Layer>],
    pub layer_registry: &'a LayerRegistry,
    pub sources: &'a SourceRegistry,
    pub budget: &'a ResourceBudget,
    pub basemap: &'a BasemapManager,
    pub terrain: Option<&'a Terrain>,
    pub terrain_mgr: &'a TerrainManager,
    pub solar_pos: &'a SolarPosition,
    pub solar_dt: &'a SolarDateTimeState,
    pub sunlight_enabled: bool,
    pub sun_intensity: f32,
    pub ambient_intensity: f32,
    pub edge_config: Option<&'a EdgeConfig>,
    pub status_message: &'a str,
}

impl<'a> MapView<'a> {
    /// Find a layer by its unique string identifier
    pub fn find_layer(&self, id: &str) -> Option<&(dyn crate::gis::layer::Layer + 'static)> {
        self.layers.iter().find(|l| l.id() == id).map(|l| &**l)
    }

    /// Returns the number of currently visible layers
    pub fn visible_layers_count(&self) -> usize {
        self.layers.iter().filter(|l| l.visible()).count()
    }

    /// Aggregates all attributions from active sources and basemaps
    pub fn attributions(&self) -> Vec<String> {
        let mut list = self.sources.attributions();
        let bm_attr = self.basemap.provider.attribution().to_string();
        if !bm_attr.is_empty() && !list.contains(&bm_attr) {
            list.push(bm_attr);
        }
        list
    }
}
