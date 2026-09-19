//! S3D Core Unified Showcase Runner
//!
//! An all-in-one interactive demo showcasing all capabilities of the core map engine:
//! 1. 3D Buildings Extrusion & Solar Shadow Study
//! 2. Live Basemap & 3D Terrain Streaming
//! 3. GIS Coordinate Systems & Measurement Tools
//!
//! Run with:
//! `cargo run --example showcase`

use chrono::{TimeZone, Utc};
use eframe::egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{wgs84_to_web_mercator, GeoCoord, ProjectOrigin};
use s3d_core::renderer::render_engine::RenderEngine;
use s3d_core::solar::sun_calc::calculate_solar_position;
use s3d_core::spatial::measurement::{MeasurementEngine, MeasurementMode, MeasurementResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShowcaseTab {
    BuildingsSolar,
    BasemapTerrain,
    GisMeasurement,
}

struct ShowcaseApp {
    map: MapEngine,
    active_tab: ShowcaseTab,

    // Tab 1: Solar State
    sim_hour: f32,
    sim_month: u32,
    animating_sun: bool,
    anim_speed: f32,

    // Tab 2: Basemap State
    selected_provider: BasemapProvider,

    // Tab 3: Measurement State
    measurement: MeasurementEngine,
    measured_points_geo: Vec<GeoCoord>,
    last_inspected_geo: Option<GeoCoord>,
}

impl ShowcaseApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        let mut map = MapEngine::new(ProjectOrigin::from_geo(melbourne));

        // Initial camera
        map.camera.pitch = 38.0f32.to_radians();
        map.camera.yaw = (-25.0f32).to_radians();
        map.camera.distance = 1600.0;
        map.camera.snap_smoothing();

        // Attach renderer
        if let Some(render_state) = &cc.wgpu_render_state {
            let renderer = RenderEngine::new(
                render_state.device.clone().into(),
                render_state.queue.clone().into(),
                render_state,
            );
            map.set_renderer(renderer);
        }

        // Preload Melbourne CBD buildings
        map.load_sample_buildings();
        map.sunlight_enabled = true;
        map.sun_intensity = 1.6;
        map.ambient_intensity = 0.45;

        let mut app = Self {
            map,
            active_tab: ShowcaseTab::BuildingsSolar,
            sim_hour: 13.0,
            sim_month: 12,
            animating_sun: false,
            anim_speed: 1.0,
            selected_provider: BasemapProvider::EsriImagery,
            measurement: MeasurementEngine::new(),
            measured_points_geo: Vec::new(),
            last_inspected_geo: None,
        };

        app.update_solar_position();
        app
    }

    fn update_solar_position(&mut self) {
        let h = self.sim_hour.floor() as u32;
        let m = ((self.sim_hour - h as f32) * 60.0).floor() as u32;
        let s = (((self.sim_hour - h as f32) * 60.0 - m as f32) * 60.0).floor() as u32;

        let local_dt = Utc.with_ymd_and_hms(2025, self.sim_month, 21, h, m, s).unwrap();
        let tz_offset = if self.sim_month >= 10 || self.sim_month <= 3 { 11.0 } else { 10.0 };

        let origin_geo = self.map.scene.origin.origin;
        self.map.solar_pos = calculate_solar_position(
            origin_geo.latitude,
            origin_geo.longitude,
            &local_dt,
            tz_offset,
        );
    }

    fn switch_tab(&mut self, tab: ShowcaseTab) {
        self.active_tab = tab;
        match tab {
            ShowcaseTab::BuildingsSolar => {
                // Ensure buildings layer is visible
                for lyr in &mut self.map.layers {
                    lyr.visible = true;
                }
                self.map.basemap.is_enabled = false;
                self.map.terrain.is_enabled = false;
                self.map.sunlight_enabled = true;
                self.measurement.set_mode(None);
            }
            ShowcaseTab::BasemapTerrain => {
                self.map.basemap.is_enabled = true;
                self.map.basemap.provider = self.selected_provider;
                self.map.terrain.is_enabled = true;
                self.measurement.set_mode(None);
            }
            ShowcaseTab::GisMeasurement => {
                self.measurement.set_mode(Some(MeasurementMode::Distance));
                self.measured_points_geo.clear();
            }
        }
        self.map.reload_all_gpu_meshes();
    }
}

