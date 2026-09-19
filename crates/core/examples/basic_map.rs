//! Basic Map Example — S3D Core
//!
//! Demonstrates the minimal setup to embed a 3D GIS map engine into an eframe desktop app.
//! Shows camera navigation, telemetry HUD, camera presets, and lighting controls.
//!
//! Run with:
//! `cargo run --example basic_map`

use eframe::egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use s3d_core::renderer::render_engine::RenderEngine;

struct BasicMapApp {
    map: MapEngine,
    hovered_geo: Option<GeoCoord>,
    clicked_geo: Option<GeoCoord>,
}

impl BasicMapApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Center the project origin at Melbourne CBD
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        let mut map = MapEngine::new(ProjectOrigin::from_geo(melbourne));

        // Configure initial camera: tilted 45° perspective looking over Melbourne
        map.camera.pitch = 45.0f32.to_radians();
        map.camera.yaw = (-30.0f32).to_radians();
        map.camera.distance = 1800.0;
        map.camera.snap_smoothing();

        // Attach WGPU RenderEngine if native GPU context is available
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
            hovered_geo: None,
            clicked_geo: None,
        }
    }
}

impl eframe::App for BasicMapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.map.update(dt);

        if self.map.camera.is_animating() || self.map.has_new_gpu_tiles {
            ctx.request_repaint();
        }

        // --- Left Sidebar: Telemetry & Controls ---
        egui::SidePanel::left("telemetry_panel")
            .resizable(true)
            .default_width(290.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("🌐 S3D Map Engine");
                ui.label(
                    egui::RichText::new("Core 3D GIS & Camera Architecture")
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.separator();

                // 1. Projection Mode & Basemap Provider
                ui.collapsing("🗺 Map Mode & Basemap", |ui| {
                    ui.label("Projection Mode:");
                    ui.horizontal(|ui| {
                        let is_globe = self.map.projection_mode == ProjectionMode::GlobeECEF;
                        if ui.selectable_label(!is_globe, "🗺 Planar (ENU)").clicked() {
                            if is_globe {
                                let landing_geo = self.hovered_geo.unwrap_or_else(|| GeoCoord::new(-37.8136, 144.9631, 0.0));
                                self.map.transition_to_planar_at_geo(landing_geo.latitude, landing_geo.longitude, 2500.0);
                                ctx.request_repaint();
                            }
                        }
                        if ui.selectable_label(is_globe, "🌐 3D Globe (ECEF)").clicked() {
                            if !is_globe {
                                self.map.transition_to_globe();
                                ctx.request_repaint();
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.label("Basemap Style:");
                    egui::ComboBox::from_id_salt("basemap_selector")
                        .selected_text(self.map.basemap.provider.display_name())
                        .show_ui(ui, |ui| {
                            for &provider in BasemapProvider::all() {
                                if ui.selectable_value(&mut self.map.basemap.provider, provider, provider.display_name()).clicked() {
                                    self.map.basemap.reset_cache();
                                    if let Some(r) = &mut self.map.renderer {
                                        r.clear_basemap_tiles();
                                    }
                                    ctx.request_repaint();
                                }
                            }
                        });

                    ui.add_space(4.0);
                    ui.checkbox(&mut self.map.basemap.is_enabled, "Show Basemap Tiles");
                });

                ui.separator();

                // Camera Telemetry
                ui.collapsing("📷 Camera Telemetry", |ui| {
                    let is_globe = self.map.projection_mode == ProjectionMode::GlobeECEF;
                    let mode_str = if is_globe { "🌐 3D Globe (ECEF)" } else { "🗺 Planar (Local ENU)" };
                    ui.monospace(format!("Mode:        {}", mode_str));

                    let (lat, lon, alt_str) = if is_globe {
                        let eye = self.map.camera.eye_position();
                        let geo = s3d_core::gis::crs::ecef_to_geodetic(eye);
                        const WGS84_RADIUS: f32 = 6_378_137.0;
                        let alt_km = (self.map.camera.distance - WGS84_RADIUS) / 1000.0;
                        (geo.latitude, geo.longitude, format!("{:.1} km (Orbit)", alt_km))
                    } else {
                        let geo = self.map.scene.origin.local_to_geo(self.map.camera.target);
                        (geo.latitude, geo.longitude, format!("{:.1} m", self.map.camera.distance))
                    };

                    ui.monospace(format!("Target Lat:  {:+.5}°", lat));
                    ui.monospace(format!("Target Lon:  {:+.5}°", lon));
                    ui.monospace(format!("Altitude:    {}", alt_str));
                    ui.monospace(format!("Pitch (Tilt):{:.1}°", self.map.camera.pitch.to_degrees()));
                    ui.monospace(format!("Yaw (Azimuth):{:.1}°", self.map.camera.yaw.to_degrees()));
                    ui.monospace(format!("FOV Y:       {:.1}°", self.map.camera.fov_y.to_degrees()));
                });

                ui.separator();

                // Camera Presets
                ui.collapsing("📍 Camera Presets", |ui| {
                    if ui.button("🌆 Melbourne CBD (3D Orbit)").clicked() {
                        if self.map.projection_mode == ProjectionMode::GlobeECEF {
                            self.map.transition_to_planar_at_geo(-37.8136, 144.9631, 1800.0);
                        } else {
                            self.map.camera.target = glam::Vec3::ZERO;
                            self.map.camera.pitch = 45.0f32.to_radians();
                            self.map.camera.yaw = (-30.0f32).to_radians();
                            self.map.camera.distance = 1800.0;
                            self.map.camera.snap_smoothing();
                        }
                        ctx.request_repaint();
                    }
                    if ui.button("🌐 Global Earth Orbit (Globe)").clicked() {
                        self.map.transition_to_globe();
                        ctx.request_repaint();
                    }
                    if ui.button("🗺 Nadir (Top-Down North)").clicked() {
                        self.map.camera.pitch = 89.9f32.to_radians();
                        self.map.camera.yaw = 0.0;
                        self.map.camera.snap_smoothing();
                        ctx.request_repaint();
                    }
                    if ui.button("📐 Isometric 45° Perspective").clicked() {
                        self.map.camera.pitch = 35.264f32.to_radians();
                        self.map.camera.yaw = 45.0f32.to_radians();
                        self.map.camera.snap_smoothing();
                        ctx.request_repaint();
                    }
                    if ui.button("🚶 Low-Angle Horizon View").clicked() {
                        self.map.camera.pitch = 15.0f32.to_radians();
                        self.map.camera.distance = 800.0;
                        self.map.camera.snap_smoothing();
                        ctx.request_repaint();
                    }
                });

                ui.separator();

                // Environment & Sun Lighting
                ui.collapsing("☀️ Sun & Environment", |ui| {
                    ui.checkbox(&mut self.map.sunlight_enabled, "Enable Direct Sunlight");
                    ui.add(egui::Slider::new(&mut self.map.sun_intensity, 0.0..=3.0).text("Sun Intensity"));
                    ui.add(egui::Slider::new(&mut self.map.ambient_intensity, 0.0..=1.5).text("Ambient"));

                    let sun_dir = self.map.solar_pos.sun_direction;
                    ui.monospace(format!("Sun Elevation: {:+.1}°", self.map.solar_pos.elevation_deg));
                    ui.monospace(format!("Sun Azimuth:   {:.1}°", self.map.solar_pos.azimuth_deg));
                    ui.monospace(format!("Sun Dir: [{:.2}, {:.2}, {:.2}]", sun_dir.x, sun_dir.y, sun_dir.z));
                });

                ui.separator();

                // Mouse Cursor Telemetry
                ui.collapsing("🎯 Spatial Picking", |ui| {
                    if let Some(hover) = self.hovered_geo {
                        ui.label("Hovered Ground Point:");
                        ui.monospace(format!("  Lat: {:+.6}°", hover.latitude));
                        ui.monospace(format!("  Lon: {:+.6}°", hover.longitude));
                    } else {
                        ui.label(egui::RichText::new("Hover map to inspect coords").italics().color(egui::Color32::GRAY));
                    }

                    if let Some(click) = self.clicked_geo {
                        ui.add_space(4.0);
                        ui.label("Last Clicked Location:");
                        ui.monospace(format!("  Lat: {:+.6}°", click.latitude));
                        ui.monospace(format!("  Lon: {:+.6}°", click.longitude));
                        ui.monospace(click.to_dms_string());
                    }
                });

                ui.separator();

                // Navigation Instructions
                ui.collapsing("ℹ️ Controls Help", |ui| {
                    ui.label("• Left Click + Drag: Cursor-locked Pan / Rotate");
                    ui.label("• Right Click + Drag (or Middle Drag / Alt+Drag): Orbit & Tilt");
                    ui.label("• Mouse Scroll: Smooth Zoom (Auto-switches to Globe when zoomed out past 50km)");
                    ui.label("• Left Click: Query coordinates");
                });
            });

        // --- Central 3D Map Viewport ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(18, 18, 24)))
            .show(ctx, |ui| {
                let response = MapWidget::new(&mut self.map).show(ui);

                // Convert hovered world point to geographic coordinates
                if let Some(hover_pt) = response.hovered_world_point {
                    self.hovered_geo = if self.map.projection_mode == ProjectionMode::GlobeECEF {
                        Some(s3d_core::gis::crs::ecef_to_geodetic(hover_pt))
                    } else {
                        Some(self.map.scene.origin.local_to_geo(hover_pt))
                    };
                } else {
                    self.hovered_geo = None;
                }

                // Process clicked world point
                if let Some(click_pt) = response.clicked_world_point {
                    self.clicked_geo = if self.map.projection_mode == ProjectionMode::GlobeECEF {
                        Some(s3d_core::gis::crs::ecef_to_geodetic(click_pt))
                    } else {
                        Some(self.map.scene.origin.local_to_geo(click_pt))
                    };
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
                flags: wgpu::InstanceFlags::empty(), // Disables Vulkan validation layer error spam
                backend_options: Default::default(),
            },
            power_preference: wgpu::PowerPreference::HighPerformance,
            device_descriptor: std::sync::Arc::new(|_adapter| wgpu::DeviceDescriptor {
                label: Some("S3D Device"),
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
            .with_title("S3D — Basic Map Engine Demo")
            .with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };

    eframe::run_native(
        "S3D Basic Map",
        native_options,
        Box::new(|cc| Ok(Box::new(BasicMapApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;
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
        let document = web_sys::window().expect("No window").document().expect("No document");
        let canvas = document
            .get_element_by_id("s3d_canvas")
            .expect("No s3d_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("Not canvas");

        eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|cc| Ok(Box::new(BasicMapApp::new(cc)))))
            .await
            .expect("failed to start s3d on web");
    });
}
