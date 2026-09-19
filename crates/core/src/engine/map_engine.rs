use glam::Vec3;
use crate::gis::basemap::BasemapManager;
use crate::gis::cache::ResourceBudget;
use crate::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use crate::gis::geojson_loader::GisFeature;
use crate::gis::layer::{Layer, LayerRegistry, LayerType};
use crate::gis::terrain::TerrainManager;
use crate::gis::threedtiles::Tiles3DManager;
use crate::gis::i3s::I3SManager;
use crate::renderer::camera::{Camera, GlobeFlightState, GoToTarget, IntoGoToOptions};
use crate::renderer::render_engine::RenderEngine;
use crate::scene::scene::Scene;
use crate::gis::source::SourceRegistry;
use crate::solar::datetime_state::SolarDateTimeState;
use crate::solar::shadow_analysis::SceneCollider;
use crate::solar::sun_calc::{calculate_solar_position, SolarPosition};
use crate::spatial::picking::{screen_to_ray, Ray};
use crate::engine::command::{
    BasemapCommand, CameraCommand, ClockCommand, CommandError, EdgeCommand,
    EnvironmentCommand, I3SCommand, LayerCommand, MapCommand, TerrainCommand,
};
use crate::engine::event::MapEvent;
use crate::engine::view::MapView;

/// Core 3D GIS & Map Engine
///
/// Encapsulates 3D GPU rendering, tile streaming (basemaps, terrain, 3D tiles),
/// vector layers, solar lighting/shadow analysis, camera navigation, and spatial picking.
///
/// Headless and decoupled from any UI framework.
pub struct MapEngine {
    // Renderer
    pub renderer: Option<RenderEngine>,

    // Scene & Coordinates
    pub scene: Scene,
    pub camera: Camera,
    pub projection_mode: ProjectionMode,

    // GIS Streaming Data Sources
    pub sources: SourceRegistry,
    pub basemap: BasemapManager,
    pub terrain: TerrainManager,
    pub i3s: I3SManager,
    pub threedtiles: Tiles3DManager,
    pub layers: Vec<Layer>,
    pub layer_registry: LayerRegistry,
    pub budget: ResourceBudget,
    pub collider: SceneCollider,

    // Lighting & Environment
    pub solar_dt: SolarDateTimeState,
    pub solar_pos: SolarPosition,
    pub sunlight_enabled: bool,
    pub sun_intensity: f32,
    pub ambient_intensity: f32,

    // Selection & State
    pub selected_feature: Option<GisFeature>,
    pub selected_layer_idx: Option<usize>,
    pub has_new_gpu_tiles: bool,
    pub gpu_meshes_dirty: bool,

    // Navigation Interaction States
    pub pan_state: Option<crate::engine::navigation::PanState>,
    pub orbit_state: Option<crate::engine::navigation::OrbitState>,
    pub zoom_anchor: Option<crate::engine::navigation::ZoomAnchor>,
    pub mouse_drag_distance: f32,

    // Active Globe Flight Animation
    pub active_globe_flight: Option<GlobeFlightState>,

    // Status & Event Queue
    pub status_message: String,
    pub events: Vec<MapEvent>,
}

impl Default for MapEngine {
    fn default() -> Self {
        // Default Melbourne CBD origin
        let origin = ProjectOrigin::from_geo(GeoCoord {
            latitude: -37.8136,
            longitude: 144.9631,
            elevation: 0.0,
        });
        Self::new(origin)
    }
}

impl MapEngine {
    /// Creates a new MapEngine initialized at the given geographic origin
    pub fn new(origin: ProjectOrigin) -> Self {
        let solar_dt = SolarDateTimeState::default();
        let utc_dt = solar_dt.to_utc_datetime();
        let solar_pos = calculate_solar_position(
            origin.origin.latitude,
            origin.origin.longitude,
            &utc_dt,
            solar_dt.timezone_offset_hours,
        );

        Self {
            renderer: None,
            scene: Scene::new(origin),
            camera: Camera::default(),
            projection_mode: ProjectionMode::PlanarENU,

            sources: SourceRegistry::new(),
            basemap: BasemapManager::new(),
            terrain: TerrainManager::new(),
            i3s: I3SManager::new(origin),
            threedtiles: Tiles3DManager::new(),
            layers: Vec::new(),
            layer_registry: LayerRegistry::default(),
            budget: ResourceBudget::default(),
            collider: SceneCollider::default(),

            solar_dt,
            solar_pos,
            sunlight_enabled: true,
            sun_intensity: 1.2,
            ambient_intensity: 0.75,

            selected_feature: None,
            selected_layer_idx: None,
            has_new_gpu_tiles: false,
            gpu_meshes_dirty: false,

            pan_state: None,
            orbit_state: None,
            zoom_anchor: None,
            mouse_drag_distance: 0.0,

            active_globe_flight: None,
            status_message: String::new(),
            events: Vec::new(),
        }
    }

    /// Attach a pre-configured RenderEngine
    pub fn set_renderer(&mut self, renderer: RenderEngine) {
        self.renderer = Some(renderer);
        self.reload_all_gpu_meshes();
    }

