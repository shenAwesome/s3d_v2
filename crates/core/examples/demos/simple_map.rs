//! # Simple Map (Quickstart)
//!
//! Demonstrates initializing a `Map` document with an OpenStreetMap basemap,
//! centered on Melbourne, and navigating the camera via `engine.goto()`.

use super::Demo;
use s3d_core::engine::map_engine::{GoToOptions, MapEngine};
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::map::{Basemap, Map};

pub struct SimpleMapDemo;

impl SimpleMapDemo {
    pub fn new() -> Self {
        Self
    }
}

impl Demo for SimpleMapDemo {
    fn id(&self) -> &'static str { "simple_map" }
    fn title(&self) -> &'static str { "Simple Map (Quickstart)" }
    fn description(&self) -> &'static str {
        "Initializing a basic map with an OpenStreetMap basemap and navigating with engine.goto()."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        // Build and attach the Map document with an OpenStreetMap basemap
        let map = Map::builder()
            .basemap(Basemap::osm())
            .origin([144.9631, -37.8136, 0.0])
            .build();
        engine.set_map(map);
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;

        ui.label("Navigate (engine.goto):");

        if ui.button("Melbourne CBD").clicked() {
            // Target coordinates + distance shorthand
            engine.goto([144.9631, -37.8136], 2500.0);
            changed = true;
        }

        if ui.button("Zoom In").clicked() {
            // Adjust distance only; target and orientation stay the same
            engine.goto((), 900.0);
            changed = true;
        }

        if ui.button("Top-Down (2D)").clicked() {
            // Adjust tilt to 0° (nadir top-down); target and distance stay the same
            engine.goto((), GoToOptions::new().with_tilt(0.0));
            changed = true;
        }

        if ui.button("Perspective (3D)").clicked() {
            // Adjust tilt to 45° (3D oblique view); target and distance stay the same
            engine.goto((), GoToOptions::new().with_tilt(45.0));
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
