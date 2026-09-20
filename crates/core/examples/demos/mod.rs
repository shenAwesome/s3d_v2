//! Demo trait and registry for S3D Core showcase examples.
//!
//! Each demo is a self-contained file implementing the [`Demo`] trait.
//! The showcase runner uses `include_str!` on each file so the code panel
//! displays the **exact source** that is running — zero drift possible.

use egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;

// Individual demo modules — one file per demo
mod simple_map;
mod basemap_switcher;
mod geojson_buildings;
mod sun_and_shadows;
mod terrain_elevation;
mod camera_fly_to;
mod coordinate_picking;
mod i3s_layer;
mod threed_tiles;
mod cad_edges;
mod unique_value_renderer;
mod class_breaks_renderer;
mod gltf_model;

/// Trait implemented by every showcase demo.
///
/// The entire demo file (imports, struct, trait impl) is displayed verbatim
/// in the code panel via `include_str!`, so every line here IS the documentation.
pub trait Demo {
    /// Unique URL-safe identifier (e.g. `"simple_map"`).
    fn id(&self) -> &'static str;
    /// Human-readable title shown in the selector dropdown.
    fn title(&self) -> &'static str;
    /// One-line description shown below the code panel title.
    fn description(&self) -> &'static str;
    /// Called once when this demo is selected. Set up map state here.
    fn setup(&mut self, map: &mut MapEngine);
    /// Called every frame in the top bar for demo-specific controls.
    /// Return `true` if state changed and a repaint is needed.
    fn controls(&mut self, ui: &mut egui::Ui, map: &mut MapEngine) -> bool;
    /// Called every frame after `MapWidget::show()`, for click/pick handlers.
    fn on_map_response(&mut self, response: &MapResponse, map: &mut MapEngine);
}

/// A registered demo entry bundling the live demo + its source code.
pub struct DemoEntry {
    pub demo: Box<dyn Demo>,
    pub source: &'static str,
}

/// Build the ordered list of all demos with their source code.
pub fn all_demos() -> Vec<DemoEntry> {
    vec![
        DemoEntry {
            demo: Box::new(simple_map::SimpleMapDemo::new()),
            source: include_str!("simple_map.rs"),
        },
        DemoEntry {
            demo: Box::new(basemap_switcher::BasemapSwitcherDemo::new()),
            source: include_str!("basemap_switcher.rs"),
        },
        DemoEntry {
            demo: Box::new(geojson_buildings::GeoJsonBuildingsDemo::new()),
            source: include_str!("geojson_buildings.rs"),
        },
        DemoEntry {
            demo: Box::new(sun_and_shadows::SunAndShadowsDemo::new()),
            source: include_str!("sun_and_shadows.rs"),
        },
        DemoEntry {
            demo: Box::new(terrain_elevation::TerrainElevationDemo::new()),
            source: include_str!("terrain_elevation.rs"),
        },
        DemoEntry {
            demo: Box::new(camera_fly_to::CameraFlyToDemo::new()),
            source: include_str!("camera_fly_to.rs"),
        },
        DemoEntry {
            demo: Box::new(coordinate_picking::CoordinatePickingDemo::new()),
            source: include_str!("coordinate_picking.rs"),
        },
        DemoEntry {
            demo: Box::new(i3s_layer::I3SLayerDemo::new()),
            source: include_str!("i3s_layer.rs"),
        },
        DemoEntry {
            demo: Box::new(threed_tiles::ThreeDTilesDemo::new()),
            source: include_str!("threed_tiles.rs"),
        },
        DemoEntry {
            demo: Box::new(cad_edges::CadEdgesDemo::new()),
            source: include_str!("cad_edges.rs"),
        },
        DemoEntry {
            demo: Box::new(unique_value_renderer::UniqueValueRendererDemo::new()),
            source: include_str!("unique_value_renderer.rs"),
        },
        DemoEntry {
            demo: Box::new(class_breaks_renderer::ClassBreaksRendererDemo::new()),
            source: include_str!("class_breaks_renderer.rs"),
        },
        DemoEntry {
            demo: Box::new(gltf_model::GltfModelDemo::new()),
            source: include_str!("gltf_model.rs"),
        },
    ]
}

/// Find demo index by URL-safe id.
pub fn demo_index_by_id(demos: &[DemoEntry], id: &str) -> Option<usize> {
    let normalized = id.trim().to_ascii_lowercase().replace('-', "_");
    demos.iter().position(|e| e.demo.id() == normalized)
}
