//! Basemap Streaming & 3D Terrain Example — S3D Core
//!
//! Demonstrates live multi-resolution raster tile streaming from web GIS services
//! (OSM, Esri Satellite) and 3D digital elevation model (DEM) terrain integration.
//!
//! Run with:
//! `cargo run --example basemap_streaming`

use eframe::egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::renderer::render_engine::RenderEngine;

struct BasemapStreamingApp {
    map: MapEngine,
    selected_provider: BasemapProvider,
}

impl BasemapStreamingApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Start centered on Mount Fuji to showcase 3D terrain elevation
        let fuji = GeoCoord::new(35.3606, 138.7274, 3776.0);
        let mut map = MapEngine::new(ProjectOrigin::from_geo(fuji));

        // Initial camera looking at Mount Fuji summit from a scenic angle
        map.camera.pitch = 30.0f32.to_radians();
        map.camera.yaw = (-45.0f32).to_radians();
        map.camera.distance = 18_000.0;
        map.camera.snap_smoothing();

        // Enable basemap & 3D terrain
        map.basemap.is_enabled = true;
        map.basemap.provider = BasemapProvider::OpenStreetMap;
        map.terrain.is_enabled = true;
        map.terrain.height_exaggeration = 1.2;

        if let Some(render_state) = &cc.wgpu_render_state {
            let renderer = RenderEngine::new(
                render_state.device.clone().into(),
                render_state.queue.clone().into(),
                render_state,
            );
            map.set_renderer(renderer);
        }

        Self {
            map,
            selected_provider: BasemapProvider::OpenStreetMap,
        }
    }

    fn jump_to(&mut self, lat: f64, lon: f64, elev: f64, dist: f32, pitch_deg: f32, yaw_deg: f32) {
        let geo = GeoCoord::new(lat, lon, elev);
        self.map.set_origin(ProjectOrigin::from_geo(geo));
        self.map.camera.target = glam::Vec3::ZERO;
        self.map.camera.pitch = pitch_deg.to_radians();
        self.map.camera.yaw = yaw_deg.to_radians();
        self.map.camera.distance = dist;
        self.map.camera.snap_smoothing();

        // Clear existing tiles to load fresh tiles for the new origin
        self.map.basemap.reset_cache();
        if let Some(r) = &mut self.map.renderer {
            r.gpu_tiles.clear();
        }
    }
}

impl eframe::App for BasemapStreamingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.map.update(dt);

        // Always request continuous repaint when streaming tiles or animating camera
        if self.map.camera.is_animating()
            || self.map.has_new_gpu_tiles
            || self.map.basemap.has_unconsumed_completed()
            || self.map.is_streaming()
        {
            ctx.request_repaint();
        }

        // --- Left Sidebar: Basemap Controls & Live Telemetry ---
        egui::SidePanel::left("basemap_panel")
            .resizable(true)
            .default_width(310.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("🗺 Basemap & 3D Terrain");
                ui.label(
                    egui::RichText::new("Live GIS Tile Streaming & DEM Elevation")
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.separator();

                // Basemap Provider Selector
                ui.collapsing("🛰 Basemap Provider", |ui| {
                    ui.checkbox(&mut self.map.basemap.is_enabled, "Enable Basemap");

                    let mut provider_changed = false;
                    for &prov in BasemapProvider::all() {
                        if ui
                            .selectable_value(&mut self.selected_provider, prov, prov.display_name())
                            .clicked()
                        {
                            provider_changed = true;
                        }
                    }

                    if provider_changed {
                        self.map.basemap.provider = self.selected_provider;
                        self.map.basemap.reset_cache();
                        if let Some(r) = &mut self.map.renderer {
                            r.gpu_tiles.clear();
                        }
                    }

                    ui.add_space(4.0);
                    ui.add(egui::Slider::new(&mut self.map.basemap.opacity, 0.0..=1.0).text("Opacity"));
                });

                ui.separator();

                // 3D Terrain Elevation Controls
                ui.collapsing("🏔 3D Digital Elevation (DEM)", |ui| {
                    ui.checkbox(&mut self.map.terrain.is_enabled, "Enable 3D Terrain Elevation");
                    ui.add(
                        egui::Slider::new(&mut self.map.terrain.height_exaggeration, 0.5..=3.0)
                            .text("Exaggeration")
                            .logarithmic(false),
                    );

                    let cached_terrain = self.map.terrain.tile_cache.len();
                    ui.monospace(format!("RAM DEM Tiles: {}", cached_terrain));
                });

                ui.separator();

                // Streaming Pipeline Metrics
                ui.collapsing("📊 Streaming Telemetry", |ui| {
                    let gpu_tiles_count = self
                        .map
                        .renderer
                        .as_ref()
                        .map(|r| r.gpu_tiles.len())
                        .unwrap_or(0);

                    ui.monospace(format!("LOD Dynamic Zoom:  {:.2}", self.map.basemap.zoom));
                    ui.monospace(format!("Active Frustum:    {} tiles", self.map.basemap.target_active_count));
                    ui.monospace(format!("GPU Textures:      {} tiles", gpu_tiles_count));
                    ui.monospace(format!("RAM Decoded Cache: {} tiles", self.map.basemap.cache_count()));

                    let in_flight = self.map.basemap.active_in_flight();
                    let pending = self.map.basemap.pending_tasks_count();
                    ui.monospace(format!("Workers In-Flight: {} tasks", in_flight));
                    ui.monospace(format!("Work Queue Depth:  {} tasks", pending));
                });

                ui.separator();

                // Scenic Jump Presets
                ui.collapsing("🚀 Scenic Mountain & Urban Presets", |ui| {
                    if ui.button("🗻 Mount Fuji, Japan (Volcano)").clicked() {
                        self.jump_to(35.3606, 138.7274, 3776.0, 18_000.0, 30.0, -45.0);
                        ctx.request_repaint();
                    }
                    if ui.button("🏞 Grand Canyon, USA (Gorge)").clicked() {
                        self.jump_to(36.0544, -112.1401, 2100.0, 15_000.0, 40.0, -60.0);
                        ctx.request_repaint();
                    }
                    if ui.button("🏔 Swiss Alps (Matterhorn)").clicked() {
                        self.jump_to(45.9763, 7.6586, 4478.0, 14_000.0, 35.0, 30.0);
                        ctx.request_repaint();
                    }
                    if ui.button("🌆 Melbourne CBD (Urban Flat)").clicked() {
                        self.jump_to(-37.8136, 144.9631, 20.0, 2_200.0, 45.0, -30.0);
                        ctx.request_repaint();
                    }
                    if ui.button("⛵ Sydney Harbour (Coastal)").clicked() {
                        self.jump_to(-33.8568, 151.2153, 10.0, 4_500.0, 40.0, 15.0);
                        ctx.request_repaint();
                    }
                });

                ui.separator();

                // Navigation Instructions
                ui.collapsing("ℹ️ Controls Help", |ui| {
                    ui.label("• Left Click + Drag: Pan across the globe");
                    ui.label("• Alt + Left Drag: Tilt / Orbit camera");
                    ui.label("• Mouse Scroll: Zoom to trigger automatic multi-LOD tile transitions");
                });
            });

        // --- Central 3D Map Viewport ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(14, 16, 22)))
            .show(ctx, |ui| {
                MapWidget::new(&mut self.map).show(ui);
            });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("S3D — Basemap & 3D Terrain Streaming Demo")
            .with_inner_size([1320.0, 840.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "S3D Basemap Streaming",
        native_options,
        Box::new(|cc| Ok(Box::new(BasemapStreamingApp::new(cc)))),
    )
}
