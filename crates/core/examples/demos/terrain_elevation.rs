//! # 3D Terrain Elevation (DEM)
//!
//! Demonstrates adding a Mapbox Terrain-RGB digital elevation model to
//! the Ground surface, rendering 3D mountain relief (Mount Fuji), and
//! interactive height exaggeration adjustment.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use s3d_core::gis::map::Basemap;

pub struct TerrainElevationDemo {
    exaggeration: f32,
}

impl TerrainElevationDemo {
    pub fn new() -> Self {
        Self { exaggeration: 1.5 }
    }
}

impl Demo for TerrainElevationDemo {
    fn id(&self) -> &'static str { "terrain_elevation" }
    fn title(&self) -> &'static str { "3D Terrain Elevation (DEM)" }
    fn description(&self) -> &'static str {
        "Adding 3D digital elevation model (DEM) terrain with height exaggeration."
    }

    fn setup(&mut self, map: &mut MapEngine) {
        // Center on Mount Fuji (3,776 m)
        let fuji = GeoCoord::new(35.3606, 138.7274, 3776.0);
        map.set_origin(ProjectOrigin::from_geo(fuji));
        map.projection_mode = ProjectionMode::PlanarENU;
        map.set_basemap(Basemap::esri_imagery());

        // Add Mapbox Terrain-RGB DEM as elevation source
        self.exaggeration = 1.5;
        map.set_elevation_layer(
            std::sync::Arc::new(s3d_core::gis::ElevationLayer::mapbox_terrain_rgb(
                "fuji_dem",
                "https://api.mapbox.com/v4/mapbox.terrain-rgb/{z}/{x}/{y}.pngraw",
            )),
            self.exaggeration,
        );

        map.goto(
            [138.7274, 35.3606],
            s3d_core::engine::map_engine::GoToOptions::immediate()
                .with_distance(18000.0)
                .with_heading(-45.0)
                .with_pitch(28.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut MapEngine) -> bool {
        ui.label("Exaggeration:");
        // Height exaggeration slider (0.5× – 3.0×)
        if ui.add(
            egui::Slider::new(&mut self.exaggeration, 0.5..=3.0).step_by(0.1),
        ).changed() {
            map.terrain.height_exaggeration = self.exaggeration;
            map.map.ground.elevation_exaggeration = self.exaggeration;
            return true;
        }
        false
    }

    fn on_map_response(&mut self, _response: &MapResponse, _map: &mut MapEngine) {
        // No click handling needed
    }
}
