//! # Solar Calculation & Shadows
//!
//! Demonstrates real-time astronomical solar position calculation (azimuth
//! and elevation) for Melbourne's coordinates and time of day, with
//! directional shadow casting on 3D buildings.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};

pub struct SunAndShadowsDemo {
    sim_hour: f32,
}

impl SunAndShadowsDemo {
    pub fn new() -> Self {
        Self { sim_hour: 14.0 }
    }
}

impl Demo for SunAndShadowsDemo {
    fn id(&self) -> &'static str { "sun_and_shadows" }
    fn title(&self) -> &'static str { "Solar Calculation & Shadows" }
    fn description(&self) -> &'static str {
        "Real-time astronomical solar calculation and directional shadow casting."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(ProjectOrigin::from_geo(melbourne));
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.basemap.is_enabled = true;
        engine.basemap.provider = BasemapProvider::OpenStreetMap;

        // Load buildings for shadow casting
        let geojson = include_str!("../../assets/sample_buildings.geojson");
        if let Ok(dataset) = s3d_core::gis::geojson_loader::parse_geojson(
            geojson,
            Some(engine.scene.origin),
        ) {
            let mut layer = s3d_core::gis::layer::FeatureLayer::new(
                "melbourne_buildings",
                "Melbourne CBD Buildings",
                s3d_core::gis::layer::LayerType::Buildings,
                [0.85, 0.88, 0.92, 1.0],
            );
            layer.features = dataset.features;
            engine.add_layer(layer);
        }

        // Set solar time and compute sun position
        self.sim_hour = 14.0;
        engine.solar_dt.hour = 14;
        engine.solar_dt.minute = 0;
        engine.update_solar_position();

        engine.goto(
            [144.9631, -37.8136],
            s3d_core::engine::map_engine::GoToOptions::immediate()
                .with_distance(1600.0)
                .with_heading(-40.0)
                .with_tilt(45.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        ui.label("Sun Hour:");
        // Slider to adjust time of day (6:00 – 18:00)
        if ui.add(
            egui::Slider::new(&mut self.sim_hour, 6.0..=18.0)
                .suffix("h")
                .step_by(1.0),
        ).changed() {
            engine.solar_dt.hour = self.sim_hour.round() as u32;
            engine.update_solar_position();
            return true;
        }
        false
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
