//! # Coordinate Picking & Raycast
//!
//! Demonstrates raycasting the screen cursor position into 3D world space,
//! intersecting the terrain surface, and extracting WGS84 geographic
//! coordinates (latitude, longitude) on click.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};

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

    fn setup(&mut self, map: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        map.set_origin(melbourne);
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap.is_enabled = true;
        map.basemap.provider = BasemapProvider::OpenStreetMap;

        self.picked_geo = None;

        map.goto([144.9631, -37.8136], GoToOptions::immediate().with_distance(2500.0).with_tilt(45.0));
    }

    fn controls(&mut self, ui: &mut egui::Ui, _map: &mut MapEngine) -> bool {
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
        false
    }

    fn on_map_response(&mut self, response: &MapResponse, map: &mut MapEngine) {
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
