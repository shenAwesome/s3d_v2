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
use s3d_core::gis::layer::{Layer, SceneLayer};

pub struct I3SPreset {
    pub name: &'static str,
    pub url: &'static str,
    pub longitude: f64,
    pub latitude: f64,
    pub camera_distance: f32,
    pub default_color: [u8; 4],
}

pub const PRESETS: &[I3SPreset] = &[
    I3SPreset {
        name: "🏢 Melbourne CBD",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AB_Melbourne_WM/SceneServer",
        longitude: 144.9631,
        latitude: -37.8136,
        camera_distance: 1200.0,
        default_color: [240, 243, 246, 255],
    },
    I3SPreset {
        name: "🏛 Glen Eira Textured",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AB_Glen_Eira_Textured/SceneServer",
        longitude: 145.0373,
        latitude: -37.9027,
        camera_distance: 1200.0,
        default_color: [240, 240, 240, 255],
    },
    I3SPreset {
        name: "🏢 AQ East",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_East/SceneServer",
        longitude: 144.8309,
        latitude: -37.7763,
        camera_distance: 800.0,
        default_color: [220, 240, 230, 255],
    },
    I3SPreset {
        name: "🏗 Arden St",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/ArdenSt_189_203_WSL1/SceneServer",
        longitude: 144.9413,
        latitude: -37.8004,
        camera_distance: 600.0,
        default_color: [255, 180, 100, 255],
    },
    I3SPreset {
        name: "🏢 AQ North",
        url: "https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer",
        longitude: 144.8260,
        latitude: -37.7749,
        camera_distance: 800.0,
        default_color: [255, 226, 165, 255],
    },
];

pub struct I3SLayerDemo {
    selected_preset: usize,
    shadows: bool,
}

impl I3SLayerDemo {
    fn check_url_preset() -> Option<usize> {
        #[cfg(target_arch = "wasm32")]
        if let Some(window) = web_sys::window() {
            let check_str = |s: &str| -> Option<usize> {
                let lower = s.to_ascii_lowercase();
                if lower.contains("glen") || lower.contains("textured") || lower.contains("preset=1") {
                    Some(1)
                } else if lower.contains("east") || lower.contains("preset=2") {
                    Some(2)
                } else if lower.contains("arden") || lower.contains("preset=3") {
                    Some(3)
                } else if lower.contains("north") || lower.contains("preset=4") {
                    Some(4)
                } else if lower.contains("melbourne") || lower.contains("cbd") || lower.contains("preset=0") {
                    Some(0)
                } else {
                    None
                }
            };

            let from_hash = window.location().hash().ok().and_then(|h| check_str(&h));
            let from_search = window.location().search().ok().and_then(|s| check_str(&s));
            return from_hash.or(from_search);
        }
        None
    }

    pub fn new() -> Self {
        Self {
            selected_preset: Self::check_url_preset().unwrap_or(0),
            shadows: true,
        }
    }
}

impl Demo for I3SLayerDemo {
    fn id(&self) -> &'static str { "i3s_layer" }
    fn title(&self) -> &'static str { "Esri I3S SceneLayer Streaming" }
    fn description(&self) -> &'static str {
        "Stream Indexed 3D Scene Layers (I3S) with nodepage index trees and binary vertex buffers."
    }

    fn setup(&mut self, map: &mut MapEngine) {
        if let Some(p) = Self::check_url_preset() {
            self.selected_preset = p;
        }
        let preset = &PRESETS[self.selected_preset];
        let origin = GeoCoord::new(preset.latitude, preset.longitude, 0.0);
        map.set_origin(origin);
        map.projection_mode = ProjectionMode::PlanarENU;
        map.basemap.is_enabled = true;
        map.basemap.provider = BasemapProvider::OpenStreetMap;

        // 1. Configure and activate Esri I3S SceneLayer streaming
        let mut scene_layer = SceneLayer::new(
            "i3s_layer",
            preset.name,
            preset.url,
        );
        let c = preset.default_color;
        scene_layer.set_tint([c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]);
        map.add_layer(scene_layer);

        // 2. Position camera overlooking 3D city scene
        map.goto(
            glam::Vec3::new(0.0, 50.0, 0.0),
            GoToOptions::immediate()
                .with_distance(preset.camera_distance)
                .with_heading(-40.0)
                .with_pitch(50.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut MapEngine) -> bool {
        let mut changed = false;

        // Sync with URL hash / search navigation (e.g. #i3s_layer?preset=1)
        if let Some(target_p) = Self::check_url_preset() {
            if target_p != self.selected_preset {
                self.selected_preset = target_p;
                let preset = &PRESETS[target_p];
                map.remove_layer("i3s_layer");
                map.set_origin(GeoCoord::new(preset.latitude, preset.longitude, 0.0));
                let mut layer = SceneLayer::new("i3s_layer", preset.name, preset.url);
                let c = preset.default_color;
                layer.set_tint([c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]);
                map.add_layer(layer);
                map.goto(
                    glam::Vec3::new(0.0, 50.0, 0.0),
                    GoToOptions::immediate()
                        .with_distance(preset.camera_distance)
                        .with_heading(-40.0)
                        .with_pitch(50.0),
                );
                changed = true;
            }
        }

        // Preset selector buttons
        ui.label("Preset:");
        for (i, preset) in PRESETS.iter().enumerate() {
            if ui.selectable_label(self.selected_preset == i, preset.name).clicked() && self.selected_preset != i {
                self.selected_preset = i;
                map.remove_layer("i3s_layer");
                map.set_origin(GeoCoord::new(preset.latitude, preset.longitude, 0.0));
                let mut layer = SceneLayer::new("i3s_layer", preset.name, preset.url);
                let c = preset.default_color;
                layer.set_tint([c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]);
                map.add_layer(layer);
                map.goto(
                    glam::Vec3::new(0.0, 50.0, 0.0),
                    GoToOptions::immediate()
                        .with_distance(preset.camera_distance)
                        .with_heading(-40.0)
                        .with_pitch(50.0),
                );
                #[cfg(target_arch = "wasm32")]
                if let Some(window) = web_sys::window() {
                    let target = format!("#i3s_layer?preset={}", i);
                    let _ = window.location().set_hash(&target);
                }
                changed = true;
            }
        }

        ui.separator();

        // Shadows toggle
        if ui.checkbox(&mut self.shadows, "Shadows").changed() {
            if let Some(layer) = map.get_layer_mut::<SceneLayer>("i3s_layer") {
                layer.set_cast_shadows(self.shadows);
            }
            changed = true;
        }

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _map: &mut MapEngine) {
        // No click handling needed
    }
}
