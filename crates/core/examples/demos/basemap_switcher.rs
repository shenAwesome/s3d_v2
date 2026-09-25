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
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::gis::terrain::{AwsTerrain, EsriTerrain, Terrain};
use s3d_core::{Basemap, GoToOptions, Map};

#[derive(Default)]
pub struct BasemapSwitcherDemo;

impl Demo for BasemapSwitcherDemo {
    fn id(&self) -> &'static str { "basemap_switcher" }
    fn title(&self) -> &'static str { "Basemap Switcher" }
    fn description(&self) -> &'static str {
        "Switch active basemap (OSM, Satellite, Streets, Topo, Grid) with 3D terrain DEM toggle."
    }

    fn setup(&mut self, map: &mut Map) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        map.set_origin(ProjectOrigin::from_geo(melbourne));
        map.basemap = Some(Basemap::osm());

        // Terrain is disabled (None) by default:
        map.terrain = None;

        // Position camera with an oblique perspective to showcase 3D terrain relief
        map.goto(
            glam::Vec3::ZERO,
            GoToOptions::immediate()
                .with_distance(3500.0)
                .with_heading(-30.0)
                .with_pitch(45.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, map: &mut Map) -> bool {
        let mut changed = false;

        // Basemap dropdown
        ui.label("Basemap:");
        let basemap_label = match &map.basemap {
            Some(Basemap::OpenStreetMap) => "OpenStreetMap",
            Some(Basemap::EsriStreet) => "Esri Streets",
            Some(Basemap::EsriTopo) => "Esri Topo",
            Some(Basemap::EsriImagery) => "Esri Imagery",
            Some(Basemap::Custom { .. }) => "Custom",
            None => "Grid",
        };
        egui::ComboBox::from_id_salt("basemap_select")
            .selected_text(basemap_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(matches!(&map.basemap, Some(Basemap::OpenStreetMap)), "OpenStreetMap").clicked() {
                    map.basemap = Some(Basemap::osm());
                    changed = true;
                }
                if ui.selectable_label(matches!(&map.basemap, Some(Basemap::EsriStreet)), "Esri Streets").clicked() {
                    map.basemap = Some(Basemap::esri_streets());
                    changed = true;
                }
                if ui.selectable_label(matches!(&map.basemap, Some(Basemap::EsriTopo)), "Esri Topo").clicked() {
                    map.basemap = Some(Basemap::esri_topo());
                    changed = true;
                }
                if ui.selectable_label(matches!(&map.basemap, Some(Basemap::EsriImagery)), "Esri Imagery").clicked() {
                    map.basemap = Some(Basemap::esri_imagery());
                    changed = true;
                }
                if ui.selectable_label(map.basemap.is_none(), "Grid").clicked() {
                    map.basemap = None;
                    changed = true;
                }
            });

        ui.separator();

        // 3D Terrain DEM dropdown
        ui.label("Terrain:");
        let (terrain_label, is_none, is_aws, is_esri) = match &map.terrain {
            None => ("Off", true, false, false),
            Some(Terrain::Aws(_)) => ("AWS Terrarium", false, true, false),
            Some(Terrain::Esri(_)) => ("Esri Terrain3D", false, false, true),
        };
        egui::ComboBox::from_id_salt("terrain_select")
            .selected_text(terrain_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(is_none, "Off").clicked() && !is_none {
                    map.terrain = None;
                    changed = true;
                }
                if ui.selectable_label(is_aws, "AWS Terrarium").clicked() && !is_aws {
                    map.terrain = Some(AwsTerrain::new().with_exaggeration(1.5).into());
                    changed = true;
                }
                if ui.selectable_label(is_esri, "Esri Terrain3D").clicked() && !is_esri {
                    map.terrain = Some(EsriTerrain::new().with_exaggeration(1.5).into());
                    changed = true;
                }
            });

        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _map: &mut Map) {
        // No click handling needed
    }
}
