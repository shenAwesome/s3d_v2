//! # Architectural CAD Edges & Silhouettes
//!
//! Demonstrates post-processing Sobel edge detection on depth and normal
//! G-buffers to produce crisp, blueprint-style CAD outlines on 3D geometry.

use super::Demo;
use s3d_core::engine::command::{EdgeCommand, MapCommand};
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::{Basemap, GoToOptions, Map};

pub struct CadEdgesDemo {
    edge_enabled: bool,
    edge_width: f32,
    color_idx: usize,
}

impl CadEdgesDemo {
    pub fn new() -> Self {
        Self {
            edge_enabled: true,
            edge_width: 1.5,
            color_idx: 0,
        }
    }
}

const COLOR_PRESETS: &[(&str, [f32; 4])] = &[
    ("Dark Ink", [0.15, 0.16, 0.18, 1.0]),
    ("Blueprint Blue", [0.10, 0.35, 0.85, 1.0]),
    ("Crisp White", [0.95, 0.95, 0.98, 1.0]),
    ("Amber Gold", [0.92, 0.65, 0.15, 1.0]),
];

impl Demo for CadEdgesDemo {
    fn id(&self) -> &'static str { "cad_edges" }
    fn title(&self) -> &'static str { "Architectural CAD Edges" }
    fn description(&self) -> &'static str {
        "Sobel edge detection on depth and normal buffers producing crisp CAD outlines."
    }

    fn setup(&mut self, map: &mut Map) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        map.set_origin(melbourne);
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap = Some(Basemap::osm());

        // Load 3D buildings for edge rendering
        let geojson = include_str!("../../assets/sample_buildings.geojson");
        if let Ok(dataset) = s3d_core::gis::geojson_loader::parse_geojson(
            geojson,
            Some(map.scene.origin),
        ) {
            let mut layer = s3d_core::gis::layer::FeatureLayer::new(
                "melbourne_buildings",
                "Melbourne CBD Buildings",
                s3d_core::gis::layer::LayerType::Buildings,
                [0.88, 0.90, 0.93, 1.0],
            );
            layer.edge_enabled = true;
            layer.features = dataset.features;
            map.add_layer(layer);
        }

        // Configure CAD edge detection pipeline
        let _ = map.apply(MapCommand::Edge(EdgeCommand::SetEnabled(self.edge_enabled)));
        let _ = map.apply(MapCommand::Edge(EdgeCommand::SetWidth(self.edge_width)));
        let _ = map.apply(MapCommand::Edge(EdgeCommand::SetColor(COLOR_PRESETS[self.color_idx].1)));

        map.goto(
            glam::Vec3::new(0.0, 40.0, 0.0),
            GoToOptions::immediate()
                .with_distance(900.0)
                .with_heading(-35.0)
                .with_pitch(50.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut Map) -> bool {
        let mut changed = false;

        // Toggle edge detection on/off
        if ui.checkbox(&mut self.edge_enabled, "CAD Edges").changed() {
            let _ = map.apply(MapCommand::Edge(EdgeCommand::SetEnabled(self.edge_enabled)));
            changed = true;
        }

        if self.edge_enabled {
            ui.separator();

            // Edge width slider
            ui.label("Width:");
            if ui.add(egui::Slider::new(&mut self.edge_width, 0.5..=4.0).step_by(0.25).suffix("px")).changed() {
                let _ = map.apply(MapCommand::Edge(EdgeCommand::SetWidth(self.edge_width)));
                changed = true;
            }

            ui.separator();

            // Color palette selector
            ui.label("Ink Color:");
            for (i, (name, color)) in COLOR_PRESETS.iter().enumerate() {
                if ui.selectable_label(self.color_idx == i, *name).clicked() && self.color_idx != i {
                    self.color_idx = i;
                    let _ = map.apply(MapCommand::Edge(EdgeCommand::SetColor(*color)));
                    changed = true;
                }
            }
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _map: &mut Map) {
        // No click handling needed
    }
}
