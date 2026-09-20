use glam::{Vec2, Vec3};
use crate::engine::map_engine::MapEngine;
use crate::gis::crs::{ecef_to_geodetic, ProjectionMode, WGS84_A};
use crate::renderer::camera::Camera;

#[derive(Debug, Clone, Copy)]
pub struct PanState {
    pub grab_point_world: Vec3,
    pub plane_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct OrbitState {
    pub anchor_world: Vec3,
    pub anchor_dist: f32,
    pub local_ray_dir: Vec3,
    pub start_pitch: f32,
    pub start_yaw: f32,
    pub start_distance: f32,
    pub start_pos: egui::Pos2,
}

#[derive(Debug, Clone, Copy)]
pub struct ZoomAnchor {
    pub anchor_world: Vec3,
    pub start_pos: egui::Pos2,
    pub timestamp: web_time::Instant,
}

/// Unified GIS & 3D Navigation Controller providing cursor-locked surface panning,
/// screen-pinned anchor orbiting, and smooth zoom across all viewports.
pub struct NavigationController;

impl NavigationController {
    /// Handles pointer dragging (Globe spin, Planar Orbit/Tilt around anchor, Planar Pan cursor-locked).
    /// Returns true if the camera was moved.
    pub fn handle_drag(
        engine: &mut MapEngine,
        ctx: &egui::Context,
        response: &egui::Response,
        rect: egui::Rect,
        width: u32,
        height: u32,
    ) -> bool {
        let mut camera_moved = false;

        if response.hovered() || response.dragged() {
            let pointer_delta = ctx.input(|i| i.pointer.delta());
            let is_primary_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
            let is_secondary_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
            let is_middle_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
            let is_any_button_down = is_primary_down || is_secondary_down || is_middle_down;
            let modifiers = ctx.input(|i| i.modifiers);

            if is_any_button_down && pointer_delta.length_sq() > 0.0 {
                engine.zoom_anchor = None;
                engine.mouse_drag_distance += pointer_delta.length();
                if engine.active_globe_flight.is_some() {
                    engine.active_globe_flight = None;
                }

                let vp_size = Vec2::new(width as f32, height as f32);

                if engine.projection_mode == ProjectionMode::GlobeECEF {
                    engine.camera.target = glam::Vec3::ZERO;

                    if let Some(curr_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        let prev_pos = curr_pos - pointer_delta;
                        let prev_local = prev_pos - rect.min;
                        let curr_local = curr_pos - rect.min;

                        let ray_prev = engine.screen_to_ray(prev_local.x, prev_local.y, vp_size.x, vp_size.y);
                        let ray_curr = engine.screen_to_ray(curr_local.x, curr_local.y, vp_size.x, vp_size.y);

                        let hit_prev = ray_prev.intersect_sphere(glam::Vec3::ZERO, WGS84_A as f32);
                        let hit_curr = ray_curr.intersect_sphere(glam::Vec3::ZERO, WGS84_A as f32);

                        if let (Some(p0), Some(p1)) = (hit_prev, hit_curr) {
                            let geo0 = ecef_to_geodetic(p0);
                            let geo1 = ecef_to_geodetic(p1);

                            let d_lat = (geo1.latitude - geo0.latitude).to_radians();
                            let mut d_lon = (geo1.longitude - geo0.longitude).to_radians();
                            while d_lon > std::f64::consts::PI { d_lon -= std::f64::consts::TAU; }
                            while d_lon < -std::f64::consts::PI { d_lon += std::f64::consts::TAU; }

                            engine.camera.pitch = (engine.camera.pitch - d_lat as f32).clamp(-1.54, 1.54);
                            engine.camera.yaw -= d_lon as f32;
                            engine.camera.normalize_yaw();
                            engine.camera.target_yaw = engine.camera.yaw;
                            engine.camera.target_pitch = engine.camera.pitch;
                            camera_moved = true;
                        } else {
                            let speed = (engine.camera.distance / 6_378_137.0).clamp(0.5, 4.0) * 0.0018;
                            engine.camera.yaw -= pointer_delta.x * speed;
                            engine.camera.pitch = (engine.camera.pitch + pointer_delta.y * speed).clamp(-1.54, 1.54);
                            engine.camera.normalize_yaw();
                            engine.camera.target_yaw = engine.camera.yaw;
                            engine.camera.target_pitch = engine.camera.pitch;
                            camera_moved = true;
                        }
                    }
                } else if is_secondary_down || is_middle_down || (is_primary_down && (modifiers.alt || modifiers.ctrl)) {
                    // Rotate & Tilt (Orbit around screen-pinned anchor)
                    if let Some(curr_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        let orbit = if let Some(o) = engine.orbit_state {
                            o
                        } else {
                            let curr_local = curr_pos - rect.min;
                            let ray = engine.screen_to_ray(curr_local.x, curr_local.y, vp_size.x, vp_size.y);
                            let anchor_pt = engine.intersect_scene_or_terrain(&ray).unwrap_or_else(|| {
                                ray.intersect_ground_plane(0.0).unwrap_or(engine.camera.target)
                            });
                            let eye0 = engine.camera.eye_position();
                            let v0 = anchor_pt - eye0;
                            let dist0 = v0.length().max(0.1);
                            let u0 = v0 / dist0;
                            let (right0, up0, forward0) = engine.camera.basis_vectors();
                            let local_ray_dir = glam::Vec3::new(
                                u0.dot(right0),
                                u0.dot(up0),
                                u0.dot(forward0),
                            );
                            let o = OrbitState {
                                anchor_world: anchor_pt,
                                anchor_dist: dist0,
                                local_ray_dir,
                                start_pitch: engine.camera.pitch,
                                start_yaw: engine.camera.yaw,
                                start_distance: engine.camera.distance,
                                start_pos: curr_pos,
                            };
                            engine.orbit_state = Some(o);
                            o
                        };

                        let delta_x = curr_pos.x - orbit.start_pos.x;
                        let delta_y = curr_pos.y - orbit.start_pos.y;
                        let sensitivity = 0.005;

                        let mut new_yaw = orbit.start_yaw - delta_x * sensitivity;
                        while new_yaw > std::f32::consts::PI { new_yaw -= std::f32::consts::TAU; }
                        while new_yaw < -std::f32::consts::PI { new_yaw += std::f32::consts::TAU; }

                        let new_pitch = (orbit.start_pitch + delta_y * sensitivity)
                            .clamp(Camera::MIN_PITCH_PLANAR, Camera::MAX_PITCH_PLANAR);

                        let (right_new, up_new, forward_new) = Camera::basis_vectors_for(new_yaw, new_pitch);

                        let d_world = (orbit.local_ray_dir.x * right_new
                            + orbit.local_ray_dir.y * up_new
                            + orbit.local_ray_dir.z * forward_new).normalize_or_zero();

                        if d_world.length_squared() > 0.5 {
                            let new_eye = orbit.anchor_world - d_world * orbit.anchor_dist;
                            let new_target = new_eye + forward_new * orbit.start_distance;

                            engine.camera.target = new_target;
                            engine.camera.target_lookat = new_target;
                            engine.camera.yaw = new_yaw;
                            engine.camera.pitch = new_pitch;
                            engine.camera.target_yaw = new_yaw;
                            engine.camera.target_pitch = new_pitch;
                            engine.camera.distance = orbit.start_distance;
                            engine.camera.target_distance = orbit.start_distance;
                            camera_moved = true;
                        }
                    }
                } else if is_primary_down {
                    // Pan: Cursor-locked terrain translation (camera ground distance invariant)
                    if let Some(curr_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        let curr_local = curr_pos - rect.min;
                        let ray_curr = engine.screen_to_ray(curr_local.x, curr_local.y, vp_size.x, vp_size.y);

                        let pan = if let Some(p) = engine.pan_state {
                            p
                        } else {
                            let grab_pt = engine.intersect_scene_or_terrain(&ray_curr).unwrap_or_else(|| {
                                ray_curr.intersect_ground_plane(0.0).unwrap_or(engine.camera.target)
                            });
                            let p = PanState {
                                grab_point_world: grab_pt,
                                plane_y: grab_pt.y,
                            };
                            engine.pan_state = Some(p);
                            p
                        };

                        if let Some(plane_hit) = ray_curr.intersect_ground_plane(pan.plane_y) {
                            let delta = pan.grab_point_world - plane_hit;
                            let max_pan_step = (engine.camera.distance * 2.5).max(100.0);
                            if delta.length() < max_pan_step {
                                engine.camera.target.x += delta.x;
                                engine.camera.target.z += delta.z;
                                engine.camera.target_lookat = engine.camera.target;
                                camera_moved = true;
                            }
                        } else {
                            let factor = engine.camera.distance * 0.001;
                            engine.camera.pan(-pointer_delta.x * factor, pointer_delta.y * factor);
                            camera_moved = true;
                        }
                    }
                }
            }
        }

        camera_moved
    }

