//! # 3D Buildings (GeoJSON Extrusion)
//!
//! Demonstrates parsing GeoJSON building footprints, 2.5D triangulation
//! via `earcutr`, height attribute extraction, and real-time 3D volumetric
//! extrusion into a `FeatureLayer`.

use super::Demo;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use s3d_core::{Basemap, GoToOptions, Map};

pub struct GeoJsonBuildingsDemo {
    inspected_building: Option<String>,
}

impl GeoJsonBuildingsDemo {
    pub fn new() -> Self {
        Self { inspected_building: None }
    }
}

impl Demo for GeoJsonBuildingsDemo {
    fn id(&self) -> &'static str { "geojson_buildings" }
    fn title(&self) -> &'static str { "3D Buildings (GeoJSON Extrusion)" }
    fn description(&self) -> &'static str {
        "Parsing GeoJSON building footprints, 2.5D triangulation, and 3D height extrusion."
    }

    fn setup(&mut self, map: &mut Map) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        map.set_origin(ProjectOrigin::from_geo(melbourne));
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap = Some(Basemap::osm());

        // Parse GeoJSON and create 3D extruded building layer
        let geojson = include_str!("../../assets/sample_buildings.geojson");
        if let Ok(dataset) = s3d_core::gis::geojson_loader::parse_geojson(
            geojson,
            Some(map.scene.origin),
        ) {
            let mut layer = s3d_core::gis::layer::FeatureLayer::new(
                "melbourne_buildings",
                "Melbourne CBD Buildings",
                s3d_core::gis::layer::LayerType::Buildings,
                [0.85, 0.88, 0.92, 1.0],
            );
            layer.features = dataset.features;
            map.add_layer(layer);
        }

        self.inspected_building = None;

        map.goto(
            [144.9631, -37.8136],
            GoToOptions::immediate()
                .with_distance(1800.0)
                .with_heading(-30.0)
                .with_tilt(50.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, _map: &mut Map) -> bool {
        // Show selected building name, if any
        if let Some(name) = &self.inspected_building {
            ui.label(
                egui::RichText::new(format!("Selected: {}", name))
                    .strong()
                    .color(egui::Color32::from_rgb(251, 191, 36)),
            );
        } else {
            ui.label(
                egui::RichText::new("Click any building to select")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        }
        false
    }

    fn on_map_response(&mut self, response: &MapResponse, map: &mut Map) {
        // Raycast click to pick building features
        if response.response.clicked() {
            if let Some(mouse_pos) = response.response.interact_pointer_pos() {
                let rect = response.response.rect;
                let ray = map.screen_to_ray(
                    mouse_pos.x - rect.min.x,
                    mouse_pos.y - rect.min.y,
                    rect.width(),
                    rect.height(),
                );
                if let Some((feat, _)) = map.pick_feature(&ray) {
                    self.inspected_building = Some(feat.name.clone());
                    map.select_feature(Some(feat));
                } else {
                    self.inspected_building = None;
                    map.select_feature(None);
                }
            }
        }
    }
}
