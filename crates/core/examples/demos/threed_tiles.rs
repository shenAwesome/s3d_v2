//! # OGC 3D Tiles Streaming
//!
//! Demonstrates streaming massive city-scale 3D Tilesets (b3dm with embedded glTF)
//! with bounding-volume hierarchy traversal and screen-space error (SSE) LOD selection.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::gis::layer::IntegratedMeshLayer;

const PRESETS: &[(&str, f64, f64, &str, f32, f32)] = &[
    (
        "Cesium Discrete LOD",
        40.042546,
        -75.612094,
        "https://raw.githubusercontent.com/CesiumGS/3d-tiles-samples/main/1.0/TilesetWithDiscreteLOD/tileset.json",
        2200.0,
        70.0,
    ),
    (
        "Geelong CBD Buildings",
        -38.1499,
        144.3617,
        "https://dt-geelong.s3.ap-southeast-2.amazonaws.com/building/geelong/tileset.json",
        1800.0,
        55.0,
    ),
];

pub struct ThreeDTilesDemo {
    selected_preset: usize,
    max_sse: f32,
    height_offset: f32,
}

impl ThreeDTilesDemo {
    pub fn new() -> Self {
        Self {
            selected_preset: 0,
            max_sse: 16.0,
            height_offset: 0.0,
        }
    }
}

impl Demo for ThreeDTilesDemo {
    fn id(&self) -> &'static str { "threed_tiles" }
    fn title(&self) -> &'static str { "OGC 3D Tiles Streaming" }
    fn description(&self) -> &'static str {
        "Stream city-scale 3D Tilesets (b3dm) with HLOD traversal and screen-space error selection."
    }

    fn setup(&mut self, map: &mut MapEngine) {
        let (name, lat, lon, url, dist, pitch_deg) = PRESETS[self.selected_preset];
        let origin = GeoCoord::new(lat, lon, 0.0);
        map.set_origin(origin);
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap.is_enabled = true;
        map.basemap.provider = BasemapProvider::OpenStreetMap;

        // 1. Configure and activate OGC 3D Tileset streaming
        let mut layer = IntegratedMeshLayer::new("threedtiles_layer", name, url);
        layer.manager.maximum_screen_space_error = self.max_sse;
        layer.manager.height_offset = self.height_offset;
        map.add_layer(layer);

        // 2. Position camera overlooking the 3D tileset
        map.goto(
            glam::Vec3::new(0.0, 50.0, 0.0),
            GoToOptions::immediate()
                .with_distance(dist)
                .with_heading(-30.0)
                .with_pitch(pitch_deg),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut MapEngine) -> bool {
        let mut changed = false;

        // Preset selector
        ui.label("Preset:");
        for (i, (name, _, _, _, _, _)) in PRESETS.iter().enumerate() {
            if ui.selectable_label(self.selected_preset == i, *name).clicked() && self.selected_preset != i {
                self.selected_preset = i;
                let (_, lat, lon, url, dist, pitch_deg) = PRESETS[i];
                let origin = GeoCoord::new(lat, lon, 0.0);
                map.set_origin(origin);
                map.remove_layer("threedtiles_layer");
                let mut layer = IntegratedMeshLayer::new("threedtiles_layer", *name, url);
                layer.manager.maximum_screen_space_error = self.max_sse;
                layer.manager.height_offset = self.height_offset;
                map.add_layer(layer);
                map.goto(
                    glam::Vec3::ZERO,
                    GoToOptions::immediate()
                        .with_distance(dist)
                        .with_heading(-30.0)
                        .with_pitch(pitch_deg),
                );
                changed = true;
            }
        }

        ui.separator();

        // Maximum Screen-Space Error (LOD refinement threshold)
        ui.label("Max SSE:");
        if ui.add(egui::Slider::new(&mut self.max_sse, 4.0..=64.0).step_by(2.0)).changed() {
            if let Some(layer) = map.get_layer_mut::<IntegratedMeshLayer>("threedtiles_layer") {
                layer.manager.maximum_screen_space_error = self.max_sse;
            }
            changed = true;
        }

        ui.separator();

        // Vertical height offset slider
        ui.label("Height Offset:");
        if ui.add(egui::Slider::new(&mut self.height_offset, -50.0..=50.0).suffix("m").step_by(1.0)).changed() {
            if let Some(layer) = map.get_layer_mut::<IntegratedMeshLayer>("threedtiles_layer") {
                layer.manager.height_offset = self.height_offset;
                layer.manager.set_height_offset(self.height_offset);
            }
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _map: &mut MapEngine) {
        // No click handling needed
    }
}
