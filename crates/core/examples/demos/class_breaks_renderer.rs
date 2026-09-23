//! # Visual Variables & Color Ramp (Skyline Heights)
//!
//! Demonstrates continuous attribute-driven color ramp styling matching the Esri
//! `VisualVariables` pattern: buildings are colored along a continuous gradient from
//! low-rise turquoise to high-rise coral-red based on building height.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::gis::geometry::{Geometry, Polygon};
use s3d_core::gis::graphic::Graphic;
use s3d_core::gis::layer::FeatureLayer;
use s3d_core::gis::renderer::Renderer;
use s3d_core::gis::symbol::Symbol3D;

pub struct ClassBreaksRendererDemo;

impl ClassBreaksRendererDemo {
    pub fn new() -> Self {
        Self
    }
}

impl Demo for ClassBreaksRendererDemo {
    fn id(&self) -> &'static str { "class_breaks_renderer" }
    fn title(&self) -> &'static str { "Visual Variables (Color Ramp)" }
    fn description(&self) -> &'static str {
        "Continuous attribute-driven color ramp gradient based on building height."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(melbourne);
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.basemap.is_enabled = true;
        engine.basemap.provider = BasemapProvider::OpenStreetMap;

        // 1. Define Simple Renderer with continuous ColorRamp VisualVariable
        let default_symbol = Symbol3D::simple_extrude([0.5, 0.5, 0.5, 1.0], 25.0);
        let renderer = Renderer::simple_with_color_ramp(
            default_symbol,
            "height",
            15.0,                    // min height (meters)
            130.0,                   // max height (meters)
            [0.12, 0.78, 0.85, 1.0], // cool turquoise for low-rise
            [0.95, 0.28, 0.15, 1.0], // vibrant coral-red for skyscrapers
        );

        // 2. Create FeatureLayer configured with the ColorRamp Renderer
        let mut layer = FeatureLayer::with_renderer("layer_skyline", "Skyline Height Gradient", renderer);

        // 3. Add Graphics of varying heights across the city grid
        let building_specs = [
            ("bldg_01", -90.0, -30.0, 30.0, 30.0, 18.0),
            ("bldg_02", -50.0, -30.0, 30.0, 30.0, 38.0),
            ("bldg_03", -10.0, -30.0, 30.0, 30.0, 62.0),
            ("bldg_04",  30.0, -30.0, 35.0, 35.0, 92.0),
            ("bldg_05",  75.0, -30.0, 40.0, 40.0, 128.0),
            ("bldg_06", -90.0,  20.0, 30.0, 30.0, 24.0),
            ("bldg_07", -50.0,  20.0, 30.0, 30.0, 48.0),
            ("bldg_08", -10.0,  20.0, 30.0, 30.0, 78.0),
            ("bldg_09",  30.0,  20.0, 35.0, 35.0, 105.0),
            ("bldg_10",  75.0,  20.0, 40.0, 40.0, 132.0),
        ];

        for (id, x, z, w, d, height) in building_specs {
            let mut g = Graphic::new(
                id,
                Geometry::Polygon(Polygon::from_ring(vec![
                    [x, z], [x + w, z], [x + w, z + d], [x, z + d], [x, z],
                ])),
            );
            g.set_attribute("height", height.to_string());
            g.symbol = Some(Symbol3D::simple_extrude([1.0, 1.0, 1.0, 1.0], height));
            layer.add_graphic(g);
        }

        engine.add_layer(layer);

        engine.goto(
            glam::Vec3::new(10.0, 45.0, 5.0),
            GoToOptions::immediate()
                .with_distance(380.0)
                .with_heading(42.0)
                .with_pitch(44.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, _engine: &mut MapEngine) -> bool {
        // Gradient ramp display
        ui.label(
            egui::RichText::new("■ 15m (Low-Rise)")
                .color(egui::Color32::from_rgb(31, 199, 217)),
        );
        ui.label("→");
        ui.label(
            egui::RichText::new("■ 130m+ (Skyscraper)")
                .color(egui::Color32::from_rgb(242, 71, 38)),
        );
        false
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
