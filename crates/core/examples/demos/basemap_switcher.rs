//! # Basemap & 3D Terrain Switcher
//!
//! Demonstrates switching the active basemap (OSM, Esri Satellite, Streets, Topo, CAD Grid)
//! and streaming global 3D digital elevation model (DEM) terrain.
//!
//! ### Supported 3D Terrain Sources:
//! - **AWS Open Data Terrarium**:
//!   - Type: 256×256 RGB-encoded PNG raster DEM tiles
//!   - Formula: `elevation = (R * 256 + G + B / 256) - 32768`
//!   - URL: `https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png`
//! - **Esri WorldElevation3D**:
//!   - Type: 257×257 LERC-compressed Float32 raster elevation tiles
//!   - URL: `https://elevation3d.arcgis.com/arcgis/rest/services/WorldElevation3D/Terrain3D/ImageServer/tile/{z}/{row}/{col}`

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::gis::map::Basemap;
use s3d_core::gis::terrain::{AwsTerrain, EsriTerrain, Terrain};

#[derive(Default)]
pub struct BasemapSwitcherDemo {
    selected_basemap: BasemapProvider,
}

impl Demo for BasemapSwitcherDemo {
    fn id(&self) -> &'static str { "basemap_switcher" }
    fn title(&self) -> &'static str { "Basemap Switcher" }
    fn description(&self) -> &'static str {
        "Switch active basemap (OSM, Satellite, Streets, Topo, Grid) with 3D terrain DEM toggle."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(ProjectOrigin::from_geo(melbourne));
        engine.set_basemap(Basemap::osm());
        self.selected_basemap = BasemapProvider::OpenStreetMap;

        // Terrain is disabled (None) by default:
        engine.terrain = None;

        // Position camera with an oblique perspective to showcase 3D terrain relief
        engine.goto(
            glam::Vec3::ZERO,
            s3d_core::engine::map_engine::GoToOptions::immediate()
                .with_distance(3500.0)
                .with_heading(-30.0)
                .with_pitch(45.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;

        // Basemap dropdown
        ui.label("Basemap:");
        let basemap_label = match self.selected_basemap {
            BasemapProvider::OpenStreetMap => "OpenStreetMap",
            BasemapProvider::EsriStreet => "Esri Streets",
            BasemapProvider::EsriTopo => "Esri Topo",
            BasemapProvider::EsriImagery => "Esri Imagery",
            BasemapProvider::None => "Grid",
        };
        egui::ComboBox::from_id_salt("basemap_select")
            .selected_text(basemap_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(self.selected_basemap == BasemapProvider::OpenStreetMap, "OpenStreetMap").clicked() {
                    self.selected_basemap = BasemapProvider::OpenStreetMap;
                    engine.set_basemap(Basemap::osm());
                    changed = true;
                }
                if ui.selectable_label(self.selected_basemap == BasemapProvider::EsriStreet, "Esri Streets").clicked() {
                    self.selected_basemap = BasemapProvider::EsriStreet;
                    engine.set_basemap(Basemap::esri_streets());
                    changed = true;
                }
                if ui.selectable_label(self.selected_basemap == BasemapProvider::EsriTopo, "Esri Topo").clicked() {
                    self.selected_basemap = BasemapProvider::EsriTopo;
                    engine.set_basemap(Basemap::esri_topo());
                    changed = true;
                }
                if ui.selectable_label(self.selected_basemap == BasemapProvider::EsriImagery, "Esri Imagery").clicked() {
                    self.selected_basemap = BasemapProvider::EsriImagery;
                    engine.set_basemap(Basemap::esri_imagery());
                    changed = true;
                }
                if ui.selectable_label(self.selected_basemap == BasemapProvider::None, "Grid").clicked() {
                    self.selected_basemap = BasemapProvider::None;
                    engine.set_basemap(Basemap::none());
                    changed = true;
                }
            });

        ui.separator();

        // 3D Terrain DEM dropdown
        ui.label("Terrain:");
        let (terrain_label, is_none, is_aws, is_esri) = match &engine.terrain {
            None => ("Off", true, false, false),
            Some(Terrain::Aws(_)) => ("AWS Terrarium", false, true, false),
            Some(Terrain::Esri(_)) => ("Esri Terrain3D", false, false, true),
        };
        egui::ComboBox::from_id_salt("terrain_select")
            .selected_text(terrain_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(is_none, "Off").clicked() && !is_none {
                    engine.terrain = None;
                    changed = true;
                }
                if ui.selectable_label(is_aws, "AWS Terrarium").clicked() && !is_aws {
                    engine.terrain = Some(AwsTerrain::new().with_exaggeration(1.5).into());
                    changed = true;
                }
                if ui.selectable_label(is_esri, "Esri Terrain3D").clicked() && !is_esri {
                    engine.terrain = Some(EsriTerrain::new().with_exaggeration(1.5).into());
                    changed = true;
                }
            });

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