    /// Handles mouse wheel scroll zoom with cursor-pinning
    pub fn handle_scroll_zoom(
        engine: &mut MapEngine,
        ctx: &egui::Context,
        response: &egui::Response,
        rect: egui::Rect,
        width: u32,
        height: u32,
    ) -> bool {
        let scroll_delta = ctx.input(|i| {
            let raw = i.raw_scroll_delta.y;
            if raw != 0.0 { raw } else { i.smooth_scroll_delta.y }
        });

        if scroll_delta == 0.0 || !response.hovered() {
            return false;
        }

        engine.pan_state = None;
        engine.orbit_state = None;
        if engine.active_globe_flight.is_some() {
            engine.active_globe_flight = None;
        }

        let vp_size = Vec2::new(width as f32, height as f32);

        if engine.projection_mode == ProjectionMode::GlobeECEF {
            const WGS84_RADIUS: f32 = WGS84_A as f32;
            let cur_alt = (engine.camera.target_distance - WGS84_RADIUS).max(10.0);
            let steps = (scroll_delta / 50.0).clamp(-4.0, 4.0);
            let factor = 0.74f32.powf(steps);
            let new_alt = (cur_alt * factor).clamp(10.0, 80_000_000.0);

            // Automatic Zoom Landing: Transition from Globe to Planar when zooming in <= threshold altitude (MSL)
            let auto_switch_threshold = engine.auto_switch_altitude.unwrap_or(0.0) as f32;
            if auto_switch_threshold > 0.0 && new_alt <= auto_switch_threshold && scroll_delta > 0.0 {
                let landing_geo = if let Some(hover_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    let local_screen = hover_pos - rect.min;
                    if local_screen.x >= 0.0 && local_screen.x <= vp_size.x && local_screen.y >= 0.0 && local_screen.y <= vp_size.y {
                        let ray = engine.screen_to_ray(local_screen.x, local_screen.y, vp_size.x, vp_size.y);
                        if let Some(hit) = ray.intersect_sphere(glam::Vec3::ZERO, WGS84_RADIUS) {
                            ecef_to_geodetic(hit)
                        } else {
                            let eye = engine.camera.eye_position();
                            ecef_to_geodetic(eye.normalize_or_zero() * WGS84_RADIUS)
                        }
                    } else {
                        let eye = engine.camera.eye_position();
                        ecef_to_geodetic(eye.normalize_or_zero() * WGS84_RADIUS)
                    }
                } else {
                    let eye = engine.camera.eye_position();
                    ecef_to_geodetic(eye.normalize_or_zero() * WGS84_RADIUS)
                };

                // Auto-switching to planar lands with Heading 0.0° (North) and Tilt 45.0° (oblique 3D perspective)
                engine.transition_to_planar_at_geo_with_pose(
                    landing_geo.latitude,
                    landing_geo.longitude,
                    new_alt,
                    0.0,
                    45.0,
                );
            } else {
                engine.camera.zoom_globe(scroll_delta);
            }
        } else if let Some(hover_pos) = ctx.input(|i| i.pointer.hover_pos()) {
            let local_screen = hover_pos - rect.min;
            if local_screen.x >= 0.0 && local_screen.x <= vp_size.x && local_screen.y >= 0.0 && local_screen.y <= vp_size.y {
                let ray_before = engine.screen_to_ray(local_screen.x, local_screen.y, vp_size.x, vp_size.y);

                let now = web_time::Instant::now();
                let anchor_pt = if let Some(za) = engine.zoom_anchor {
                    if now.duration_since(za.timestamp).as_millis() < 400
                        && (hover_pos - za.start_pos).length() < 12.0
                    {
                        za.anchor_world
                    } else {
                        let pt = engine.intersect_scene_or_terrain(&ray_before).unwrap_or_else(|| {
                            ray_before.intersect_ground_plane(0.0).unwrap_or(engine.camera.target)
                        });
                        engine.zoom_anchor = Some(ZoomAnchor {
                            anchor_world: pt,
                            start_pos: hover_pos,
                            timestamp: now,
                        });
                        pt
                    }
                } else {
                    let pt = engine.intersect_scene_or_terrain(&ray_before).unwrap_or_else(|| {
                        ray_before.intersect_ground_plane(0.0).unwrap_or(engine.camera.target)
                    });
                    engine.zoom_anchor = Some(ZoomAnchor {
                        anchor_world: pt,
                        start_pos: hover_pos,
                        timestamp: now,
                    });
                    pt
                };
                if let Some(za) = &mut engine.zoom_anchor {
                    za.timestamp = now;
                }

                let steps = (scroll_delta / 50.0).clamp(-4.0, 4.0);
                let factor = 0.74f32.powf(steps);

                let current_td = engine.camera.target_distance;
                let new_camera_dist = (current_td * factor).clamp(0.5, 80_000_000.0);
                let scale_ratio = new_camera_dist / current_td;

                let current_tl = engine.camera.target_lookat;
                let new_target = anchor_pt + (current_tl - anchor_pt) * scale_ratio;

                engine.camera.target_lookat = new_target;
                engine.camera.target_distance = new_camera_dist;
                engine.camera.target = new_target;
                engine.camera.distance = new_camera_dist;

                // Smoothly un-tilt and un-rotate towards Top-Down / North-Up when zooming out above 3km
                if scroll_delta < 0.0 && engine.camera.target_distance > 3_000.0 {
                    let unlevel_t = ((engine.camera.target_distance - 3_000.0) / 35_000.0).clamp(0.0, 1.0);
                    let blend = (0.25 * unlevel_t).clamp(0.05, 0.35);
                    engine.camera.pitch = engine.camera.pitch * (1.0 - blend) + 1.54 * blend;

                    let mut d_yaw = 0.0 - engine.camera.yaw;
                    while d_yaw > std::f32::consts::PI { d_yaw -= std::f32::consts::TAU; }
                    while d_yaw < -std::f32::consts::PI { d_yaw += std::f32::consts::TAU; }
                    engine.camera.yaw += d_yaw * blend;
                    engine.camera.normalize_yaw();
                    engine.camera.target_pitch = engine.camera.pitch;
                    engine.camera.target_yaw = engine.camera.yaw;
                }

                // Auto-transition to 3D Globe when zooming out past threshold (+10% hysteresis deadband)
                let auto_switch_threshold = engine.auto_switch_altitude.unwrap_or(0.0) as f32;
                if auto_switch_threshold > 0.0 && engine.camera.target_distance > auto_switch_threshold * 1.1 && scroll_delta < 0.0 {
                    engine.transition_to_globe();
                }
            } else {
                engine.camera.zoom_planar(scroll_delta);
                let auto_switch_threshold = engine.auto_switch_altitude.unwrap_or(0.0) as f32;
                if auto_switch_threshold > 0.0 && engine.camera.target_distance > auto_switch_threshold * 1.1 && scroll_delta < 0.0 {
                    engine.transition_to_globe();
                }
            }
        } else {
            engine.camera.zoom_planar(scroll_delta);
            let auto_switch_threshold = engine.auto_switch_altitude.unwrap_or(0.0) as f32;
            if auto_switch_threshold > 0.0 && engine.camera.target_distance > auto_switch_threshold * 1.1 && scroll_delta < 0.0 {
                engine.transition_to_globe();
            }
        }

        true
    }

    /// Resets transient interaction states when pointer is released
    pub fn handle_pointer_release(engine: &mut MapEngine, ctx: &egui::Context) -> bool {
        if ctx.input(|i| i.pointer.any_released()) {
            engine.pan_state = None;
            engine.orbit_state = None;
        }
        false
    }
}
