//! # Basemap Switcher
//!
//! Demonstrates switching the active basemap at runtime between
//! OpenStreetMap, Esri World Streets, Esri World Topo, and Esri Imagery.

use super::Demo;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::gis::map::Basemap;

pub struct BasemapSwitcherDemo {
    selected: BasemapProvider,
}

impl BasemapSwitcherDemo {
    pub fn new() -> Self {
        Self { selected: BasemapProvider::OpenStreetMap }
    }
}

impl Demo for BasemapSwitcherDemo {
    fn id(&self) -> &'static str { "basemap_switcher" }
    fn title(&self) -> &'static str { "Basemap Switcher" }
    fn description(&self) -> &'static str {
        "Switching active basemap using Esri-style presets (OSM, Esri Satellite, Streets, Topo)."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(ProjectOrigin::from_geo(melbourne));
        engine.set_basemap(Basemap::osm());
        self.selected = BasemapProvider::OpenStreetMap;

        engine.goto([144.9631, -37.8136], 2500.0);
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;
        // Basemap selector buttons
        if ui.selectable_label(self.selected == BasemapProvider::OpenStreetMap, "OpenStreetMap").clicked() {
            self.selected = BasemapProvider::OpenStreetMap;
            engine.set_basemap(Basemap::osm());
            changed = true;
        }
        if ui.selectable_label(self.selected == BasemapProvider::EsriStreet, "Esri Streets").clicked() {
            self.selected = BasemapProvider::EsriStreet;
            engine.set_basemap(Basemap::esri_streets());
            changed = true;
        }
        if ui.selectable_label(self.selected == BasemapProvider::EsriTopo, "Esri Topo").clicked() {
            self.selected = BasemapProvider::EsriTopo;
            engine.set_basemap(Basemap::esri_topo());
            changed = true;
        }
        if ui.selectable_label(self.selected == BasemapProvider::EsriImagery, "Esri Imagery").clicked() {
            self.selected = BasemapProvider::EsriImagery;
            engine.set_basemap(Basemap::esri_imagery());
            changed = true;
        }
        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
