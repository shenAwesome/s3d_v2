//! S3D Core Examples — OpenLayers Style
//!
//! Clean, minimal-UI examples demonstrating individual `s3d-core` engine features.
//! Modeled directly after OpenLayers Examples (https://openlayers.org/en/latest/examples/):
//! - Zero extraneous UI dashboards, telemetry panels, or fake toolbars.
//! - Pure Map Viewport (`MapWidget`).
//! - Dedicated Code Section showing the 100% matching, runnable Rust code for that feature.
//!
//! Run native desktop:
//! `cargo run --example showcase`
//!
//! Run browser (WASM):
//! `demo.bat`

use eframe::egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use s3d_core::renderer::render_engine::RenderEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreExample {
    SimpleMap,
    BasemapSwitcher,
    GlobeProjection,
    GeoJsonBuildings,
    SunAndShadows,
    TerrainElevation,
    CameraFlyTo,
    CoordinatePicking,
}

impl CoreExample {
    pub fn all() -> &'static [CoreExample] {
        &[
            CoreExample::SimpleMap,
            CoreExample::BasemapSwitcher,
            CoreExample::GlobeProjection,
            CoreExample::GeoJsonBuildings,
            CoreExample::SunAndShadows,
            CoreExample::TerrainElevation,
            CoreExample::CameraFlyTo,
            CoreExample::CoordinatePicking,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            CoreExample::SimpleMap => "simple_map",
            CoreExample::BasemapSwitcher => "basemap_switcher",
            CoreExample::GlobeProjection => "globe_projection",
            CoreExample::GeoJsonBuildings => "geojson_buildings",
            CoreExample::SunAndShadows => "sun_and_shadows",
            CoreExample::TerrainElevation => "terrain_elevation",
            CoreExample::CameraFlyTo => "camera_navigation",
            CoreExample::CoordinatePicking => "coordinate_picking",
        }
    }

    pub fn from_id(s: &str) -> Option<CoreExample> {
        let s = s.trim().to_ascii_lowercase().replace('-', "_");
        match s.as_str() {
            "simple_map" | "simple" | "quickstart" => Some(CoreExample::SimpleMap),
            "basemap_switcher" | "basemap" | "basemaps" => Some(CoreExample::BasemapSwitcher),
            "globe_projection" | "globe" | "projection" => Some(CoreExample::GlobeProjection),
            "geojson_buildings" | "buildings" | "geojson" => Some(CoreExample::GeoJsonBuildings),
            "sun_and_shadows" | "sun" | "solar" | "shadows" => Some(CoreExample::SunAndShadows),
            "terrain_elevation" | "terrain" | "elevation" | "dem" => Some(CoreExample::TerrainElevation),
            "camera_navigation" | "camera" | "flyto" => Some(CoreExample::CameraFlyTo),
            "coordinate_picking" | "picking" | "coordinates" => Some(CoreExample::CoordinatePicking),
            _ => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            CoreExample::SimpleMap => "Simple Map (Quickstart)",
            CoreExample::BasemapSwitcher => "Basemap Switcher",
            CoreExample::GlobeProjection => "3D Globe Projection",
            CoreExample::GeoJsonBuildings => "3D Buildings (GeoJSON Extrusion)",
            CoreExample::SunAndShadows => "Solar Calculation & Shadows",
            CoreExample::TerrainElevation => "3D Terrain Elevation (DEM)",
            CoreExample::CameraFlyTo => "Camera Navigation & Angles",
            CoreExample::CoordinatePicking => "Coordinate Picking & Raycast",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CoreExample::SimpleMap => {
                "Demonstrates the minimal setup to initialize MapEngine and stream OpenStreetMap tiles."
            }
            CoreExample::BasemapSwitcher => {
                "Demonstrates switching between live raster tile providers (OpenStreetMap, Esri Satellite, Streets, Topo)."
            }
            CoreExample::GlobeProjection => {
                "Demonstrates switching between flat planar (Local ENU) and whole-Earth 3D Globe (ECEF) projections."
            }
            CoreExample::GeoJsonBuildings => {
                "Demonstrates parsing GeoJSON building footprints, 2.5D triangulation, and 3D height extrusion."
            }
            CoreExample::SunAndShadows => {
                "Demonstrates real-time astronomical solar calculation and directional shadow casting."
            }
            CoreExample::TerrainElevation => {
                "Demonstrates streaming 3D digital elevation model (DEM) terrain with height exaggeration."
            }
            CoreExample::CameraFlyTo => {
                "Demonstrates positioning and aiming the 3D GIS camera using target, distance, pitch, and yaw."
            }
            CoreExample::CoordinatePicking => {
                "Demonstrates raycasting screen cursor position to geographic WGS84 coordinates on click."
            }
        }
    }

    pub fn code_snippet(&self) -> &'static str {
        match self {
            CoreExample::SimpleMap => {
r#"use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};

// 1. Initialize MapEngine centered on Melbourne CBD
let origin = ProjectOrigin::from_geo(GeoCoord::new(-37.8136, 144.9631, 0.0));
let mut map = MapEngine::new(origin);

// 2. OpenStreetMap standard tiles are enabled by default
map.basemap.is_enabled = true;

// 3. Render map viewport inside egui UI
MapWidget::new(&mut map).show(ui);"#
            }
            CoreExample::BasemapSwitcher => {
r#"use s3d_core::gis::basemap::BasemapProvider;

// Switch basemap provider (OpenStreetMap, EsriImagery, EsriStreet, EsriTopo)
map.basemap.provider = BasemapProvider::EsriImagery;
map.basemap.reset_cache();

if let Some(renderer) = &mut map.renderer {
    renderer.clear_basemap_tiles();
}"#
            }
            CoreExample::GlobeProjection => {
r#"// Transition to whole-Earth 3D Globe (ECEF)
map.transition_to_globe();

// Or transition down to local planar ENU at specific coordinates
map.transition_to_planar_at_geo(-37.8136, 144.9631, 2000.0);"#
            }
            CoreExample::GeoJsonBuildings => {
r#"// Ingest GeoJSON building footprints, triangulate 2.5D polygons, and extrude 3D meshes
map.load_sample_buildings();

// Or load custom GeoJSON text:
// map.load_geojson(geojson_string, "Building Layer")?;"#
            }
            CoreExample::SunAndShadows => {
r#"// 1. Set time of day (hour and minute)
map.solar_dt.hour = 14;
map.solar_dt.minute = 0;

// 2. Recalculate astronomical solar position and update directional shadows
map.update_solar_position();"#
            }
            CoreExample::TerrainElevation => {
r#"use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};

// 1. Center map on Mount Fuji summit (elevation 3,776 m)
let fuji = GeoCoord::new(35.3606, 138.7274, 3776.0);
map.set_origin(ProjectOrigin::from_geo(fuji));

// 2. Enable 3D digital elevation model (DEM) terrain streaming and set height exaggeration
map.terrain.is_enabled = true;
map.terrain.height_exaggeration = 1.5;"#
            }
            CoreExample::CameraFlyTo => {
r#"// Position camera with target offset, distance (meters), pitch (tilt), and yaw (azimuth)
map.look_at(
    glam::Vec3::ZERO,
    1800.0,
    35.0f32.to_radians(),
    (-30.0f32).to_radians(),
);"#
            }
            CoreExample::CoordinatePicking => {
r#"// 1. Raycast screen cursor position into 3D world space
let ray = map.screen_to_ray(cursor_x, cursor_y, viewport_w, viewport_h);

// 2. Test intersection with building colliders, 3D terrain DEM, or ground plane
if let Some(hit_world) = map.intersect_scene_or_terrain(&ray) {
    // 3. Convert local Cartesian coordinates to geographic WGS84
    let geo = map.scene.origin.local_to_geo(hit_world);
    println!("Picked Lat: {:.6}°, Lon: {:.6}°", geo.latitude, geo.longitude);
}"#
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    MapAndCode,
    MapOnly,
    CodeOnly,
}

pub struct ShowcaseApp {
    map: MapEngine,
    active_example: CoreExample,
    view_mode: ViewMode,
    last_copied_at: Option<f64>,

    // Minimal parameters matching the displayed code
    selected_provider: BasemapProvider,
    sim_hour: f32,
    terrain_exaggeration: f32,
    picked_geo: Option<GeoCoord>,
    inspected_building: Option<String>,
}

#[cfg(target_arch = "wasm32")]
pub fn get_example_from_url() -> Option<CoreExample> {
    let window = web_sys::window()?;
    let location = window.location();
    if let Ok(hash) = location.hash() {
        let clean = hash.trim_start_matches('#').trim();
        if !clean.is_empty() {
            if let Some(ex) = CoreExample::from_id(clean) {
                return Some(ex);
            }
        }
    }
    if let Ok(search) = location.search() {
        let clean = search.trim_start_matches('?');
        for part in clean.split('&') {
            let mut kv = part.split('=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                if k == "example" || k == "ex" {
                    if let Some(ex) = CoreExample::from_id(v) {
                        return Some(ex);
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
pub fn sync_url_with_example(example: CoreExample) {
    if let Some(window) = web_sys::window() {
        let location = window.location();
        let target_hash = format!("#{}", example.id());
        if let Ok(cur_hash) = location.hash() {
            if cur_hash != target_hash {
                let _ = location.set_hash(&target_hash);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_initial_example() -> CoreExample {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let clean = arg.trim_start_matches("--example=").trim_start_matches("--");
        if let Some(ex) = CoreExample::from_id(clean) {
            return ex;
        }
    }
    CoreExample::SimpleMap
}

#[cfg(target_arch = "wasm32")]
pub fn get_initial_example() -> CoreExample {
    get_example_from_url().unwrap_or(CoreExample::SimpleMap)
}

impl ShowcaseApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        let mut map = MapEngine::new(ProjectOrigin::from_geo(melbourne));

        // Initial camera
        map.camera.pitch = 35.0f32.to_radians();
        map.camera.yaw = (-25.0f32).to_radians();
        map.camera.distance = 1800.0;
        map.camera.snap_smoothing();

        // Attach GPU renderer
        if let Some(render_state) = &cc.wgpu_render_state {
            let renderer = RenderEngine::new(
                render_state.device.clone().into(),
                render_state.queue.clone().into(),
                render_state,
            );
            map.set_renderer(renderer);
        }

        let initial_example = get_initial_example();
        let mut app = Self {
            map,
            active_example: initial_example,
            view_mode: ViewMode::MapAndCode,
            last_copied_at: None,
            selected_provider: BasemapProvider::OpenStreetMap,
            sim_hour: 14.0,
            terrain_exaggeration: 1.5,
            picked_geo: None,
            inspected_building: None,
        };

        app.apply_example_setup(initial_example);
        app
    }

    fn apply_example_setup(&mut self, example: CoreExample) {
        self.active_example = example;
        self.picked_geo = None;
        self.inspected_building = None;

        #[cfg(target_arch = "wasm32")]
        sync_url_with_example(example);

        match example {
            CoreExample::SimpleMap => {
                let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
                self.map.set_origin(ProjectOrigin::from_geo(melbourne));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = BasemapProvider::OpenStreetMap;
                self.map.terrain.is_enabled = false;
                self.map.layers.clear();
                self.map.reload_all_gpu_meshes();
                self.map.camera.distance = 2500.0;
                self.map.camera.pitch = 30.0f32.to_radians();
                self.map.camera.yaw = 0.0;
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
                self.map.basemap.reset_cache();
                if let Some(r) = &mut self.map.renderer {
                    r.clear_basemap_tiles();
                }
            }
            CoreExample::BasemapSwitcher => {
                let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
                self.map.set_origin(ProjectOrigin::from_geo(melbourne));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = self.selected_provider;
                self.map.terrain.is_enabled = false;
                self.map.layers.clear();
                self.map.reload_all_gpu_meshes();
                self.map.camera.distance = 2000.0;
                self.map.camera.pitch = 35.0f32.to_radians();
                self.map.camera.yaw = (-25.0f32).to_radians();
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
                self.map.basemap.reset_cache();
                if let Some(r) = &mut self.map.renderer {
                    r.clear_basemap_tiles();
                }
            }
            CoreExample::GlobeProjection => {
                self.map.transition_to_globe();
            }
            CoreExample::GeoJsonBuildings => {
                let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
                self.map.set_origin(ProjectOrigin::from_geo(melbourne));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = BasemapProvider::OpenStreetMap;
                self.map.terrain.is_enabled = false;
                self.map.load_sample_buildings();
                self.map.camera.distance = 1800.0;
                self.map.camera.pitch = 40.0f32.to_radians();
                self.map.camera.yaw = (-30.0f32).to_radians();
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
            }
            CoreExample::SunAndShadows => {
                let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
                self.map.set_origin(ProjectOrigin::from_geo(melbourne));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = BasemapProvider::OpenStreetMap;
                self.map.terrain.is_enabled = false;
                self.map.load_sample_buildings();
                self.map.solar_dt.hour = self.sim_hour.round() as u32;
                self.map.solar_dt.minute = 0;
                self.map.update_solar_position();
                self.map.camera.distance = 1600.0;
                self.map.camera.pitch = 45.0f32.to_radians();
                self.map.camera.yaw = (-40.0f32).to_radians();
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
            }
            CoreExample::TerrainElevation => {
                let fuji = GeoCoord::new(35.3606, 138.7274, 3776.0);
                self.map.set_origin(ProjectOrigin::from_geo(fuji));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = BasemapProvider::EsriImagery;
                self.map.terrain.is_enabled = true;
                self.map.terrain.height_exaggeration = self.terrain_exaggeration;
                self.map.layers.clear();
                self.map.reload_all_gpu_meshes();
                self.map.camera.distance = 18000.0;
                self.map.camera.pitch = 28.0f32.to_radians();
                self.map.camera.yaw = (-45.0f32).to_radians();
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
                self.map.basemap.reset_cache();
                if let Some(r) = &mut self.map.renderer {
                    r.clear_basemap_tiles();
                }
            }
            CoreExample::CameraFlyTo => {
                let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
                self.map.set_origin(ProjectOrigin::from_geo(melbourne));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = BasemapProvider::OpenStreetMap;
                self.map.terrain.is_enabled = false;
                self.map.load_sample_buildings();
                self.map.camera.distance = 2200.0;
                self.map.camera.pitch = 30.0f32.to_radians();
                self.map.camera.yaw = 0.0;
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
            }
            CoreExample::CoordinatePicking => {
                let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
                self.map.set_origin(ProjectOrigin::from_geo(melbourne));
                self.map.projection_mode = ProjectionMode::PlanarENU;
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = BasemapProvider::OpenStreetMap;
                self.map.terrain.is_enabled = false;
                self.map.layers.clear();
                self.map.reload_all_gpu_meshes();
                self.map.camera.distance = 2500.0;
                self.map.camera.pitch = 45.0f32.to_radians();
                self.map.camera.yaw = 0.0;
                self.map.camera.target = glam::Vec3::ZERO;
                self.map.camera.snap_smoothing();
            }
        }
    }
}

impl eframe::App for ShowcaseApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.map.update(dt);

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(url_ex) = get_example_from_url() {
                if url_ex != self.active_example {
                    self.apply_example_setup(url_ex);
                }
            }
        }

        if self.map.camera.is_animating()
            || self.map.has_new_gpu_tiles
            || self.map.basemap.has_unconsumed_completed()
            || self.map.is_streaming()
        {
            ctx.request_repaint();
        }

        // --- Top Bar: Minimal Example Selector & View Mode ---
        egui::TopBottomPanel::top("example_top_bar")
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(22, 24, 30)).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("🗺 S3D Core").strong().color(egui::Color32::from_rgb(96, 165, 250)));
                    ui.separator();

                    // Example Selector Dropdown
                    egui::ComboBox::from_id_salt("core_example_selector")
                        .selected_text(self.active_example.title())
                        .width(250.0)
                        .show_ui(ui, |ui| {
                            for &ex in CoreExample::all() {
                                if ui.selectable_label(self.active_example == ex, ex.title()).clicked() {
                                    self.apply_example_setup(ex);
                                }
                            }
                        });

                    ui.separator();

                    // View Mode Toggle
                    ui.selectable_value(&mut self.view_mode, ViewMode::MapAndCode, "Map & Code");
                    ui.selectable_value(&mut self.view_mode, ViewMode::MapOnly, "Map Only");
                    ui.selectable_value(&mut self.view_mode, ViewMode::CodeOnly, "Code Only");

                    ui.separator();

                    // --- Feature-Specific Minimal Controls (Directly Derived from Code) ---
                    match self.active_example {
                        CoreExample::BasemapSwitcher => {
                            let prev_provider = self.selected_provider;
                            for &prov in BasemapProvider::all() {
                                ui.selectable_value(&mut self.selected_provider, prov, prov.display_name());
                            }
                            if self.selected_provider != prev_provider {
                                self.map.basemap.provider = self.selected_provider;
                                self.map.basemap.reset_cache();
                                if let Some(r) = &mut self.map.renderer {
                                    r.clear_basemap_tiles();
                                }
                            }
                        }
                        CoreExample::GlobeProjection => {
                            let is_globe = self.map.projection_mode == ProjectionMode::GlobeECEF;
                            if ui.selectable_label(!is_globe, "🗺 Planar (ENU)").clicked() && is_globe {
                                self.map.transition_to_planar_at_geo(-37.8136, 144.9631, 2000.0);
                            }
                            if ui.selectable_label(is_globe, "🌐 Globe (ECEF)").clicked() && !is_globe {
                                self.map.transition_to_globe();
                            }
                        }
                        CoreExample::SunAndShadows => {
                            ui.label("Sun Hour:");
                            if ui.add(egui::Slider::new(&mut self.sim_hour, 6.0..=18.0).suffix("h").step_by(1.0)).changed() {
                                self.map.solar_dt.hour = self.sim_hour.round() as u32;
                                self.map.update_solar_position();
                            }
                        }
                        CoreExample::TerrainElevation => {
                            ui.label("Exaggeration:");
                            if ui.add(egui::Slider::new(&mut self.terrain_exaggeration, 0.5..=3.0).step_by(0.1)).changed() {
                                self.map.terrain.height_exaggeration = self.terrain_exaggeration;
                            }
                        }
                        CoreExample::CameraFlyTo => {
                            if ui.button("Melbourne").clicked() {
                                self.map.look_at(glam::Vec3::ZERO, 1800.0, 35.0f32.to_radians(), (-30.0f32).to_radians());
                            }
                            if ui.button("Nadir Top-Down").clicked() {
                                self.map.look_at(glam::Vec3::ZERO, 2500.0, 89.9f32.to_radians(), 0.0);
                            }
                            if ui.button("Isometric 45°").clicked() {
                                self.map.look_at(glam::Vec3::ZERO, 2200.0, 35.26f32.to_radians(), 45.0f32.to_radians());
                            }
                        }
                        CoreExample::CoordinatePicking => {
                            if let Some(geo) = self.picked_geo {
                                ui.label(egui::RichText::new(format!("Picked: Lat {:.5}°, Lon {:.5}°", geo.latitude, geo.longitude)).strong().color(egui::Color32::from_rgb(52, 211, 153)));
                            } else {
                                ui.label(egui::RichText::new("Click map to pick coordinates").italics().color(egui::Color32::GRAY));
                            }
                        }
                        CoreExample::GeoJsonBuildings => {
                            if let Some(name) = &self.inspected_building {
                                ui.label(egui::RichText::new(format!("Selected: {}", name)).strong().color(egui::Color32::from_rgb(251, 191, 36)));
                            } else {
                                ui.label(egui::RichText::new("Click any building to select").italics().color(egui::Color32::GRAY));
                            }
                        }
                        _ => {}
                    }
                });
            });

        // --- Bottom Code Section (OpenLayers Style) ---
        if self.view_mode != ViewMode::MapOnly {
            let height = if self.view_mode == ViewMode::CodeOnly {
                ctx.screen_rect().height() - 44.0
            } else {
                240.0
            };

            egui::TopBottomPanel::bottom("code_section_panel")
                .resizable(self.view_mode == ViewMode::MapAndCode)
                .min_height(140.0)
                .max_height(500.0)
                .default_height(height)
                .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(15, 17, 23)).inner_margin(12.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.active_example.title()).strong().color(egui::Color32::WHITE));
                        ui.label(egui::RichText::new(format!("— {}", self.active_example.description())).color(egui::Color32::GRAY));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let current_time = ctx.input(|i| i.time);
                            let is_recently_copied = self
                                .last_copied_at
                                .map_or(false, |t| current_time - t < 2.0);

                            if is_recently_copied {
                                ui.label(egui::RichText::new("✓ Copied!").color(egui::Color32::from_rgb(52, 211, 153)));
                            } else if ui.button("📋 Copy Code").clicked() {
                                ctx.copy_text(self.active_example.code_snippet().to_string());
                                self.last_copied_at = Some(current_time);
                            }

                            ui.label(egui::RichText::new("Rust (s3d-core)").monospace().color(egui::Color32::from_rgb(147, 197, 253)));
                        });
                    });

                    ui.separator();

                    // Scrollable Code Snippet Container
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut snippet = self.active_example.code_snippet();
                            ui.add(
                                egui::TextEdit::multiline(&mut snippet)
                                    .font(egui::TextStyle::Monospace)
                                    .text_color(egui::Color32::from_rgb(226, 232, 240))
                                    .desired_width(f32::INFINITY)
                                    .lock_focus(true)
                                    .interactive(false),
                            );
                        });
                });
        }

        // --- Central Map Viewport ---
        if self.view_mode != ViewMode::CodeOnly {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(10, 12, 16)))
                .show(ctx, |ui| {
                    let response = MapWidget::new(&mut self.map).show(ui);

                    // Coordinate Picking Click Handler
                    if self.active_example == CoreExample::CoordinatePicking {
                        if let Some(world_pt) = response.clicked_world_point {
                            self.picked_geo = if self.map.projection_mode == ProjectionMode::GlobeECEF {
                                Some(s3d_core::gis::crs::ecef_to_geodetic(world_pt))
                            } else {
                                Some(self.map.scene.origin.local_to_geo(world_pt))
                            };
                        }
                    }

                    // GeoJSON Building Click Selection Handler
                    if self.active_example == CoreExample::GeoJsonBuildings {
                        if response.response.clicked() {
                            if let Some(mouse_pos) = response.response.interact_pointer_pos() {
                                let rect = response.response.rect;
                                let ray = self.map.screen_to_ray(
                                    mouse_pos.x - rect.min.x,
                                    mouse_pos.y - rect.min.y,
                                    rect.width(),
                                    rect.height(),
                                );
                                if let Some((feat, _)) = self.map.pick_feature(&ray) {
                                    self.inspected_building = Some(feat.name.clone());
                                    self.map.select_feature(Some(feat));
                                } else {
                                    self.inspected_building = None;
                                    self.map.select_feature(None);
                                }
                            }
                        }
                    }
                });
        }
    }
}

// ============================================================================
// Native Desktop Runner
// ============================================================================
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("S3D Core Examples — OpenLayers Style")
            .with_inner_size([1280.0, 840.0]),
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(
                egui_wgpu::WgpuSetupCreateNew {
                    instance_descriptor: wgpu::InstanceDescriptor {
                        backends: wgpu::Backends::PRIMARY,
                        flags: wgpu::InstanceFlags::empty(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "S3D Core Examples",
        options,
        Box::new(|cc| Ok(Box::new(ShowcaseApp::new(cc)))),
    )
}

// ============================================================================
// WebAssembly (Browser) Runner
// ============================================================================
#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("s3d_canvas")
            .expect("Failed to find #s3d_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("s3d_canvas was not a HtmlCanvasElement");

        let web_options = eframe::WebOptions::default();
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(ShowcaseApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe on canvas");
    });
}