    /// Changes the project origin (local coordinate system center) and updates GIS caches
    pub fn set_origin(&mut self, origin: ProjectOrigin) {
        self.scene.origin = origin;
        self.i3s.set_origin(origin);
        self.basemap.reset_cache();
        if let Some(r) = &mut self.renderer {
            r.clear_basemap_tiles();
        }
        self.reload_all_gpu_meshes();
        self.events.push(MapEvent::OriginChanged(origin));
    }

    /// Returns an immutable read view facade over MapEngine state
    pub fn view(&self) -> MapView<'_> {
        let edge_cfg = self.renderer.as_ref().map(|r| &r.edge_renderer.config);
        MapView {
            origin: &self.scene.origin,
            camera: &self.camera,
            layers: &self.layers,
            layer_registry: &self.layer_registry,
            sources: &self.sources,
            budget: &self.budget,
            basemap: &self.basemap,
            terrain: &self.terrain,
            i3s: &self.i3s,
            solar_pos: &self.solar_pos,
            solar_dt: &self.solar_dt,
            sunlight_enabled: self.sunlight_enabled,
            sun_intensity: self.sun_intensity,
            ambient_intensity: self.ambient_intensity,
            edge_config: edge_cfg,
            status_message: &self.status_message,
        }
    }

    /// Recomputes solar position based on geographic origin and solar datetime
    pub fn update_solar_position(&mut self) {
        let utc_dt = self.solar_dt.to_utc_datetime();
        self.solar_pos = calculate_solar_position(
            self.scene.origin.origin.latitude,
            self.scene.origin.origin.longitude,
            &utc_dt,
            self.solar_dt.timezone_offset_hours,
        );
    }

    /// Returns true if any background streaming pipeline is actively downloading tiles/data
    pub fn is_streaming(&self) -> bool {
        self.basemap.is_streaming()
            || self.terrain.is_streaming()
            || self.threedtiles.is_streaming()
            || self.i3s.is_streaming()
    }

    /// Per-frame update: advances flight transitions, updates streaming data, syncs solar position
    pub fn update(&mut self, dt: f32) {
        // 1. Advance camera smooth damping
        if self.camera.update_smooth_zoom(dt) {
            self.events.push(MapEvent::CameraMoved);
        }

        // 2. Advance globe flights if active
        if self.update_globe_flight(dt) {
            self.events.push(MapEvent::CameraMoved);
        }

        // 3. Update streaming tiles (basemap, terrain, 3d tiles)
        self.update_streaming();

        // 4. Automatic GPU mesh synchronization
        if self.gpu_meshes_dirty {
            self.reload_all_gpu_meshes();
            self.gpu_meshes_dirty = false;
        }
    }

    /// Updates Basemap, 3D Terrain, 3D Tiles, and I3S streaming
    pub fn update_streaming(&mut self) {
        let (vp_w, vp_h) = if let Some(renderer) = &self.renderer {
            (renderer.current_width as f32, renderer.current_height as f32)
        } else {
            (1280.0, 720.0)
        };

        if self.basemap.is_enabled || self.terrain.is_enabled {
            let active_tiles = if self.projection_mode == ProjectionMode::GlobeECEF {
                self.basemap.calculate_globe_camera_tiles(&self.camera, vp_w, vp_h)
            } else {
                let center_geo = self.scene.origin.local_to_geo(self.camera.target);
                let dyn_zoom = crate::gis::basemap::BasemapManager::calculate_camera_lod_zoom_with_hysteresis(
                    self.camera.distance,
                    self.camera.fov_y,
                    vp_h,
                    center_geo.latitude,
                    self.basemap.zoom,
                );
                self.basemap.zoom = dyn_zoom;
                self.basemap.calculate_camera_tiles(
                    &self.scene.origin,
                    &self.camera,
                    vp_w,
                    vp_h,
                )
            };
            self.basemap.target_active_count = active_tiles.len();
            self.basemap.previous_active_tiles = active_tiles.iter().copied().collect();

            let mut new_tiles = Vec::new();
            if self.basemap.is_enabled {
                self.basemap.request_tiles(&active_tiles);
                new_tiles = self.basemap.drain_completed_tiles();
            } else if self.terrain.is_enabled {
                if let Some(renderer) = &self.renderer {
                    for &coord in &active_tiles {
                        if !renderer.gpu_tiles.contains_key(&coord) {
                            new_tiles.push(crate::gis::basemap::DecodedTile::dummy(coord));
                        }
                    }
                }
            }

            if !new_tiles.is_empty() {
                self.has_new_gpu_tiles = true;
            }

            if self.terrain.is_enabled {
                self.terrain.request_tiles(&active_tiles);
            }
            let new_terrain_tiles = if self.terrain.is_enabled {
                self.terrain.drain_completed()
            } else {
                Vec::new()
            };

            if !new_terrain_tiles.is_empty() {
                self.has_new_gpu_tiles = true;
            }

            if let Some(renderer) = &mut self.renderer {
                let grid_mode = if self.basemap.is_enabled { 0.0 } else { 1.0 };
                renderer.update_basemap_uniforms(self.basemap.opacity, grid_mode, self.basemap.show_debug_borders);

                for tile in new_tiles {
                    let terrain_tile = if self.terrain.is_enabled {
                        self.terrain.get_terrain_for_tile(tile.coord)
                    } else {
                        None
                    };
                    let evicted = renderer.add_tile(
                        tile,
                        &self.scene.origin,
                        self.basemap.opacity,
                        Some(&self.terrain),
                        terrain_tile.as_ref(),
                        self.terrain.height_exaggeration,
                    );
                    for ev in evicted {
                        self.basemap.unmark_requested(ev);
                    }
                }

                for terrain_tile in &new_terrain_tiles {
                    let affected_coords: Vec<crate::gis::basemap::TileCoord> = renderer
                        .gpu_tiles
                        .keys()
                        .copied()
                        .filter(|c| c == &terrain_tile.coord || c.is_descendant_of(&terrain_tile.coord) || terrain_tile.coord.is_descendant_of(c))
                        .collect();
                    for aff in affected_coords {
                        let aff_terrain = self.terrain.get_terrain_for_tile(aff);
                        renderer.update_tile_terrain_mesh(
                            aff,
                            &self.scene.origin,
                            Some(&self.terrain),
                            aff_terrain.as_ref(),
                            self.terrain.height_exaggeration,
                        );
                    }
                }

                let idle_evicted = renderer.prune_unneeded_tiles(&active_tiles);
                for ev in idle_evicted {
                    self.basemap.unmark_requested(ev);
                }
            }
        } else if let Some(renderer) = &mut self.renderer {
            if !renderer.gpu_tiles.is_empty() {
                renderer.clear_basemap_tiles();
            }
        }

        // I3S
        if self.i3s.is_enabled {
            let visible_nodes = self.i3s.calculate_visible_nodes(&self.camera, vp_w, vp_h, &self.scene.origin);
            self.i3s.request_nodes(&visible_nodes);
            let new_i3s_nodes = self.i3s.drain_completed();
            if !new_i3s_nodes.is_empty() {
                self.has_new_gpu_tiles = true;
            }
            if let Some(renderer) = &mut self.renderer {
                for node in new_i3s_nodes {
                    for f in &node.features {
                        self.collider.add_mesh(&f.raw_mesh, &format!("i3s_feat_{}_{}", node.node_id, f.feature_id));
                    }
                    renderer.add_i3s_tile(node, [1.0, 1.0, 1.0], self.i3s.opacity, true);
                }
                let evicted_i3s = renderer.prune_unneeded_i3s_tiles(&visible_nodes);
                for ev in evicted_i3s {
                    self.i3s.unmark_loaded(ev);
                    self.collider.remove_features_with_prefix(&format!("i3s_feat_{}_", ev));
                }
            }
        }

        // 3D Tiles
        if self.threedtiles.is_enabled {
            let (active_3d_tiles, newly_loaded) = self.threedtiles.update_streaming(
                &self.scene.origin,
                &self.camera,
                vp_w,
                vp_h,
                self.projection_mode,
            );
            if !newly_loaded.is_empty() {
                self.has_new_gpu_tiles = true;
            }
            if let Some(renderer) = &mut self.renderer {
                let tint = self.threedtiles.tint;
                let opacity = self.threedtiles.opacity;

                for mesh in &newly_loaded {
                    let feat_id = format!("threedtile_{}", mesh.id);
                    self.collider.remove_features_with_prefix(&feat_id);
                    self.collider.add_threedtile_mesh(mesh, &feat_id);
                    renderer.add_threedtile(mesh.clone(), tint, opacity, self.threedtiles.replace_texture);
                }
                for tile_id in &active_3d_tiles {
                    let feat_id = format!("threedtile_{}", tile_id);
                    if !renderer.threedtiles_gpu_tiles.contains_key(tile_id) {
                        if let Some(mesh) = self.threedtiles.loaded_tiles.get(tile_id) {
                            self.collider.remove_features_with_prefix(&feat_id);
                            self.collider.add_threedtile_mesh(mesh, &feat_id);
                            renderer.add_threedtile(mesh.clone(), tint, opacity, self.threedtiles.replace_texture);
                        }
                    }
                }
                let evicted = renderer.prune_unneeded_threedtiles(&active_3d_tiles);
                for ev in evicted {
                    self.collider.remove_features_with_prefix(&format!("threedtile_{}", ev));
                }
            }
        }
    }

    /// Updates active globe flight transition animation
    pub fn update_globe_flight(&mut self, dt: f32) -> bool {
        const WGS84_RADIUS: f32 = crate::gis::crs::WGS84_A as f32;

        let mut flight = match self.active_globe_flight.take() {
            Some(f) => f,
            None => return false,
        };

        let mut continue_anim = true;

        match &mut flight.phase {
            crate::renderer::camera::FlightPhase::Spinning {
                start_pitch,
                start_yaw,
                target_pitch,
                target_yaw_diff,
                start_dist: _,
                elapsed,
                duration,
            } => {
                *elapsed += dt;
                let t = (*elapsed / *duration).clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t);

                self.camera.pitch = *start_pitch + (*target_pitch - *start_pitch) * s;
                self.camera.yaw = *start_yaw + *target_yaw_diff * s;
                self.camera.normalize_yaw();
                self.camera.target = glam::Vec3::ZERO;
                self.camera.target_lookat = glam::Vec3::ZERO;
                self.camera.target_distance = self.camera.distance;

                if t >= 1.0 {
                    self.camera.snap_smoothing();
                    let cur_dist = self.camera.distance;
                    let target_end_dist = WGS84_RADIUS + 300_000.0 * 0.95;
                    flight.phase = crate::renderer::camera::FlightPhase::Descent {
                        start_dist: cur_dist,
                        end_dist: target_end_dist,
                        elapsed: 0.0,
                        duration: 0.85,
                    };
                }
            }
            crate::renderer::camera::FlightPhase::Descent {
                start_dist,
                end_dist,
                elapsed,
                duration,
            } => {
                *elapsed += dt;
                let t = (*elapsed / *duration).clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t);

                let start_alt = (*start_dist - WGS84_RADIUS).max(100.0);
                let end_alt = (*end_dist - WGS84_RADIUS).max(100.0);
                let cur_alt = (start_alt.ln() + (end_alt.ln() - start_alt.ln()) * s).exp();
                self.camera.distance = WGS84_RADIUS + cur_alt;
                self.camera.target_distance = self.camera.distance;
                self.camera.target = glam::Vec3::ZERO;
                self.camera.target_lookat = glam::Vec3::ZERO;

                if t >= 1.0 {
                    self.camera.snap_smoothing();
                    let target_geo = flight.target_geo;
                    let target_planar_dist = flight.target_planar_dist;
                    self.transition_to_planar_at_geo(target_geo.latitude, target_geo.longitude, 20_000.0);

                    flight.phase = crate::renderer::camera::FlightPhase::PlanarLanding {
                        start_dist: 20_000.0,
                        target_dist: target_planar_dist,
                        start_pitch: 1.54,
                        target_pitch: 0.55,
                        start_yaw: 0.0,
                        target_yaw: 0.78,
                        elapsed: 0.0,
                        duration: 0.85,
                    };
                }
            }
            crate::renderer::camera::FlightPhase::PlanarLanding {
                start_dist,
                target_dist,
                start_pitch,
                target_pitch,
                start_yaw,
                target_yaw,
                elapsed,
                duration,
            } => {
                *elapsed += dt;
                let t = (*elapsed / *duration).clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t);

                let cur_dist = (start_dist.ln() + (target_dist.ln() - start_dist.ln()) * s).exp();
                self.camera.distance = cur_dist;
                self.camera.target_distance = cur_dist;
                self.camera.target_lookat = self.camera.target;
                self.camera.pitch = *start_pitch + (*target_pitch - *start_pitch) * s;
                self.camera.yaw = *start_yaw + (*target_yaw - *start_yaw) * s;
                self.camera.normalize_yaw();

                if t >= 1.0 {
                    self.camera.snap_smoothing();
                    continue_anim = false;
                }
            }
        }

        if continue_anim {
            self.active_globe_flight = Some(flight);
        }
        true
    }

    pub fn transition_to_planar_at_geo(&mut self, lat: f64, lon: f64, target_distance: f32) {
        if self.projection_mode == ProjectionMode::PlanarENU {
            return;
        }
        self.projection_mode = ProjectionMode::PlanarENU;

        let local_landing = self.scene.origin.lat_lon_to_local(lat, lon, 0.0);
        if local_landing.x.abs() < 50_000.0 && local_landing.z.abs() < 50_000.0 {
            self.camera.target = glam::Vec3::new(local_landing.x, 0.0, local_landing.z);
            self.camera.target_lookat = self.camera.target;
        } else {
            self.scene.origin = ProjectOrigin::from_geo(GeoCoord {
                latitude: lat,
                longitude: lon,
                elevation: 0.0,
            });
            self.camera.target = glam::Vec3::ZERO;
            self.camera.target_lookat = glam::Vec3::ZERO;
        }

        if let Some(r) = &mut self.renderer {
            r.projection_mode = ProjectionMode::PlanarENU;
            r.morph_progress = 0.0;
            r.clear_basemap_tiles();
            r.clear_i3s_tiles();
            r.clear_threedtiles();
        }
        self.basemap.reset_cache();
        self.terrain.clear_cache();
        self.i3s.clear_streaming_state();
        self.i3s.set_origin(self.scene.origin);

        self.camera.distance = target_distance.clamp(100.0, 50_000.0);
        self.camera.target_distance = self.camera.distance;
        self.camera.pitch = 1.54;
        self.camera.yaw = 0.0;
        self.camera.snap_smoothing();
        self.reload_all_gpu_meshes();
    }

    pub fn transition_to_globe(&mut self) {
        if self.projection_mode == ProjectionMode::GlobeECEF {
            return;
        }

        let current_geo = self.scene.origin.local_to_geo(self.camera.target);
        self.projection_mode = ProjectionMode::GlobeECEF;

        if let Some(renderer) = &mut self.renderer {
            renderer.projection_mode = ProjectionMode::GlobeECEF;
            renderer.morph_progress = 1.0;
            renderer.clear_basemap_tiles();
            renderer.clear_i3s_tiles();
            renderer.clear_threedtiles();
            renderer.clear_meshes();
        }
        self.basemap.reset_cache();
        self.terrain.clear_cache();
        self.i3s.clear_streaming_state();

        const WGS84_RADIUS: f32 = crate::gis::crs::WGS84_A as f32;
        self.camera.target = glam::Vec3::ZERO;
        self.camera.target_lookat = glam::Vec3::ZERO;
        self.camera.distance = WGS84_RADIUS + 50_000.0 * 1.05;
        self.camera.target_distance = self.camera.distance;

        let (pitch, yaw) = crate::gis::crs::geo_to_globe_camera_angles(&current_geo);
        self.camera.pitch = pitch;
        self.camera.yaw = yaw;
        self.camera.normalize_yaw();
        self.camera.snap_smoothing();
        self.reload_all_gpu_meshes();
    }

    /// Applies a MapCommand to mutate map state
    pub fn apply(&mut self, cmd: MapCommand) -> Result<(), CommandError> {
        match cmd {
            MapCommand::Camera(cam_cmd) => match cam_cmd {
                CameraCommand::Pan { delta_x, delta_y } => {
                    self.pan(delta_x, delta_y);
                }
                CameraCommand::Orbit { delta_yaw, delta_pitch } => {
                    self.orbit(delta_yaw, delta_pitch);
                }
                CameraCommand::Zoom(delta) => {
                    self.zoom(delta);
                }
                CameraCommand::ZoomScale(scale) => {
                    self.zoom_scale(scale);
                }
                CameraCommand::ViewTop => {
                    self.view_top();
                }
                CameraCommand::ViewPerspective => {
                    self.view_perspective();
                }
                CameraCommand::AlignNorth => {
                    self.align_north();
                }
                CameraCommand::RotateYaw(angle) => {
                    self.camera.yaw += angle;
                    self.events.push(MapEvent::CameraMoved);
                }
                CameraCommand::ResetView => {
                    self.reset_view();
                }
                CameraCommand::FlyTo { target_geo, distance } => {
                    self.fly_to_geo(target_geo, distance);
                }
                CameraCommand::LookAt { target, distance, pitch, yaw } => {
                    self.look_at(target, distance, pitch, yaw);
                }
                CameraCommand::GoTo { target, options } => {
                    self.goto(target, options);
                }
            },
            MapCommand::Layer(layer_cmd) => match layer_cmd {
                LayerCommand::SetVisibility { id, visible } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                        l.visible = visible;
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::SetOpacity { id, opacity } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                        l.opacity = opacity.clamp(0.0, 1.0);
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::SetColorTint { id, tint } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                        l.color_tint = tint;
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::SetShadow { id, cast_shadows } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                        l.cast_shadows = cast_shadows;
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::Remove(id) => {
                    if let Some(pos) = self.layers.iter().position(|l| l.id == id) {
                        self.layers.remove(pos);
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::Add(layer) => {
                    self.layers.push(*layer);
                    self.reload_all_gpu_meshes();
                }
                LayerCommand::AddDescriptor(_) => {}
            },
            MapCommand::Basemap(bm_cmd) => match bm_cmd {
                BasemapCommand::SetProvider(provider) => {
                    self.basemap.provider = provider;
                    self.basemap.is_enabled = true;
                    self.events.push(MapEvent::BasemapProviderChanged(provider));
                }
                BasemapCommand::SetEnabled(enabled) => {
                    self.basemap.is_enabled = enabled;
                }
                BasemapCommand::ResetCache => {
                    self.basemap.reset_cache();
                    if let Some(r) = &mut self.renderer {
                        r.gpu_tiles.clear();
                    }
                }
            },
            MapCommand::Terrain(t_cmd) => match t_cmd {
                TerrainCommand::SetEnabled(enabled) => {
                    self.terrain.is_enabled = enabled;
                    self.events.push(MapEvent::TerrainToggled(enabled));
                }
                TerrainCommand::SetHeightExaggeration(exagg) => {
                    self.terrain.height_exaggeration = exagg;
                }
            },
            MapCommand::I3S(i3s_cmd) => match i3s_cmd {
                I3SCommand::SetEnabled(enabled) => {
                    self.i3s.is_enabled = enabled;
                }
                I3SCommand::SetServiceUrl(url) => {
                    self.i3s.service_url = url;
                }
                I3SCommand::SetOpacity(opacity) => {
                    self.i3s.opacity = opacity;
                }
                I3SCommand::SetLodThresholdScale(scale) => {
                    self.i3s.lod_threshold_scale = scale;
                }
            },
            MapCommand::Clock(clock_cmd) => match clock_cmd {
                ClockCommand::SetTime { hour, minute } => {
                    self.solar_dt.hour = hour;
                    self.solar_dt.minute = minute;
                    self.update_solar_position();
                }
                ClockCommand::SetDate { month, day } => {
                    self.solar_dt.month = month;
                    self.solar_dt.day = day;
                    self.update_solar_position();
                }
                ClockCommand::SetDateTime(dt) => {
                    self.solar_dt = dt;
                    self.update_solar_position();
                }
            },
            MapCommand::Environment(env_cmd) => match env_cmd {
                EnvironmentCommand::SetSunlightEnabled(enabled) => {
                    self.sunlight_enabled = enabled;
                }
                EnvironmentCommand::SetSunIntensity(intensity) => {
                    self.sun_intensity = intensity;
                }
                EnvironmentCommand::SetAmbientIntensity(intensity) => {
                    self.ambient_intensity = intensity;
                }
            },
            MapCommand::Edge(edge_cmd) => match edge_cmd {
                EdgeCommand::SetEnabled(enabled) => {
                    if let Some(r) = &mut self.renderer {
                        r.edge_renderer.config.enabled = enabled;
                    }
                }
                EdgeCommand::SetWidth(w) => {
                    if let Some(r) = &mut self.renderer {
                        r.edge_renderer.config.width = w;
                    }
                }
                EdgeCommand::SetColor(col) => {
                    if let Some(r) = &mut self.renderer {
                        r.edge_renderer.config.color = col;
                    }
                }
                EdgeCommand::SetDepthThreshold(d) => {
                    if let Some(r) = &mut self.renderer {
                        r.edge_renderer.config.depth_threshold = d;
                    }
                }
                EdgeCommand::SetNormalThreshold(n) => {
                    if let Some(r) = &mut self.renderer {
                        r.edge_renderer.config.normal_threshold = n;
                    }
                }
            },
            MapCommand::SetStatusMessage(msg) => {
                self.status_message = msg.clone();
                self.events.push(MapEvent::StatusChanged(msg));
            }
        }
        Ok(())
    }

    // --- Navigation Conveniences ---

    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        self.camera.pan(delta_x, delta_y);
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.camera.orbit(delta_yaw, delta_pitch);
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.camera.zoom(delta);
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn zoom_scale(&mut self, scale: f32) {
        self.camera.distance = (self.camera.distance * scale).clamp(2.0, 80_000_000.0);
        self.camera.target_distance = self.camera.distance;
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn look_at(&mut self, target: Vec3, distance: f32, pitch: f32, yaw: f32) {
        self.camera.target = target;
        self.camera.target_lookat = target;
        self.camera.distance = distance;
        self.camera.target_distance = distance;
        self.camera.pitch = pitch;
        self.camera.target_pitch = pitch;
        self.camera.yaw = yaw;
        self.camera.target_yaw = yaw;
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn fly_to_geo(&mut self, target_geo: GeoCoord, distance: f32) {
        self.camera.zoom_to_geo(&self.scene.origin, target_geo.latitude, target_geo.longitude, 0.0, distance);
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn goto<T: IntoGoToOptions>(&mut self, target: impl Into<GoToTarget>, options: T) {
        self.camera.goto(&self.scene.origin, self.projection_mode, target.into(), options.into_goto_options());
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn view_top(&mut self) {
        self.camera.pitch = 0.01;
        self.camera.yaw = 0.0;
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn view_perspective(&mut self) {
        self.camera.pitch = 0.75;
        self.camera.yaw = -0.785;
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn align_north(&mut self) {
        self.camera.yaw = 0.0;
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn reset_view(&mut self) {
        self.camera = Camera::default();
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn set_projection_mode(&mut self, mode: ProjectionMode) {
        self.projection_mode = mode;
        if let Some(r) = &mut self.renderer {
            r.projection_mode = mode;
            r.morph_progress = if mode == ProjectionMode::GlobeECEF { 1.0 } else { 0.0 };
        }
    }

    // --- GIS Layers & Meshes ---

    pub fn add_layer(&mut self, layer: Layer) -> usize {
        self.layers.push(layer);
        let idx = self.layers.len() - 1;
        self.reload_all_gpu_meshes();
        idx
    }

    pub fn remove_layer(&mut self, id: &str) -> bool {
        if let Some(pos) = self.layers.iter().position(|l| l.id == id) {
            self.layers.remove(pos);
            self.reload_all_gpu_meshes();
            true
        } else {
            false
        }
    }

    pub fn load_geojson(&mut self, geojson_str: &str, layer_name: &str) -> Result<usize, String> {
        let dataset = crate::gis::geojson_loader::parse_geojson(geojson_str, Some(self.scene.origin))?;
        let mut layer = Layer::new(
            format!("layer_{}", self.layers.len()),
            layer_name.to_string(),
            LayerType::Buildings,
            [0.85, 0.88, 0.92, 1.0],
        );
        layer.features = dataset.features;
        self.layers.push(layer);
        let idx = self.layers.len() - 1;
        self.reload_all_gpu_meshes();
        Ok(idx)
    }

    /// Loads the builtin Melbourne CBD sample buildings dataset
    pub fn load_sample_buildings(&mut self) {
        let sample_geojson = include_str!("../../assets/sample_buildings.geojson");
        if let Ok(ds) = crate::gis::geojson_loader::parse_geojson(sample_geojson, Some(self.scene.origin)) {
            let mut lyr = Layer::new(
                "layer_melbourne_cbd_default".to_string(),
                "Melbourne CBD Buildings".to_string(),
                LayerType::Buildings,
                [0.85, 0.88, 0.92, 1.0],
            );
            lyr.features = ds.features;
            if let Some(pos) = self.layers.iter().position(|l| l.id == "layer_melbourne_cbd_default") {
                self.layers[pos] = lyr;
            } else {
                self.layers.insert(0, lyr);
            }
            self.selected_layer_idx = Some(0);
            self.reload_all_gpu_meshes();
        }
    }

    /// Rebuilds extruded 3D meshes for a specific layer if missing
    pub fn rebuild_layer_feature_meshes(&mut self, layer_idx: usize) {
        if layer_idx >= self.layers.len() {
            return;
        }
        let origin = self.scene.origin;
        for feature in &mut self.layers[layer_idx].features {
            if feature.mesh.is_none() && !feature.geo_polygons.is_empty() {
                let mut combined_mesh = crate::gis::extrusion::RawMeshData::new();
                for poly in &feature.geo_polygons {
                    let rings_local: Vec<Vec<glam::Vec2>> = poly.iter().map(|ring| {
                        ring.iter().map(|&[lat, lon]| {
                            let loc = origin.lat_lon_to_local(lat, lon, 0.0);
                            glam::Vec2::new(loc.x, -loc.z)
                        }).collect()
                    }).collect();
                    if let Some(poly_mesh) = crate::gis::extrusion::extrude_polygon(&rings_local, feature.min_height, (feature.height - feature.min_height).max(1.0)) {
                        let v_offset = combined_mesh.positions.len() as u32;
                        combined_mesh.positions.extend(poly_mesh.positions);
                        combined_mesh.normals.extend(poly_mesh.normals);
                        combined_mesh.uvs.extend(poly_mesh.uvs);
                        for idx in poly_mesh.indices {
                            combined_mesh.indices.push(v_offset + idx);
                        }
                    }
                }
                if !combined_mesh.positions.is_empty() {
                    feature.mesh = Some(combined_mesh);
                }
            }
        }
    }

    /// Re-uploads and syncs all layer feature meshes and colliders to the 3D renderer
    pub fn reload_all_gpu_meshes(&mut self) {
        self.scene.nodes.clear();
        self.collider.clear();

        for i in 0..self.layers.len() {
            let needs_rebuild = self.layers[i].features.iter().any(|f| {
                !f.geo_polygons.is_empty() && f.mesh.is_none()
            });
            if needs_rebuild {
                self.rebuild_layer_feature_meshes(i);
            }
        }

        // 1. Populate SceneCollider and SceneNodes for spatial queries and picking
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            for feature in &layer.features {
                if let Some(mesh) = &feature.mesh {
                    self.collider.add_mesh(mesh, &feature.id);

                    let mut aabb_min = Vec3::splat(f32::INFINITY);
                    let mut aabb_max = Vec3::splat(f32::NEG_INFINITY);
                    for p in &mesh.positions {
                        let v = Vec3::from_array(*p);
                        aabb_min = aabb_min.min(v);
                        aabb_max = aabb_max.max(v);
                    }

                    let mut node = crate::scene::node::SceneNode::new(
                        feature.id.clone(),
                        feature.name.clone(),
                        layer.id.clone(),
                        Some(feature.id.clone()),
                        0,
                        aabb_min,
                        aabb_max,
                    );
                    node.is_visible = layer.visible;
                    node.shadow_color = feature.shadow_color;
                    self.scene.nodes.push(node);
                }
            }
        }

        // 2. Upload to GPU RenderEngine if present
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_meshes();
            renderer.clear_custom_shadow_colors();

            for layer in &self.layers {
                if !layer.visible {
                    continue;
                }
                let mut raw_meshes_casters = Vec::new();
                let mut raw_meshes_ground = Vec::new();

                for feature in &layer.features {
                    if let Some(mesh) = &feature.mesh {
                        let mut aabb_min = Vec3::splat(f32::INFINITY);
                        let mut aabb_max = Vec3::splat(f32::NEG_INFINITY);
                        for p in &mesh.positions {
                            let v = Vec3::from_array(*p);
                            aabb_min = aabb_min.min(v);
                            aabb_max = aabb_max.max(v);
                        }
                        let mut feat_tint = layer.color_tint;
                        if let Some(c) = feature.color {
                            feat_tint = c;
                        }
                        feat_tint[3] *= layer.opacity;
                        let is_edge_enabled = layer.edge_enabled && feature.edge_enabled;
                        let is_flat = feature.height <= 0.25;

                        if is_flat {
                            raw_meshes_ground.push((mesh, feat_tint, is_edge_enabled, aabb_min, aabb_max));
                        } else {
                            raw_meshes_casters.push((mesh, feat_tint, is_edge_enabled, aabb_min, aabb_max));
                        }
                    }
                }

                let chunk_size = 10000;
                let should_cast = layer.cast_shadows;
                for chunk in raw_meshes_casters.chunks(chunk_size) {
                    let mut chunk_min = Vec3::splat(f32::INFINITY);
                    let mut chunk_max = Vec3::splat(f32::NEG_INFINITY);
                    let mut chunk_items = Vec::with_capacity(chunk.len());
                    for (m, col, edge_en, aabb_m1, aabb_m2) in chunk {
                        chunk_min = chunk_min.min(*aabb_m1);
                        chunk_max = chunk_max.max(*aabb_m2);
                        chunk_items.push((*m, *col, *edge_en));
                    }
                    renderer.load_batched_chunk(&chunk_items, layer.shadow_color, should_cast, chunk_min, chunk_max);
                }

                for chunk in raw_meshes_ground.chunks(chunk_size) {
                    let mut chunk_min = Vec3::splat(f32::INFINITY);
                    let mut chunk_max = Vec3::splat(f32::NEG_INFINITY);
                    let mut chunk_items = Vec::with_capacity(chunk.len());
                    for (m, col, edge_en, aabb_m1, aabb_m2) in chunk {
                        chunk_min = chunk_min.min(*aabb_m1);
                        chunk_max = chunk_max.max(*aabb_m2);
                        chunk_items.push((*m, *col, *edge_en));
                    }
                    renderer.load_batched_chunk(&chunk_items, layer.shadow_color, false, chunk_min, chunk_max);
                }
            }
        }
        self.gpu_meshes_dirty = false;
    }

    // --- Spatial Queries & Picking ---

    pub fn screen_to_ray(&self, screen_x: f32, screen_y: f32, width: f32, height: f32) -> Ray {
        let view_proj = self.camera.view_proj_matrix(width / height.max(1.0));
        screen_to_ray(glam::Vec2::new(screen_x, screen_y), glam::Vec2::new(width, height), view_proj)
    }

    pub fn intersect_ground(&self, ray: &Ray) -> Option<Vec3> {
        if ray.direction.y.abs() < 1e-6 {
            return None;
        }
        let t = -ray.origin.y / ray.direction.y;
        if t >= 0.0 {
            Some(ray.origin + ray.direction * t)
        } else {
            None
        }
    }

    /// Determines the 3D world intersection point on building colliders, 3D terrain DEM, or ground plane
    pub fn intersect_scene_or_terrain(&self, ray: &Ray) -> Option<Vec3> {
        const WGS84_RADIUS: f32 = crate::gis::crs::WGS84_A as f32;

        if self.projection_mode == ProjectionMode::GlobeECEF {
            return ray.intersect_sphere(Vec3::ZERO, WGS84_RADIUS);
        }

        // 1. Building mesh collider hit
        let p0 = ray.origin;
        let p1 = ray.origin + ray.direction * 50_000.0;
        let mesh_hit = self.collider.cast_ray_segment(p0, p1).map(|(_, pt, _)| pt);

        // 2. Terrain or ground hit
        let ground_hit = if self.terrain.is_enabled {
            let origin = self.scene.origin;
            let terrain = &self.terrain;
            ray.intersect_terrain_bisection(|x, z| {
                let geo = origin.local_to_geo(Vec3::new(x, 0.0, z));
                let geodetic_elev = terrain
                    .sample_elevation(geo.latitude, geo.longitude)
                    .unwrap_or(origin.origin.elevation as f32) as f64;
                let local_surface = origin.lat_lon_to_local(geo.latitude, geo.longitude, geodetic_elev);
                local_surface.y
            }, -2000.0)
        } else {
            ray.intersect_ground_plane(0.0)
        };

        match (mesh_hit, ground_hit) {
            (Some(m), Some(g)) => {
                if (m - ray.origin).length_squared() < (g - ray.origin).length_squared() {
                    Some(m)
                } else {
                    Some(g)
                }
            }
            (Some(m), None) => Some(m),
            (None, Some(g)) => Some(g),
            (None, None) => None,
        }
    }

    /// Cast a ray and test for intersection against buildings/features
    pub fn pick_feature(&self, ray: &Ray) -> Option<(GisFeature, Vec3)> {
        let p0 = ray.origin;
        let p1 = ray.origin + ray.direction * 50_000.0;
        let (_dist, hit_pt, feat_id) = self.collider.cast_ray_segment(p0, p1)?;
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            if let Some(feat) = layer.features.iter().find(|f| f.id == feat_id) {
                return Some((feat.clone(), hit_pt));
            }
        }
        None
    }

    /// Sets the active selected feature and syncs the highlight mesh to the renderer
    pub fn select_feature(&mut self, feature: Option<GisFeature>) {
        if let Some(r) = &mut self.renderer {
            r.set_selected_mesh(feature.as_ref().and_then(|f| f.mesh.as_ref()));
        }
        self.events.push(MapEvent::FeatureSelected(feature.clone()));
        self.selected_feature = feature;
    }

    // --- Events ---

    pub fn push_event(&mut self, event: MapEvent) {
        self.events.push(event);
    }

    pub fn poll_events(&mut self) -> Vec<MapEvent> {
        std::mem::take(&mut self.events)
    }
}