impl eframe::App for ShowcaseApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.map.update(dt);

        if self.animating_sun {
            self.sim_hour = (self.sim_hour + self.anim_speed * dt) % 24.0;
            self.update_solar_position();
            ctx.request_repaint();
        }

        if self.map.camera.is_animating()
            || self.map.has_new_gpu_tiles
            || self.map.basemap.has_unconsumed_completed()
            || self.map.is_streaming()
        {
            ctx.request_repaint();
        }

        // --- Top Navigation Bar ---
        egui::TopBottomPanel::top("showcase_top_bar")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(26, 29, 36))
                    .stroke(egui::Stroke::new(1.0f32, egui::Color32::from_rgb(45, 52, 64)))
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("🌐 S3D Engine Showcase");
                    ui.separator();

                    if ui.selectable_label(self.active_tab == ShowcaseTab::BuildingsSolar, "🏙 3D Buildings & Sun Study").clicked() {
                        self.switch_tab(ShowcaseTab::BuildingsSolar);
                    }
                    if ui.selectable_label(self.active_tab == ShowcaseTab::BasemapTerrain, "🗺 Live Basemap & Terrain").clicked() {
                        self.switch_tab(ShowcaseTab::BasemapTerrain);
                    }
                    if ui.selectable_label(self.active_tab == ShowcaseTab::GisMeasurement, "📏 GIS Coordinates & Measurement").clicked() {
                        self.switch_tab(ShowcaseTab::GisMeasurement);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("s3d-core v0.2.0 • Pure Rust / WGPU")
                                .small()
                                .color(egui::Color32::from_rgb(148, 163, 184)),
                        );
                    });
                });
            });

        // --- Left Sidebar: Contextual Controls ---
        egui::SidePanel::left("showcase_side_panel")
            .resizable(true)
            .default_width(330.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);

                match self.active_tab {
                    ShowcaseTab::BuildingsSolar => {
                        ui.heading("☀️ Solar Shadow Analysis");
                        ui.label(egui::RichText::new("Realistic daylight calculations & building extrusion").small().color(egui::Color32::GRAY));
                        ui.separator();

                        ui.label("Local Time of Day:");
                        let hour_label = format!("{:02}:{:02}", self.sim_hour.floor() as u32, ((self.sim_hour.fract()) * 60.0) as u32);
                        if ui.add(egui::Slider::new(&mut self.sim_hour, 0.0..=24.0).text(hour_label)).changed() {
                            self.update_solar_position();
                        }

                        ui.label("Month:");
                        let month_name = match self.sim_month {
                            1 => "Jan (Summer)", 3 => "Mar (Autumn)", 6 => "Jun (Winter)", 9 => "Sep (Spring)", 12 => "Dec (Summer)", _ => "Season",
                        };
                        if ui.add(egui::Slider::new(&mut self.sim_month, 1..=12).text(month_name)).changed() {
                            self.update_solar_position();
                        }

                        ui.horizontal(|ui| {
                            if ui.button(if self.animating_sun { "⏸ Pause" } else { "▶ Animate Sun" }).clicked() {
                                self.animating_sun = !self.animating_sun;
                            }
                            ui.add(egui::Slider::new(&mut self.anim_speed, 0.1..=5.0).text("Speed"));
                        });

                        ui.separator();

                        // Selected building inspector
                        ui.label(egui::RichText::new("Building Inspector").strong());
                        if let Some(feat) = &self.map.selected_feature {
                            ui.monospace(format!("Name:   {}", feat.name));
                            ui.monospace(format!("Height: {:.1} m", feat.height));
                            if let Some(lvls) = feat.properties.get("levels") {
                                ui.monospace(format!("Levels: {} storeys", lvls));
                            }
                            if let Some(arch) = feat.properties.get("architect") {
                                ui.monospace(format!("Design: {}", arch));
                            }
                        } else {
                            ui.label(egui::RichText::new("Click any building in the 3D map to select").italics().color(egui::Color32::GRAY));
                        }
                    }

                    ShowcaseTab::BasemapTerrain => {
                        ui.heading("🛰 Basemap Streaming");
                        ui.label(egui::RichText::new("Live multi-resolution raster tiles & 3D DEM elevation").small().color(egui::Color32::GRAY));
                        ui.separator();

                        for &prov in &[
                            BasemapProvider::EsriImagery,
                            BasemapProvider::OpenStreetMap,
                            BasemapProvider::CartoLight,
                            BasemapProvider::CartoDark,
                            BasemapProvider::EsriTopo,
                        ] {
                            if ui.selectable_value(&mut self.selected_provider, prov, prov.display_name()).clicked() {
                                self.map.basemap.provider = self.selected_provider;
                                self.map.basemap.reset_cache();
                                if let Some(r) = &mut self.map.renderer {
                                    r.gpu_tiles.clear();
                                }
                            }
                        }

                        ui.separator();
                        ui.checkbox(&mut self.map.terrain.is_enabled, "3D Terrain Elevation");
                        ui.add(egui::Slider::new(&mut self.map.terrain.height_exaggeration, 0.5..=3.0).text("Exaggeration"));

                        ui.separator();
                        ui.label(egui::RichText::new("Scenic Terrain Presets").strong());
                        if ui.button("🗻 Mount Fuji, Japan").clicked() {
                            self.map.set_origin(ProjectOrigin::from_geo(GeoCoord::new(35.3606, 138.7274, 3776.0)));
                            self.map.camera.pitch = 30.0f32.to_radians();
                            self.map.camera.distance = 18_000.0;
                            self.map.camera.snap_smoothing();
                        }
                        if ui.button("🏞 Grand Canyon, USA").clicked() {
                            self.map.set_origin(ProjectOrigin::from_geo(GeoCoord::new(36.0544, -112.1401, 2100.0)));
                            self.map.camera.pitch = 38.0f32.to_radians();
                            self.map.camera.distance = 15_000.0;
                            self.map.camera.snap_smoothing();
                        }
                        if ui.button("🌆 Melbourne CBD").clicked() {
                            self.map.set_origin(ProjectOrigin::from_geo(GeoCoord::new(-37.8136, 144.9631, 20.0)));
                            self.map.camera.pitch = 40.0f32.to_radians();
                            self.map.camera.distance = 2_200.0;
                            self.map.camera.snap_smoothing();
                        }
                    }

                    ShowcaseTab::GisMeasurement => {
                        ui.heading("📏 Measurement & CRS");
                        ui.label(egui::RichText::new("Interactive 3D distance & coordinate conversions").small().color(egui::Color32::GRAY));
                        ui.separator();

                        ui.label("Click two points on the 3D map to measure distance:");
                        if ui.button("🔄 Clear Measurement").clicked() {
                            self.measurement.clear();
                            self.measured_points_geo.clear();
                        }

                        if let Some(res) = &self.measurement.current_result {
                            if let MeasurementResult::Distance { distance_3d, horizontal_dist, vertical_diff, geodesic_dist_m, .. } = res {
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("Measurement Results:").strong().color(egui::Color32::from_rgb(52, 211, 153)));
                                ui.monospace(format!("Straight-line 3D: {:.2} m", distance_3d));
                                ui.monospace(format!("Horizontal Ground: {:.2} m", horizontal_dist));
                                ui.monospace(format!("Height Difference: {:.2} m", vertical_diff));
                                ui.monospace(format!("Geodesic (WGS84):  {:.2} m", geodesic_dist_m));
                            }
                        }

                        ui.separator();
                        ui.label(egui::RichText::new("Coordinate Inspector").strong());
                        if let Some(geo) = self.last_inspected_geo {
                            ui.monospace(format!("WGS84 Lat:  {:+.6}°", geo.latitude));
                            ui.monospace(format!("WGS84 Lon:  {:+.6}°", geo.longitude));
                            ui.monospace(format!("DMS:        {}", geo.to_dms_string()));

                            let (mx, my) = wgs84_to_web_mercator(geo.latitude, geo.longitude);
                            ui.monospace(format!("Web Mercator X: {:.1} m", mx));
                            ui.monospace(format!("Web Mercator Y: {:.1} m", my));
                        } else {
                            ui.label(egui::RichText::new("Click ground to inspect geographic coordinate").italics().color(egui::Color32::GRAY));
                        }
                    }
                }

                ui.separator();
                ui.collapsing("ℹ️ Navigation Controls", |ui| {
                    ui.label("• Left Click + Drag: Pan map");
                    ui.label("• Alt + Left Drag / Right Drag: Orbit & Pitch");
                    ui.label("• Scroll: Zoom in/out");
                });
            });

        // --- Central 3D Viewport ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(18, 20, 26)))
            .show(ctx, |ui| {
                let response = MapWidget::new(&mut self.map).show(ui);

                if let Some(pt) = response.clicked_world_point {
                    let geo = self.map.scene.origin.local_to_geo(pt);
                    self.last_inspected_geo = Some(geo);

                    if self.active_tab == ShowcaseTab::GisMeasurement {
                        self.measurement.add_point(pt, &self.map.scene.origin);
                        self.measured_points_geo.push(geo);
                    }
                }
            });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::init();

    let wgpu_options = egui_wgpu::WgpuConfiguration {
        wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(egui_wgpu::WgpuSetupCreateNew {
            instance_descriptor: wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                flags: wgpu::InstanceFlags::empty(),
                backend_options: Default::default(),
            },
            power_preference: wgpu::PowerPreference::HighPerformance,
            device_descriptor: std::sync::Arc::new(|_adapter| wgpu::DeviceDescriptor {
                label: Some("S3D Showcase Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            }),
            native_adapter_selector: None,
            trace_path: None,
        }),
        present_mode: wgpu::PresentMode::AutoVsync,
        ..Default::default()
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("S3D Core — Interactive Showcase")
            .with_inner_size([1400.0, 900.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };

    eframe::run_native(
        "S3D Showcase",
        native_options,
        Box::new(|cc| Ok(Box::new(ShowcaseApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;

    // Redirect log messages to browser console
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions {
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(egui_wgpu::WgpuSetupCreateNew {
                instance_descriptor: wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window found")
            .document()
            .expect("No document found");

        let canvas = document
            .get_element_by_id("s3d_canvas")
            .expect("Canvas element 's3d_canvas' not found")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("Element 's3d_canvas' is not a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(ShowcaseApp::new(cc)))),
            )
            .await
            .expect("failed to start s3d showcase on web");
    });
}
