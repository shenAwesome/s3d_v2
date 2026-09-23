#[cfg(feature = "egui")]
use glam::Vec3;
#[cfg(feature = "egui")]
use crate::engine::map_engine::MapEngine;
#[cfg(feature = "egui")]
use crate::engine::event::MapEvent;
#[cfg(feature = "egui")]
use crate::gis::crs::ProjectionMode;

#[cfg(feature = "egui")]
/// Response returned from rendering MapEngine into an egui::Ui
pub struct MapResponse {
    pub response: egui::Response,
    pub hovered_world_point: Option<Vec3>,
    pub clicked_world_point: Option<Vec3>,
    pub camera_moved: bool,
}

#[cfg(feature = "egui")]
/// Standalone, reusable Map Widget for egui
pub struct MapWidget<'a> {
    engine: &'a mut MapEngine,
}

#[cfg(feature = "egui")]
impl<'a> MapWidget<'a> {
    pub fn new(engine: &'a mut MapEngine) -> Self {
        Self { engine }
    }

    pub fn show(self, ui: &mut egui::Ui) -> MapResponse {
        self.engine.show_ui(ui)
    }
}

#[cfg(feature = "egui")]
impl<'a> egui::Widget for MapWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.engine.show_ui(ui).response
    }
}

#[cfg(feature = "egui")]
impl MapEngine {
    /// Convenient shorthand to render the map directly into an egui container.
    ///
    /// Shorthand for `MapWidget::new(&mut map).show(ui)`.
    pub fn show(&mut self, ui: &mut egui::Ui) -> MapResponse {
        self.show_ui(ui)
    }

    /// Renders the MapEngine canvas widget inside the given egui::Ui container.
    pub fn show_ui(&mut self, ui: &mut egui::Ui) -> MapResponse {
        self.sync_terrain_if_changed();

        let available_size = ui.available_size();
        let width = (available_size.x.floor() as u32).max(64);
        let height = (available_size.y.floor() as u32).max(64);

        let Some(renderer) = &mut self.renderer else {
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(width as f32, height as f32),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(24, 24, 28));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "S3D Map: Renderer Initializing...",
                egui::FontId::proportional(14.0),
                egui::Color32::GRAY,
            );
            return MapResponse {
                response,
                hovered_world_point: None,
                clicked_world_point: None,
                camera_moved: false,
            };
        };

        // 1. GPU 3D Render Pass
        renderer.projection_mode = self.projection_mode;
        renderer.morph_progress = if self.projection_mode == ProjectionMode::GlobeECEF {
            1.0
        } else {
            0.0
        };
        renderer.resize(width, height);

        let dummy_measurement = crate::spatial::measurement::MeasurementEngine::new();
        let dummy_toolbox = crate::spatial::toolbox::ToolboxEngine::default();

        renderer.render(
            &self.scene,
            &self.camera,
            &self.solar_pos,
            &dummy_measurement,
            &dummy_toolbox,
            self.basemap.is_enabled || self.terrain_mgr.is_enabled,
            self.basemap.zoom,
            self.sun_intensity,
            self.ambient_intensity,
            self.sunlight_enabled,
        );

        // 2. Allocate Canvas Rect in egui
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width as f32, height as f32),
            egui::Sense::click_and_drag(),
        );

        // Paint rendered texture with exact UV mapping
        let uv_max_x = (renderer.current_width as f32) / (crate::renderer::render_engine::MAX_VIEWPORT_WIDTH as f32);
        let uv_max_y = (renderer.current_height as f32) / (crate::renderer::render_engine::MAX_VIEWPORT_HEIGHT as f32);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(uv_max_x, uv_max_y));
        ui.painter().image(
            renderer.egui_texture_id,
            rect,
            uv,
            egui::Color32::WHITE,
        );

        let mut camera_moved = false;
        let mut hovered_world_point = None;
        let mut clicked_world_point = None;

        if response.drag_started() {
            self.mouse_drag_distance = 0.0;
        }

        // 3. User Navigation Interaction (Cursor-locked Pan, Screen-pinned Orbit, Pinned Zoom)
        camera_moved |= crate::engine::navigation::NavigationController::handle_drag(
            self,
            ui.ctx(),
            &response,
            rect,
            width,
            height,
        );

        camera_moved |= crate::engine::navigation::NavigationController::handle_scroll_zoom(
            self,
            ui.ctx(),
            &response,
            rect,
            width,
            height,
        );

        camera_moved |= crate::engine::navigation::NavigationController::handle_pointer_release(
            self,
            ui.ctx(),
        );

        // Hover picking
        if let Some(pos) = response.hover_pos() {
            let local_x = pos.x - rect.min.x;
            let local_y = pos.y - rect.min.y;
            let ray = self.screen_to_ray(local_x, local_y, width as f32, height as f32);
            if let Some(hit) = self.intersect_scene_or_terrain(&ray) {
                hovered_world_point = Some(hit);
            }
        }

        // Click picking (only when not dragging)
        if response.clicked() && self.mouse_drag_distance < 6.0 {
            if let Some(pos) = response.interact_pointer_pos() {
                let local_x = pos.x - rect.min.x;
                let local_y = pos.y - rect.min.y;
                let ray = self.screen_to_ray(local_x, local_y, width as f32, height as f32);

                if let Some((feat, hit)) = self.pick_feature(&ray) {
                    self.select_feature(Some(feat));
                    clicked_world_point = Some(hit);
                } else if let Some(hit) = self.intersect_scene_or_terrain(&ray) {
                    self.select_feature(None);
                    let geo = self.scene.origin.local_to_geo(hit);
                    self.events.push(MapEvent::CoordinatesClicked { geo, local: hit });
                    clicked_world_point = Some(hit);
                } else {
                    self.select_feature(None);
                }
            }
        }

        // Reset drag distance when released
        if ui.input(|i| i.pointer.any_released()) {
            self.mouse_drag_distance = 0.0;
        }

        if camera_moved {
            self.events.push(MapEvent::CameraMoved);
        }

        MapResponse {
            response,
            hovered_world_point,
            clicked_world_point,
            camera_moved,
        }
    }
}
