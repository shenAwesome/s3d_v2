//! S3D Core Examples — OpenLayers Style
//!
//! Clean, minimal-UI examples demonstrating individual `s3d-core` engine features.
//! Modeled directly after OpenLayers Examples (https://openlayers.org/en/latest/examples/):
//! - Zero extraneous UI dashboards, telemetry panels, or fake toolbars.
//! - Pure Map Viewport (`MapWidget`).
//! - Dedicated Code Section showing the **actual source file** for each demo.
//!
//! Run native desktop:
//! `cargo run --example showcase`
//!
//! Run browser (WASM):
//! `demo.bat`

#[path = "demos/mod.rs"]
mod demos;

use demos::{DemoEntry, demo_index_by_id};
use eframe::egui;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapWidget;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::renderer::render_engine::RenderEngine;

// ============================================================================
// URL / CLI Routing
// ============================================================================

#[cfg(target_arch = "wasm32")]
fn get_example_id_from_url() -> Option<String> {
    let window = web_sys::window()?;
    let location = window.location();
    if let Ok(hash) = location.hash() {
        let clean = hash.trim_start_matches('#').trim();
        if !clean.is_empty() {
            let id = clean.split(&['?', '&', ':', '/'][..]).next().unwrap_or(clean);
            return Some(id.to_ascii_lowercase().replace('-', "_"));
        }
    }
    if let Ok(search) = location.search() {
        let clean = search.trim_start_matches('?');
        for part in clean.split('&') {
            let mut kv = part.split('=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                if k == "example" || k == "ex" {
                    return Some(v.to_ascii_lowercase().replace('-', "_"));
                }
            }
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn sync_url_hash(demos: &[DemoEntry], idx: usize) {
    if let Some(window) = web_sys::window() {
        let target = format!("#{}", demos[idx].demo.id());
        if let Ok(cur) = window.location().hash() {
            if !cur.starts_with(&target) {
                let _ = window.location().set_hash(&target);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn get_initial_example_id() -> Option<String> {
    for arg in std::env::args().skip(1) {
        let clean = arg.trim_start_matches("--example=").trim_start_matches("--");
        if !clean.is_empty() {
            return Some(clean.to_ascii_lowercase().replace('-', "_"));
        }
    }
    None
}

// ============================================================================
// Showcase Application
// ============================================================================

pub struct ShowcaseApp {
    engine: MapEngine,
    demos: Vec<DemoEntry>,
    active_idx: usize,
    code_text: String,
    last_copied_at: Option<f64>,
}

impl ShowcaseApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);

        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        let mut engine = MapEngine::new(ProjectOrigin::from_geo(melbourne));

        // Default camera
        engine.goto(
            glam::Vec3::ZERO,
            GoToOptions::immediate()
                .with_distance(1800.0)
                .with_heading(0.0)
                .with_pitch(35.0),
        );

        // Attach GPU renderer
        if let Some(render_state) = &cc.wgpu_render_state {
            let renderer = RenderEngine::new(
                render_state.device.clone().into(),
                render_state.queue.clone().into(),
                render_state,
            );
            engine.set_renderer(renderer);
        }

        let demos = demos::all_demos();

        // Resolve initial example from CLI or URL
        #[cfg(not(target_arch = "wasm32"))]
        let initial_idx = get_initial_example_id()
            .and_then(|id| demo_index_by_id(&demos, &id))
            .unwrap_or(0);
        #[cfg(target_arch = "wasm32")]
        let initial_idx = get_example_id_from_url()
            .and_then(|id| demo_index_by_id(&demos, &id))
            .unwrap_or(0);

        let code_text = demos[initial_idx].source.to_string();

        let mut app = Self {
            engine,
            demos,
            active_idx: initial_idx,
            code_text,
            last_copied_at: None,
        };

        app.demos[initial_idx].demo.setup(&mut app.engine);

        #[cfg(target_arch = "wasm32")]
        sync_url_hash(&app.demos, app.active_idx);

        app
    }

    fn switch_to(&mut self, idx: usize) {
        if idx == self.active_idx { return; }
        self.active_idx = idx;
        self.code_text = self.demos[idx].source.to_string();

        // Reset map state before activating new demo
        self.engine.clear_layers();
        self.engine.terrain.is_enabled = false;
        self.engine.selected_feature = None;
        if let Some(r) = &mut self.engine.renderer {
            r.set_selected_mesh(None);
            r.edge_renderer.config.enabled = false;
        }
        self.engine.align_north();

        self.demos[idx].demo.setup(&mut self.engine);

        #[cfg(target_arch = "wasm32")]
        sync_url_hash(&self.demos, idx);
    }
}

impl eframe::App for ShowcaseApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.engine.update(dt);

        // WASM: sync example from URL hash changes
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(url_id) = get_example_id_from_url() {
                if let Some(idx) = demo_index_by_id(&self.demos, &url_id) {
                    if idx != self.active_idx {
                        self.switch_to(idx);
                    }
                }
            }
        }

        // Request repaint when streaming or animating
        if self.engine.camera.is_animating()
            || self.engine.has_new_gpu_tiles
            || self.engine.basemap.has_unconsumed_completed()
            || self.engine.is_streaming()
        {
            ctx.request_repaint();
        }

        // --- Top Bar: Example Selector + Demo Controls ---
        egui::TopBottomPanel::top("example_top_bar")
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(22, 24, 30)).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new("🗺 S3D Core")
                            .strong()
                            .color(egui::Color32::from_rgb(96, 165, 250)),
                    );
                    ui.separator();

                    // Example selector dropdown
                    let current_title = self.demos[self.active_idx].demo.title();
                    egui::ComboBox::from_id_salt("core_example_selector")
                        .selected_text(current_title)
                        .width(250.0)
                        .show_ui(ui, |ui| {
                            for i in 0..self.demos.len() {
                                let title = self.demos[i].demo.title();
                                if ui.selectable_label(i == self.active_idx, title).clicked() {
                                    self.switch_to(i);
                                }
                            }
                        });

                    ui.separator();

                    // Delegate demo-specific controls to the active demo
                    if self.demos[self.active_idx].demo.controls(ui, &mut self.engine) {
                        ctx.request_repaint();
                    }
                });
            });

        // --- Right Panel: Source Code (actual file via include_str!) ---
        let initial_panel_width = (ctx.screen_rect().width() * 0.45).clamp(340.0, 680.0);
        egui::SidePanel::right("code_side_panel")
            .resizable(true)
            .default_width(initial_panel_width)
            .min_width(280.0)
            .max_width(ctx.screen_rect().width() * 0.75)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(15, 17, 23)).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(self.demos[self.active_idx].demo.title())
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let current_time = ctx.input(|i| i.time);
                        let is_recently_copied = self
                            .last_copied_at
                            .map_or(false, |t| current_time - t < 2.0);

                        if is_recently_copied {
                            ui.label(
                                egui::RichText::new("✓ Copied!")
                                    .color(egui::Color32::from_rgb(52, 211, 153)),
                            );
                        } else if ui.button("📋 Copy All").clicked() {
                            ctx.copy_text(self.code_text.clone());
                            self.last_copied_at = Some(current_time);
                        }

                        ui.label(
                            egui::RichText::new("Rust (s3d-core)")
                                .monospace()
                                .color(egui::Color32::from_rgb(147, 197, 253)),
                        );
                    });
                });

                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(self.demos[self.active_idx].demo.description())
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.separator();

                // Display the actual source file — what you see IS what runs
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.code_text)
                                .font(egui::TextStyle::Monospace)
                                .text_color(egui::Color32::from_rgb(226, 232, 240))
                                .desired_width(f32::INFINITY)
                                .lock_focus(true),
                        );
                    });
            });

        // --- Central Panel: 3D Map Viewport ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(10, 12, 16)))
            .show(ctx, |ui| {
                let response = MapWidget::new(&mut self.engine).show(ui);

                // Delegate click/pick handling to the active demo
                self.demos[self.active_idx].demo.on_map_response(&response, &mut self.engine);
            });
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
