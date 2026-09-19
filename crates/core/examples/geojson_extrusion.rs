//! 3D GeoJSON Extrusion & Solar Shadow Study Example — S3D Core
//!
//! Loads Melbourne CBD 3D building footprints, demonstrates automatic 2.5D-to-3D polygon
//! triangulation/extrusion, real-time sun/shadow simulation, and interactive 3D feature picking.
//!
//! Run with:
//! `cargo run --example geojson_extrusion`

use chrono::{TimeZone, Utc};
use eframe::egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::renderer::render_engine::RenderEngine;
use s3d_core::solar::sun_calc::calculate_solar_position;

struct GeojsonExtrusionApp {
    map: MapEngine,
    // Solar simulation state
    sim_hour: f32,
    sim_month: u32,
    sim_day: u32,
    animating_sun: bool,
    anim_speed: f32, // Hours per real-time second
}

impl GeojsonExtrusionApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Melbourne CBD coordinates
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        let mut map = MapEngine::new(ProjectOrigin::from_geo(melbourne));

        // Initial camera looking towards Melbourne CBD skyscrapers
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

        // Load Melbourne CBD sample buildings (includes Eureka Tower, Rialto Towers, etc.)
        map.load_sample_buildings();

        // Enable sunlight and crisp directional shadows
        map.sunlight_enabled = true;
        map.sun_intensity = 1.6;
        map.ambient_intensity = 0.45;

        // Default to midday on summer solstice in Melbourne (Dec 21, 13:00 AEDT)
        let sim_hour = 13.0;
        let sim_month = 12;
        let sim_day = 21;

        let mut app = Self {
            map,
            sim_hour,
            sim_month,
            sim_day,
            animating_sun: false,
            anim_speed: 1.0,
        };

        app.update_solar_position();
        app
    }

    fn update_solar_position(&mut self) {
        let h = self.sim_hour.floor() as u32;
        let m = ((self.sim_hour - h as f32) * 60.0).floor() as u32;
        let s = (((self.sim_hour - h as f32) * 60.0 - m as f32) * 60.0).floor() as u32;

        let year = 2025;
        let local_dt = Utc.with_ymd_and_hms(year, self.sim_month, self.sim_day.clamp(1, 28), h, m, s).unwrap();

        // Melbourne is UTC+10 (AEST) or UTC+11 (AEDT daylight saving)
        let tz_offset = if self.sim_month >= 10 || self.sim_month <= 3 { 11.0 } else { 10.0 };

        let origin_geo = self.map.scene.origin.origin;
        self.map.solar_pos = calculate_solar_position(
            origin_geo.latitude,
            origin_geo.longitude,
            &local_dt,
            tz_offset,
        );
    }
}

