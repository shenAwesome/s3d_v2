//! # Coordinate Picking & Raycast
//!
//! Demonstrates raycasting the screen cursor position into 3D world space,
//! intersecting the terrain surface, and extracting WGS84 geographic
//! coordinates (latitude, longitude) on click.

use super::Demo;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::gis::terrain::AwsTerrain;
use s3d_core::{Basemap, GoToOptions, Map};

pub struct CoordinatePickingDemo {
    picked_geo: Option<GeoCoord>,
}

impl CoordinatePickingDemo {
    pub fn new() -> Self {
        Self { picked_geo: None }
    }
}

impl Demo for CoordinatePickingDemo {
    fn id(&self) -> &'static str { "coordinate_picking" }
    fn title(&self) -> &'static str { "Coordinate Picking & Raycast" }
    fn description(&self) -> &'static str {
        "Raycasting screen cursor position to geographic WGS84 coordinates on click."
    }

    fn setup(&mut self, map: &mut Map) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        map.set_origin(melbourne);
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap = Some(Basemap::osm());

        self.picked_geo = None;

        map.goto([144.9631, -37.8136], GoToOptions::immediate().with_distance(2500.0).with_tilt(45.0));
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut Map) -> bool {
        let mut changed = false;

        // Display picked coordinates, or prompt to click
        if let Some(geo) = self.picked_geo {
            ui.label(
                egui::RichText::new(format!("Picked: Lat {:.5}°, Lon {:.5}°", geo.latitude, geo.longitude))
                    .strong()
                    .color(egui::Color32::from_rgb(52, 211, 153)),
            );
        } else {
            ui.label(
                egui::RichText::new("Click map to pick coordinates")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.separator();

        let mut terrain_enabled = map.terrain.is_some();
        if ui.checkbox(&mut terrain_enabled, "3D Terrain").changed() {
            if terrain_enabled {
                map.terrain = Some(AwsTerrain::new().with_exaggeration(1.5).into());
            } else {
                map.terrain = None;
            }
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, response: &MapResponse, map: &mut Map) {
        // Convert clicked world point to WGS84 geographic coordinates
        if let Some(world_pt) = response.clicked_world_point {
            self.picked_geo = if map.projection_mode == ProjectionMode::GlobeECEF {
                Some(s3d_core::gis::crs::ecef_to_geodetic(world_pt))
            } else {
                Some(map.scene.origin.local_to_geo(world_pt))
            };
        }
    }
}
