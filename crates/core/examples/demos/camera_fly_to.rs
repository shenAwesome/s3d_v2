//! # Camera Navigation & Angles
//!
//! Demonstrates positioning and aiming the 3D GIS camera using target point,
//! distance (meters), pitch (tilt), and yaw (heading) with smooth transitions.

use super::Demo;
use s3d_core::engine::map_engine::{GoToOptions, MapEngine};
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};

pub struct CameraFlyToDemo;

impl CameraFlyToDemo {
    pub fn new() -> Self {
        Self
    }
}

impl Demo for CameraFlyToDemo {
    fn id(&self) -> &'static str { "camera_navigation" }
    fn title(&self) -> &'static str { "Camera Navigation (GoTo)" }
    fn description(&self) -> &'static str {
        "Positioning and animating 3D GIS camera with engine.goto() target, distance, heading, and pitch."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(ProjectOrigin::from_geo(melbourne));
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.basemap.is_enabled = true;
        engine.basemap.provider = BasemapProvider::OpenStreetMap;

        // Load buildings so camera angles have visible geometry
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

        engine.goto(
            [144.9631, -37.8136],
            GoToOptions::immediate()
                .with_distance(2200.0)
                .with_heading(0.0)
                .with_pitch(30.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;
        // Camera navigation presets via engine.goto with smooth animation
        if ui.button("Melbourne (Oblique)").clicked() {
            engine.goto(
                [144.9631, -37.8136],
                GoToOptions::new()
                    .with_distance(1800.0)
                    .with_heading(-30.0)
                    .with_pitch(35.0),
            );
            changed = true;
        }
        if ui.button("Nadir Top-Down").clicked() {
            engine.goto(
                [144.9631, -37.8136],
                GoToOptions::new()
                    .with_distance(2500.0)
                    .with_heading(0.0)
                    .with_pitch(89.0),
            );
            changed = true;
        }
        if ui.button("Isometric 45°").clicked() {
            engine.goto(
                [144.9631, -37.8136],
                GoToOptions::new()
                    .with_distance(2200.0)
                    .with_heading(45.0)
                    .with_pitch(35.26),
            );
            changed = true;
        }
        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
