//! # 3D Terrain Elevation (DEM)
//!
//! Demonstrates streaming global 3D digital elevation model (DEM) terrain,
//! rendering 3D mountain relief (Mount Fuji), and interactive height exaggeration adjustment.

use super::Demo;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use s3d_core::gis::terrain::{AwsTerrain, EsriTerrain, Terrain};
use s3d_core::{Basemap, GoToOptions, Map};

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

    fn setup(&mut self, map: &mut Map) {
        // Center on Mount Fuji (3,776 m)
        let fuji = GeoCoord::new(35.3606, 138.7274, 3776.0);
        map.set_origin(ProjectOrigin::from_geo(fuji));
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap = Some(Basemap::esri_imagery());

        // Configure 3D Terrain DEM with height exaggeration:
        self.exaggeration = 1.5;
        map.terrain = Some(AwsTerrain::new().with_exaggeration(self.exaggeration).into());

        map.goto(
            [138.7274, 35.3606],
            GoToOptions::immediate()
                .with_distance(18000.0)
                .with_heading(-45.0)
                .with_pitch(28.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut Map) -> bool {
        let mut changed = false;

        ui.label("DEM Provider:");
        let is_aws = matches!(map.terrain, Some(Terrain::Aws(_)));
        let is_esri = matches!(map.terrain, Some(Terrain::Esri(_)));

        if ui.selectable_label(is_aws, "AWS Terrarium").clicked() && !is_aws {
            map.terrain = Some(AwsTerrain::new().with_exaggeration(self.exaggeration).into());
            changed = true;
        }
        if ui.selectable_label(is_esri, "Esri Terrain3D").clicked() && !is_esri {
            map.terrain = Some(EsriTerrain::new().with_exaggeration(self.exaggeration).into());
            changed = true;
        }

        ui.separator();

        ui.label("Exaggeration:");
        // Height exaggeration slider (0.5× – 3.0×)
        if ui.add(
            egui::Slider::new(&mut self.exaggeration, 0.5..=3.0).step_by(0.1),
        ).changed() {
            if let Some(terrain) = &mut map.terrain {
                terrain.set_exaggeration(self.exaggeration);
            }
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _map: &mut Map) {
        // No click handling needed
    }
}
