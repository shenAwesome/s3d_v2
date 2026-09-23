//! # Unique Value Zoning (Data-Driven Symbology)
//!
//! Demonstrates data-driven polygon extrusion and styling matching the Esri
//! `UniqueValueRenderer` pattern: graphics are symbolized categorically based
//! on urban zoning attributes.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::gis::geometry::{Geometry, Polygon};
use s3d_core::gis::graphic::Graphic;
use s3d_core::gis::layer::FeatureLayer;
use s3d_core::gis::renderer::{Renderer, UniqueValueInfo};
use s3d_core::gis::symbol::Symbol3D;

pub struct UniqueValueRendererDemo;

impl UniqueValueRendererDemo {
    pub fn new() -> Self {
        Self
    }
}

impl Demo for UniqueValueRendererDemo {
    fn id(&self) -> &'static str { "unique_value_renderer" }
    fn title(&self) -> &'static str { "Unique Value Zoning (Data-Driven)" }
    fn description(&self) -> &'static str {
        "Categorical polygon extrusion and styling driven by land-use zoning attributes."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(melbourne);
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.basemap.is_enabled = true;
        engine.basemap.provider = BasemapProvider::OpenStreetMap;

        // 1. Define data-driven UniqueValueRenderer
        let renderer = Renderer::UniqueValue {
            field: "zone".to_string(),
            default_symbol: Symbol3D::simple_extrude([0.5, 0.5, 0.5, 1.0], 20.0),
            unique_value_infos: vec![
                UniqueValueInfo {
                    value: "residential".to_string(),
                    symbol: Symbol3D::simple_extrude([0.96, 0.72, 0.20, 1.0], 28.0),
                    label: "Medium-Density Residential".to_string(),
                },
                UniqueValueInfo {
                    value: "commercial".to_string(),
                    symbol: Symbol3D::simple_extrude([0.18, 0.55, 0.94, 1.0], 75.0),
                    label: "Commercial Office Tower".to_string(),
                },
                UniqueValueInfo {
                    value: "civic".to_string(),
                    symbol: Symbol3D::simple_extrude([0.88, 0.24, 0.24, 1.0], 42.0),
                    label: "Public Civic Facility".to_string(),
                },
                UniqueValueInfo {
                    value: "parkland".to_string(),
                    symbol: Symbol3D::simple_fill([0.22, 0.75, 0.35, 1.0]),
                    label: "Urban Open Space".to_string(),
                },
            ],
            visual_variables: Vec::new(),
        };

        // 2. Create FeatureLayer with data-driven renderer
        let mut layer = FeatureLayer::with_renderer("layer_zoning", "Urban Precinct Zoning", renderer);

        // 3. Add Graphics with polygon geometry and zoning attributes
        // Commercial Office Tower
        let mut g_comm = Graphic::new(
            "lot_comm",
            Geometry::Polygon(Polygon::from_ring(vec![
                [-50.0, -50.0], [10.0, -50.0], [10.0, 10.0], [-50.0, 10.0], [-50.0, -50.0],
            ])),
        );
        g_comm.set_attribute("zone", "commercial");
        layer.add_graphic(g_comm);

        // Residential Blocks
        let mut g_res1 = Graphic::new(
            "lot_res1",
            Geometry::Polygon(Polygon::from_ring(vec![
                [30.0, -50.0], [80.0, -50.0], [80.0, -10.0], [30.0, -10.0], [30.0, -50.0],
            ])),
        );
        g_res1.set_attribute("zone", "residential");
        layer.add_graphic(g_res1);

        let mut g_res2 = Graphic::new(
            "lot_res2",
            Geometry::Polygon(Polygon::from_ring(vec![
                [30.0, 5.0], [80.0, 5.0], [80.0, 45.0], [30.0, 45.0], [30.0, 5.0],
            ])),
        );
        g_res2.set_attribute("zone", "residential");
        layer.add_graphic(g_res2);

        // Public Civic Facility
        let mut g_civic = Graphic::new(
            "lot_civic",
            Geometry::Polygon(Polygon::from_ring(vec![
                [-50.0, 25.0], [-5.0, 25.0], [-5.0, 65.0], [-50.0, 65.0], [-50.0, 25.0],
            ])),
        );
        g_civic.set_attribute("zone", "civic");
        layer.add_graphic(g_civic);

        // Parkland Lawn
        let mut g_park = Graphic::new(
            "lot_park",
            Geometry::Polygon(Polygon::from_ring(vec![
                [-50.0, 80.0], [80.0, 80.0], [80.0, 130.0], [-50.0, 130.0], [-50.0, 80.0],
            ])),
        );
        g_park.set_attribute("zone", "parkland");
        layer.add_graphic(g_park);

        engine.add_layer(layer);

        engine.goto(
            glam::Vec3::new(15.0, 25.0, 40.0),
            GoToOptions::immediate()
                .with_distance(360.0)
                .with_heading(35.0)
                .with_pitch(48.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, _engine: &mut MapEngine) -> bool {
        // Display zoning legend
        ui.label(egui::RichText::new("■ Commercial").color(egui::Color32::from_rgb(46, 140, 240)));
        ui.label(egui::RichText::new("■ Residential").color(egui::Color32::from_rgb(245, 184, 51)));
        ui.label(egui::RichText::new("■ Civic").color(egui::Color32::from_rgb(225, 61, 61)));
        ui.label(egui::RichText::new("■ Parkland").color(egui::Color32::from_rgb(56, 191, 89)));
        false
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