impl eframe::App for GeojsonExtrusionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.map.update(dt);

        // Advance sun animation if playing
        if self.animating_sun {
            self.sim_hour = (self.sim_hour + self.anim_speed * dt) % 24.0;
            self.update_solar_position();
            ctx.request_repaint();
        }

        if self.map.camera.is_animating() {
            ctx.request_repaint();
        }

        // --- Left Sidebar: Solar Simulation & Feature Inspector ---
        egui::SidePanel::left("extrusion_side_panel")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("🏙 3D Buildings & Sun Simulation");
                ui.label(
                    egui::RichText::new("Polygon Extrusion, Shadows & Picking")
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.separator();

                // Solar Shadow Study Panel
                ui.collapsing("☀️ Solar Shadow Simulation", |ui| {
                    ui.checkbox(&mut self.map.sunlight_enabled, "Enable Sunlight & Shadows");

                    let mut time_changed = false;

                    // Hour of Day Slider
                    ui.label("Local Time of Day:");
                    let hour_label = format!("{:02}:{:02}", self.sim_hour.floor() as u32, ((self.sim_hour.fract()) * 60.0) as u32);
                    if ui.add(egui::Slider::new(&mut self.sim_hour, 0.0..=24.0).text(hour_label)).changed() {
                        time_changed = true;
                    }

                    // Month Slider
                    ui.label("Month (Season):");
                    let month_name = match self.sim_month {
                        1 => "Jan (Summer)",
                        2 => "Feb (Summer)",
                        3 => "Mar (Autumn)",
                        4 => "Apr (Autumn)",
                        5 => "May (Autumn)",
                        6 => "Jun (Winter)",
                        7 => "Jul (Winter)",
                        8 => "Aug (Winter)",
                        9 => "Sep (Spring)",
                        10 => "Oct (Spring)",
                        11 => "Nov (Spring)",
                        12 => "Dec (Summer)",
                        _ => "",
                    };
                    if ui.add(egui::Slider::new(&mut self.sim_month, 1..=12).text(month_name)).changed() {
                        time_changed = true;
                    }

                    if time_changed {
                        self.update_solar_position();
                    }

                    ui.add_space(4.0);

                    // Animation Controls
                    ui.horizontal(|ui| {
                        if ui.button(if self.animating_sun { "⏸ Pause Sun" } else { "▶ Animate Sun" }).clicked() {
                            self.animating_sun = !self.animating_sun;
                        }
                        ui.add(egui::Slider::new(&mut self.anim_speed, 0.1..=5.0).text("Speed"));
                    });

                    ui.add_space(4.0);

                    // Sun Angle Telemetry
                    let pos = &self.map.solar_pos;
                    ui.monospace(format!("Sun Elevation: {:+.1}° {}", pos.elevation_deg, if pos.is_daylight { "☀️ Daylight" } else { "🌙 Night" }));
                    ui.monospace(format!("Sun Azimuth:   {:.1}°", pos.azimuth_deg));
                    ui.monospace(format!("Sunrise:       ~{:02}:00", pos.sunrise_hour.floor() as u32));
                    ui.monospace(format!("Sunset:        ~{:02}:00", pos.sunset_hour.floor() as u32));
                });

                ui.separator();

                // Selected Feature Inspector
                ui.collapsing("🏢 Building Inspector", |ui| {
                    if let Some(feat) = &self.map.selected_feature {
                        ui.label(
                            egui::RichText::new(&feat.name)
                                .strong()
                                .size(15.0)
                                .color(egui::Color32::from_rgb(96, 165, 250)),
                        );
                        ui.add_space(2.0);

                        ui.monospace(format!("Building ID:   {}", feat.id));
                        ui.monospace(format!("Type:          {}", feat.feature_type));
                        ui.monospace(format!("Height:        {:.1} m", feat.height));

                        if let Some(levels) = feat.properties.get("levels") {
                            ui.monospace(format!("Storeys:       {} floors", levels));
                        }
                        if let Some(usage) = feat.properties.get("use") {
                            ui.monospace(format!("Usage:         {}", usage));
                        }
                        if let Some(arch) = feat.properties.get("architect") {
                            ui.monospace(format!("Architect:     {}", arch));
                        }

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Centroid Location:").small());
                        ui.monospace(format!("  Lat: {:+.6}°", feat.center_geo.latitude));
                        ui.monospace(format!("  Lon: {:+.6}°", feat.center_geo.longitude));

                        if !feat.properties.is_empty() {
                            ui.add_space(4.0);
                            ui.collapsing("All Properties", |ui| {
                                for (k, v) in &feat.properties {
                                    ui.monospace(format!("{}: {}", k, v));
                                }
                            });
                        }

                        ui.add_space(6.0);
                        if ui.button("🎯 Center View on Building").clicked() {
                            self.map.camera.target = feat.center_local;
                            self.map.camera.distance = 500.0;
                            self.map.camera.pitch = 30.0f32.to_radians();
                            self.map.camera.snap_smoothing();
                            ctx.request_repaint();
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("👉 Click on any building in the 3D map to inspect its height, properties, and cast shadows.")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    }
                });

                ui.separator();

                // Architectural Edge & Styling Controls
                ui.collapsing("🎨 Layer & Edge Styling", |ui| {
                    if let Some(r) = &mut self.map.renderer {
                        ui.checkbox(&mut r.edge_renderer.config.enabled, "Architectural Edges");
                        ui.add(egui::Slider::new(&mut r.edge_renderer.config.width, 0.5..=3.0).text("Edge Width"));
                        ui.add(egui::Slider::new(&mut r.edge_renderer.config.depth_threshold, 0.01..=0.5).text("Depth Sensitivity"));
                    }

                    if let Some(lyr) = self.map.layers.first_mut() {
                        ui.add(egui::Slider::new(&mut lyr.opacity, 0.1..=1.0).text("Layer Opacity"));
                        ui.checkbox(&mut lyr.cast_shadows, "Cast Shadows");
                    }
                });

                ui.separator();

                // Quick Preset Views
                ui.collapsing("🎥 Landmark Presets", |ui| {
                    if ui.button("🌟 Eureka Tower (297m)").clicked() {
                        // Eureka Tower position
                        self.map.camera.target = glam::Vec3::new(96.0, 0.0, 840.0);
                        self.map.camera.distance = 750.0;
                        self.map.camera.pitch = 35.0f32.to_radians();
                        self.map.camera.yaw = (-45.0f32).to_radians();
                        self.map.camera.snap_smoothing();
                        ctx.request_repaint();
                    }
                    if ui.button("🏢 Rialto Towers (251m)").clicked() {
                        self.map.camera.target = glam::Vec3::new(-450.0, 0.0, 150.0);
                        self.map.camera.distance = 700.0;
                        self.map.camera.pitch = 32.0f32.to_radians();
                        self.map.camera.yaw = 20.0f32.to_radians();
                        self.map.camera.snap_smoothing();
                        ctx.request_repaint();
                    }
                    if ui.button("🌆 Full CBD Overview").clicked() {
                        self.map.camera.target = glam::Vec3::ZERO;
                        self.map.camera.distance = 1800.0;
                        self.map.camera.pitch = 40.0f32.to_radians();
                        self.map.camera.yaw = (-25.0f32).to_radians();
                        self.map.camera.snap_smoothing();
                        ctx.request_repaint();
                    }
                });
            });

        // --- Central 3D Map Viewport ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(18, 20, 26)))
            .show(ctx, |ui| {
                MapWidget::new(&mut self.map).show(ui);
            });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("S3D — 3D Buildings Extrusion & Solar Shadow Study")
            .with_inner_size([1360.0, 880.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "S3D GeoJSON Extrusion",
        native_options,
        Box::new(|cc| Ok(Box::new(GeojsonExtrusionApp::new(cc)))),
    )
}
