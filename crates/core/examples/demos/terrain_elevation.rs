//! # 3D Terrain Elevation (DEM)
//!
//! Demonstrates streaming global 3D digital elevation model (DEM) terrain,
//! rendering 3D mountain relief (Mount Fuji), and interactive height exaggeration adjustment.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use s3d_core::gis::map::Basemap;
use s3d_core::gis::terrain::{AwsTerrain, EsriTerrain, Terrain};

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

    fn setup(&mut self, engine: &mut MapEngine) {
        // Center on Mount Fuji (3,776 m)
        let fuji = GeoCoord::new(35.3606, 138.7274, 3776.0);
        engine.set_origin(ProjectOrigin::from_geo(fuji));
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.set_basemap(Basemap::esri_imagery());

        // Configure 3D Terrain DEM with height exaggeration:
        self.exaggeration = 1.5;
        engine.terrain = Some(AwsTerrain::new().with_exaggeration(self.exaggeration).into());

        engine.goto(
            [138.7274, 35.3606],
            s3d_core::engine::map_engine::GoToOptions::immediate()
                .with_distance(18000.0)
                .with_heading(-45.0)
                .with_pitch(28.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;

        ui.label("DEM Provider:");
        let is_aws = matches!(engine.terrain, Some(Terrain::Aws(_)));
        let is_esri = matches!(engine.terrain, Some(Terrain::Esri(_)));

        if ui.selectable_label(is_aws, "AWS Terrarium").clicked() && !is_aws {
            engine.terrain = Some(AwsTerrain::new().with_exaggeration(self.exaggeration).into());
            changed = true;
        }
        if ui.selectable_label(is_esri, "Esri Terrain3D").clicked() && !is_esri {
            engine.terrain = Some(EsriTerrain::new().with_exaggeration(self.exaggeration).into());
            changed = true;
        }

        ui.separator();

        ui.label("Exaggeration:");
        // Height exaggeration slider (0.5× – 3.0×)
        if ui.add(
            egui::Slider::new(&mut self.exaggeration, 0.5..=3.0).step_by(0.1),
        ).changed() {
            if let Some(terrain) = &mut engine.terrain {
                terrain.set_exaggeration(self.exaggeration);
            }
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
