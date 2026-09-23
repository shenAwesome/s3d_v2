//! # Esri I3S 3D Object SceneLayer Streaming
//!
//! Demonstrates streaming OGC/Esri Indexed 3D Scene Layers (I3S/SLPK)
//! with hierarchical node index pages, binary vertex attribute buffers,
//! and per-building feature mesh segmentation in Melbourne CBD.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::gis::i3s::spec::I3S_PRESETS;
use s3d_core::gis::layer::{Layer, SceneLayer};

pub struct I3SLayerDemo {
    selected_preset: usize,
    opacity: f32,
}

impl I3SLayerDemo {
    pub fn new() -> Self {
        Self {
            selected_preset: 0,
            opacity: 1.0,
        }
    }
}

impl Demo for I3SLayerDemo {
    fn id(&self) -> &'static str { "i3s_layer" }
    fn title(&self) -> &'static str { "Esri I3S SceneLayer Streaming" }
    fn description(&self) -> &'static str {
        "Stream Indexed 3D Scene Layers (I3S) with nodepage index trees and binary vertex buffers."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let preset = &I3S_PRESETS[self.selected_preset];
        let origin = GeoCoord::new(preset.latitude, preset.longitude, 0.0);
        engine.set_origin(origin);
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.basemap.is_enabled = true;
        engine.basemap.provider = BasemapProvider::OpenStreetMap;

        // 1. Configure and activate Esri I3S SceneLayer streaming
        let scene_layer = SceneLayer::new(
            "i3s_layer",
            preset.name,
            preset.url,
        );
        engine.add_layer(scene_layer);

        // 2. Position camera overlooking 3D city scene
        engine.goto(
            glam::Vec3::new(0.0, 50.0, 0.0),
            GoToOptions::immediate()
                .with_distance(preset.camera_distance)
                .with_heading(-40.0)
                .with_pitch(50.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;

        // Preset selector buttons
        ui.label("Preset:");
        for (i, preset) in I3S_PRESETS.iter().enumerate() {
            if ui.selectable_label(self.selected_preset == i, preset.name).clicked() && self.selected_preset != i {
                self.selected_preset = i;
                engine.remove_layer("i3s_layer");
                engine.set_origin(GeoCoord::new(preset.latitude, preset.longitude, 0.0));
                engine.add_layer(SceneLayer::new("i3s_layer", preset.name, preset.url));
                engine.goto(
                    glam::Vec3::new(0.0, 50.0, 0.0),
                    GoToOptions::immediate()
                        .with_distance(preset.camera_distance)
                        .with_heading(-40.0)
                        .with_pitch(50.0),
                );
                changed = true;
            }
        }

        ui.separator();

        // Opacity slider
        ui.label("Opacity:");
        if ui.add(egui::Slider::new(&mut self.opacity, 0.1..=1.0).step_by(0.05)).changed() {
            if let Some(layer) = engine.get_layer_mut::<SceneLayer>("i3s_layer") {
                layer.set_opacity(self.opacity);
            }
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
