use glam::Vec3;
use crate::gis::basemap::{Basemap, BasemapManager};
use crate::gis::cache::ResourceBudget;
use crate::gis::crs::{GeoCoord, ProjectOrigin, ProjectionMode};
use crate::gis::geojson_loader::GisFeature;
use crate::gis::layer::{
    FeatureLayer, Layer, LayerGpuContext, LayerRegistry, LayerStatus, LayerType,
    LayerUpdateContext,
};
use crate::gis::terrain::{Terrain, TerrainManager};
use crate::renderer::camera::{Camera, GlobeFlightState};
use crate::renderer::render_engine::RenderEngine;
use crate::scene::scene::Scene;
use crate::gis::source::SourceRegistry;
use crate::solar::datetime_state::SolarDateTimeState;
use crate::solar::shadow_analysis::SceneCollider;
use crate::solar::sun_calc::{calculate_solar_position, SolarPosition};
use crate::spatial::picking::{screen_to_ray, Ray};
use crate::engine::command::{
    BasemapCommand, CameraCommand, ClockCommand, CommandError, EdgeCommand,
    EnvironmentCommand, LayerCommand, MapCommand, TerrainCommand,
};
use crate::engine::event::MapEvent;
use crate::engine::view::MapView;

static MAP_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The central 3D GIS Map engine.
///
/// Encapsulates GIS layers, backdrop basemap, 3D elevation terrain,
/// camera navigation, solar lighting/shadow analysis, spatial picking, and WGPU rendering.
pub struct Map {
    // GIS Document Metadata & CRS
    pub id: String,
    pub title: String,
    pub spatial_reference: crate::gis::geometry::SpatialReference,

    // Renderer
    pub renderer: Option<RenderEngine>,

    // Scene & Coordinates
    pub scene: Scene,
    pub camera: Camera,
    pub projection_mode: ProjectionMode,

    // GIS Streaming Data Sources & Layers
    pub sources: SourceRegistry,
    pub basemap: Option<Basemap>,
    pub(crate) basemap_mgr: BasemapManager,
    previous_basemap: Option<Basemap>,
    pub terrain: Option<Terrain>,
    pub terrain_mgr: TerrainManager,
    previous_terrain: Option<Terrain>,
    pub layers: Vec<Box<dyn Layer>>,
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

    // Viewing Mode & Projection Auto-Switch Configuration
    pub viewing_mode: crate::gis::map::ViewingMode,
    previous_viewing_mode: crate::gis::map::ViewingMode,
    pub auto_switch_altitude: Option<f64>,

    // Status & Event Queue
    pub status_message: String,
    pub events: Vec<MapEvent>,
}

/// Backward compatibility alias: `MapEngine` is now identical to [`Map`].
pub type MapEngine = Map;

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

impl Map {
    /// Creates a new Map initialized with default settings (Melbourne CBD origin, WGS 84, OSM basemap).
    pub fn new() -> Self {
        let origin = ProjectOrigin::from_geo(GeoCoord {
            latitude: -37.8136,
            longitude: 144.9631,
            elevation: 0.0,
        });
        Self::from_origin(origin)
    }

    /// Creates a new Map initialized at the given geographic project origin.
    pub fn from_origin(origin: impl Into<ProjectOrigin>) -> Self {
        let origin = origin.into();
        let solar_dt = SolarDateTimeState::default();
        let utc_dt = solar_dt.to_utc_datetime();
        let solar_pos = calculate_solar_position(
            origin.origin.latitude,
            origin.origin.longitude,
            &utc_dt,
            solar_dt.timezone_offset_hours,
        );

        Self {
            id: format!("map_{}", MAP_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            title: "Untitled Map".to_string(),
            spatial_reference: crate::gis::geometry::SpatialReference::Wgs84,
            renderer: None,
            scene: Scene::new(origin),
            camera: Camera::default(),
            projection_mode: ProjectionMode::PlanarENU,

            sources: SourceRegistry::new(),
            basemap: Some(Basemap::osm()),
            basemap_mgr: BasemapManager::new(),
            previous_basemap: None,
            terrain: None,
            terrain_mgr: TerrainManager::new(),
            previous_terrain: None,
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
            viewing_mode: crate::gis::map::ViewingMode::default(),
            previous_viewing_mode: crate::gis::map::ViewingMode::default(),
            auto_switch_altitude: Some(50_000.0),
            status_message: String::new(),
            events: Vec::new(),
        }
    }

    /// Returns the geographic origin of the map.
    pub fn origin(&self) -> GeoCoord {
        self.scene.origin.origin
    }

    /// Synchronizes changes in `self.viewing_mode` to projection mode and altitude threshold.
    pub fn sync_viewing_mode_if_changed(&mut self) {
        if self.viewing_mode == self.previous_viewing_mode {
            return;
        }

        match self.viewing_mode {
            crate::gis::map::ViewingMode::Auto { threshold_altitude } => {
                self.auto_switch_altitude = Some(threshold_altitude);
            }
            crate::gis::map::ViewingMode::Globe => {
                self.auto_switch_altitude = None;
                self.transition_to_globe();
            }
            crate::gis::map::ViewingMode::Planar => {
                self.auto_switch_altitude = None;
                self.projection_mode = ProjectionMode::PlanarENU;
                if let Some(r) = &mut self.renderer {
                    r.projection_mode = ProjectionMode::PlanarENU;
                    r.morph_progress = 0.0;
                }
            }
        }

        self.previous_viewing_mode = self.viewing_mode;
    }

    /// Synchronizes changes in `self.basemap` to `self.basemap_mgr` and reloads renderer tiles if needed.
    pub fn sync_basemap_if_changed(&mut self) {
        if self.basemap == self.previous_basemap {
            return;
        }

        match &self.basemap {
            Some(bm) => {
                self.basemap_mgr.set_basemap(bm.clone());
                self.basemap_mgr.is_enabled = true;
                self.basemap_mgr.reset_cache();
                if let Some(r) = &mut self.renderer {
                    r.clear_basemap_tiles();
                }
                self.events.push(MapEvent::BasemapChanged(Some(bm.clone())));
            }
            None => {
                self.basemap_mgr.is_enabled = false;
                self.basemap_mgr.reset_cache();
                if let Some(r) = &mut self.renderer {
                    r.clear_basemap_tiles();
                }
                self.events.push(MapEvent::BasemapChanged(None));
            }
        }

        self.previous_basemap = self.basemap.clone();
    }

    /// Synchronizes changes in `self.terrain` to `self.terrain_mgr` and reloads renderer meshes if needed.
    pub fn sync_terrain_if_changed(&mut self) {
        if self.terrain == self.previous_terrain {
            return;
        }

        match &self.terrain {
            Some(t) => {
                let prev_provider = self.terrain_mgr.provider;
                let prev_url = self.terrain_mgr.custom_url.clone();
                let new_provider = t.provider();
                let new_url = t.custom_url().map(|s| s.to_string());
                let new_exagg = t.exaggeration();
                let was_enabled = self.terrain_mgr.is_enabled;
                let prev_exagg = self.terrain_mgr.height_exaggeration;

                self.terrain_mgr.is_enabled = true;
                self.terrain_mgr.height_exaggeration = new_exagg;

                let provider_or_url_changed = prev_provider != new_provider || prev_url != new_url;
                if provider_or_url_changed {
                    self.terrain_mgr.set_provider_with_url(new_provider, new_url);
                }

                if !was_enabled || provider_or_url_changed || (prev_exagg - new_exagg).abs() > 1e-4 {
                    if let Some(renderer) = &mut self.renderer {
                        if self.basemap_mgr.is_enabled {
                            renderer.reload_all_terrain_meshes(&self.scene.origin, &self.terrain_mgr);
                        }
                    }
                    if !was_enabled {
                        self.events.push(MapEvent::TerrainToggled(true));
                    }
                }
            }
            None => {
                let was_enabled = self.terrain_mgr.is_enabled;
                self.terrain_mgr.is_enabled = false;
                if was_enabled {
                    if let Some(renderer) = &mut self.renderer {
                        if self.basemap_mgr.is_enabled {
                            renderer.reload_all_terrain_meshes(&self.scene.origin, &self.terrain_mgr);
                        } else {
                            renderer.clear_basemap_tiles();
                        }
                    }
                    self.events.push(MapEvent::TerrainToggled(false));
                }
            }
        }

        self.previous_terrain = self.terrain.clone();
    }

    /// Attach a pre-configured RenderEngine
    pub fn set_renderer(&mut self, renderer: RenderEngine) {
        self.renderer = Some(renderer);
        self.reload_all_gpu_meshes();
    }

    /// Changes the project origin (local coordinate system center) and updates GIS caches.
    /// Accepts `ProjectOrigin`, `GeoCoord`, or `[x, y, z]` array (`[longitude, latitude, elevation]`).
    pub fn set_origin(&mut self, origin: impl Into<ProjectOrigin>) {
        let origin = origin.into();
        self.scene.origin = origin;
        for layer in &mut self.layers {
            layer.on_origin_changed(&origin);
        }
        self.basemap_mgr.reset_cache();
        if let Some(r) = &mut self.renderer {
            r.clear_basemap_tiles();
        }
        self.reload_all_gpu_meshes();
        self.events.push(MapEvent::OriginChanged(origin));
    }

    /// Returns an immutable read view facade over Map state
    pub fn view(&self) -> MapView<'_> {
        let edge_cfg = self.renderer.as_ref().map(|r| &r.edge_renderer.config);
        MapView {
            origin: &self.scene.origin,
            camera: &self.camera,
            layers: &self.layers,
            layer_registry: &self.layer_registry,
            sources: &self.sources,
            budget: &self.budget,
            basemap: self.basemap.as_ref(),
            basemap_mgr: &self.basemap_mgr,
            terrain: self.terrain.as_ref(),
            terrain_mgr: &self.terrain_mgr,
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
        self.basemap_mgr.is_streaming()
            || self.terrain_mgr.is_streaming()
            || self.layers.iter().any(|l| l.is_streaming())
    }

    /// Returns true if downloaded basemap or terrain tiles are waiting to be uploaded to GPU
    pub fn has_unconsumed_completed(&self) -> bool {
        self.basemap_mgr.has_unconsumed_completed() || self.terrain_mgr.has_unconsumed_completed()
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
        // 0. Synchronize viewing mode, terrain & basemap if modified
        self.sync_viewing_mode_if_changed();
        self.sync_terrain_if_changed();
        self.sync_basemap_if_changed();

        let (vp_w, vp_h) = if let Some(renderer) = &self.renderer {
            (renderer.current_width as f32, renderer.current_height as f32)
        } else {
            (1280.0, 720.0)
        };

        if self.basemap_mgr.is_enabled || self.terrain_mgr.is_enabled {
            let active_tiles = if self.projection_mode == ProjectionMode::GlobeECEF {
                self.basemap_mgr.calculate_globe_camera_tiles(&self.camera, vp_w, vp_h)
            } else {
                let center_geo = self.scene.origin.local_to_geo(self.camera.target);
                let dyn_zoom = crate::gis::basemap::BasemapManager::calculate_camera_lod_zoom_with_hysteresis(
                    self.camera.distance,
                    self.camera.fov_y,
                    vp_h,
                    center_geo.latitude,
                    self.basemap_mgr.zoom,
                );
                self.basemap_mgr.zoom = dyn_zoom;
                self.basemap_mgr.calculate_camera_tiles(
                    &self.scene.origin,
                    &self.camera,
                    vp_w,
                    vp_h,
                )
            };
            self.basemap_mgr.target_active_count = active_tiles.len();
            self.basemap_mgr.previous_active_tiles = active_tiles.iter().copied().collect();

            let mut new_tiles = Vec::new();
            if self.basemap_mgr.is_enabled {
                self.basemap_mgr.request_tiles(&active_tiles);
                new_tiles = self.basemap_mgr.drain_completed_tiles();
            } else if self.terrain_mgr.is_enabled {
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

            if self.terrain_mgr.is_enabled {
                self.terrain_mgr.request_tiles(&active_tiles);
            }
            let new_terrain_tiles = if self.terrain_mgr.is_enabled {
                self.terrain_mgr.drain_completed()
            } else {
                Vec::new()
            };

            if !new_terrain_tiles.is_empty() {
                self.has_new_gpu_tiles = true;
            }

            if let Some(renderer) = &mut self.renderer {
                let grid_mode = if self.basemap_mgr.is_enabled { 0.0 } else { 1.0 };
                renderer.update_basemap_uniforms(self.basemap_mgr.opacity, grid_mode, self.basemap_mgr.show_debug_borders);

                for tile in new_tiles {
                    let terrain_tile = if self.terrain_mgr.is_enabled {
                        self.terrain_mgr.get_terrain_for_tile(tile.coord)
                    } else {
                        None
                    };
                    let evicted = renderer.add_tile(
                        tile,
                        &self.scene.origin,
                        self.basemap_mgr.opacity,
                        if self.terrain_mgr.is_enabled { Some(&self.terrain_mgr) } else { None },
                        terrain_tile.as_ref(),
                        self.terrain_mgr.height_exaggeration,
                    );
                    for ev in evicted {
                        self.basemap_mgr.unmark_requested(ev);
                    }
                }

                if self.terrain_mgr.is_enabled {
                    for terrain_tile in &new_terrain_tiles {
                        let affected_coords: Vec<crate::gis::basemap::TileCoord> = renderer
                            .gpu_tiles
                            .keys()
                            .copied()
                            .filter(|c| c == &terrain_tile.coord || c.is_descendant_of(&terrain_tile.coord) || terrain_tile.coord.is_descendant_of(c))
                            .collect();
                        for aff in affected_coords {
                            let aff_terrain = self.terrain_mgr.get_terrain_for_tile(aff);
                            renderer.update_tile_terrain_mesh(
                                aff,
                                &self.scene.origin,
                                Some(&self.terrain_mgr),
                                aff_terrain.as_ref(),
                                self.terrain_mgr.height_exaggeration,
                            );
                        }
                    }
                }

                let idle_evicted = renderer.prune_unneeded_tiles(&active_tiles);
                for ev in idle_evicted {
                    self.basemap_mgr.unmark_requested(ev);
                }
            }
        } else if let Some(renderer) = &mut self.renderer {
            if !renderer.gpu_tiles.is_empty() {
                renderer.clear_basemap_tiles();
            }
        }

        // Operational Layers (polymorphic streaming, LOD evaluation, GPU synchronization)
        let update_ctx = LayerUpdateContext {
            camera: &self.camera,
            origin: &self.scene.origin,
            projection_mode: self.projection_mode,
            viewport_width: vp_w,
            viewport_height: vp_h,
            dt: 0.016,
        };

        for layer in &mut self.layers {
            if layer.visible() {
                let status = layer.update(&update_ctx);
                if let LayerStatus::Streaming { pending_requests } = status {
                    if pending_requests > 0 {
                        self.has_new_gpu_tiles = true;
                    }
                }
            }
        }

        if let Some(renderer) = &mut self.renderer {
            let mut gpu_ctx = LayerGpuContext {
                renderer,
                collider: &mut self.collider,
                origin: &self.scene.origin,
                projection_mode: self.projection_mode,
            };
            for layer in &mut self.layers {
                if layer.visible() {
                    layer.sync_gpu(&mut gpu_ctx);
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
                    self.transition_to_planar_at_geo_with_pose(target_geo.latitude, target_geo.longitude, 20_000.0, 0.0, 45.0);

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

    /// Transitions from 3D Globe to Local Planar ENU centered on the specified geographic coordinate,
    /// with customizable camera heading (degrees, 0° = North) and tilt (degrees, 0° = nadir top-down, 45° = oblique).
    pub fn transition_to_planar_at_geo_with_pose(
        &mut self,
        lat: f64,
        lon: f64,
        target_distance: f32,
        heading_deg: f32,
        tilt_deg: f32,
    ) {
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
        self.basemap_mgr.reset_cache();
        self.terrain_mgr.clear_cache();
        for layer in &mut self.layers {
            layer.on_origin_changed(&self.scene.origin);
        }

        self.camera.distance = target_distance.clamp(100.0, 50_000.0);
        self.camera.target_distance = self.camera.distance;

        // Heading: 0° = North, 90° = East
        self.camera.yaw = heading_deg.to_radians();
        self.camera.normalize_yaw();
        self.camera.target_yaw = self.camera.yaw;

        // Tilt: 0° = nadir top-down, 45° = oblique, 85° = horizon (pitch = 90° - tilt)
        let pitch_deg = (90.0 - tilt_deg).clamp(2.0, 88.5);
        self.camera.pitch = pitch_deg.to_radians();
        self.camera.target_pitch = self.camera.pitch;

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
        self.basemap_mgr.reset_cache();
        self.terrain_mgr.clear_cache();

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
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id() == id) {
                        l.set_visible(visible);
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::SetOpacity { id, opacity } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id() == id) {
                        l.set_opacity(opacity);
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::SetColorTint { id, tint } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id() == id) {
                        l.set_color_tint(tint);
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::SetShadow { id, cast_shadows } => {
                    if let Some(l) = self.layers.iter_mut().find(|l| l.id() == id) {
                        l.set_cast_shadows(cast_shadows);
                        self.reload_all_gpu_meshes();
                    } else {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::Remove(id) => {
                    if !self.remove_layer(&id) {
                        return Err(CommandError::LayerNotFound(id));
                    }
                }
                LayerCommand::Add(layer) => {
                    self.layers.push(layer);
                    self.reload_all_gpu_meshes();
                }
                LayerCommand::AddDescriptor(_) => {}
            },
            MapCommand::Basemap(bm_cmd) => match bm_cmd {
                BasemapCommand::Set(basemap_opt) => {
                    self.basemap = basemap_opt;
                    self.sync_basemap_if_changed();
                }
                BasemapCommand::ResetCache => {
                    self.basemap_mgr.reset_cache();
                    if let Some(r) = &mut self.renderer {
                        r.gpu_tiles.clear();
                    }
                }
            },
            MapCommand::Terrain(t_cmd) => match t_cmd {
                TerrainCommand::SetEnabled(enabled) => {
                    if enabled {
                        if self.terrain.is_none() {
                            self.terrain = Some(crate::gis::terrain::AwsTerrain::default().into());
                        }
                    } else {
                        self.terrain = None;
                    }
                    self.sync_terrain_if_changed();
                }
                TerrainCommand::SetHeightExaggeration(exagg) => {
                    if let Some(t) = &mut self.terrain {
                        t.set_exaggeration(exagg);
                    }
                    self.terrain_mgr.height_exaggeration = exagg;
                    if let Some(renderer) = &mut self.renderer {
                        renderer.reload_all_terrain_meshes(&self.scene.origin, &self.terrain_mgr);
                    }
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

    /// Navigates the map camera to a target destination with optional distance, heading, tilt, and animation.
    ///
    /// - In **Planar (ENU)** mode: targets the local tangent coordinates converted from geographic coordinates.
    ///   Applies distance, heading, and tilt if specified in `options`.
    /// - In **Globe (ECEF)** mode: centers the geographic coordinate on the 3D globe looking nadir towards
    ///   Earth center. `heading` and `tilt` are ignored. If `distance` is None, current distance is retained.
    /// - By default, smoothly animates camera motion (`animate = true`). Set `opts.animate = false`
    ///   or use `GoToOptions::immediate()` for instant teleportation.
    pub fn goto<T: IntoGoToOptions>(&mut self, target: impl Into<GoToTarget>, options: T) {
        let target = target.into();
        let options = options.into_goto_options();
        let animate = options.as_ref().map_or(true, |o| o.is_animated());
        const WGS84_RADIUS: f32 = 6_378_137.0;

        match self.projection_mode {
            ProjectionMode::PlanarENU => {
                let auto_threshold = self.auto_switch_altitude.unwrap_or(0.0) as f32;
                if auto_threshold > 0.0 {
                    if let Some(opts) = options {
                        if let Some(dist) = opts.distance {
                            if dist > auto_threshold * 1.1 {
                                // Distance exceeds planar limit: auto-transition up to Globe
                                let geo = match target {
                                    GoToTarget::Current => self.scene.origin.local_to_geo(self.camera.target),
                                    GoToTarget::Geo { longitude, latitude, elevation } => {
                                        GeoCoord::new(latitude, longitude, elevation.unwrap_or(0.0))
                                    }
                                    GoToTarget::Local(pt) => self.scene.origin.local_to_geo(pt),
                                };
                                self.transition_to_globe();
                                self.goto(geo, options);
                                return;
                            }
                        }
                    }
                }

                let (local_pt, target_geo) = match target {
                    GoToTarget::Current => (self.camera.target, None),
                    GoToTarget::Geo { longitude, latitude, elevation } => {
                        let elev = elevation.unwrap_or(0.0);
                        let pt = self.scene.origin.lat_lon_to_local(latitude, longitude, elev);
                        (pt, Some((latitude, longitude)))
                    }
                    GoToTarget::Local(pt) => (pt, None),
                };

                // If target coordinate is > 50km from current origin, re-center origin to prevent tangential planar distortion
                let local_pt = if let Some((lat, lon)) = target_geo {
                    if local_pt.x.abs() > 50_000.0 || local_pt.z.abs() > 50_000.0 {
                        self.set_origin(GeoCoord::new(lat, lon, 0.0));
                        Vec3::new(0.0, local_pt.y, 0.0)
                    } else {
                        local_pt
                    }
                } else {
                    local_pt
                };

                if animate {
                    self.camera.target_lookat = local_pt;

                    if let Some(opts) = options {
                        if let Some(dist) = opts.distance {
                            self.camera.target_distance = dist.max(1.0);
                        }
                        if let Some(heading_deg) = opts.heading {
                            let mut target_yaw = heading_deg.to_radians();
                            while target_yaw > std::f32::consts::PI {
                                target_yaw -= std::f32::consts::TAU;
                            }
                            while target_yaw < -std::f32::consts::PI {
                                target_yaw += std::f32::consts::TAU;
                            }
                            self.camera.target_yaw = target_yaw;
                        }
                        if let Some(tilt_deg) = opts.tilt {
                            let pitch_deg = (90.0 - tilt_deg).clamp(2.0, 88.5);
                            self.camera.target_pitch = pitch_deg.to_radians();
                        }
                    }
                } else {
                    self.camera.target = local_pt;
                    self.camera.target_lookat = local_pt;

                    if let Some(opts) = options {
                        if let Some(dist) = opts.distance {
                            self.camera.distance = dist.max(1.0);
                            self.camera.target_distance = self.camera.distance;
                        }
                        if let Some(heading_deg) = opts.heading {
                            self.camera.yaw = heading_deg.to_radians();
                            self.camera.normalize_yaw();
                            self.camera.target_yaw = self.camera.yaw;
                        }
                        if let Some(tilt_deg) = opts.tilt {
                            let pitch_deg = (90.0 - tilt_deg).clamp(2.0, 88.5);
                            self.camera.pitch = pitch_deg.to_radians();
                            self.camera.target_pitch = self.camera.pitch;
                        }
                    }
                    self.camera.snap_smoothing();
                }
            }
            ProjectionMode::GlobeECEF => {
                let geo = match target {
                    GoToTarget::Current => crate::gis::crs::ecef_to_geodetic(self.camera.eye_position()),
                    GoToTarget::Geo { longitude, latitude, elevation } => {
                        GeoCoord::new(latitude, longitude, elevation.unwrap_or(0.0))
                    }
                    GoToTarget::Local(pt) => self.scene.origin.local_to_geo(pt),
                };

                // Interpret distance: if dist < WGS84_RADIUS, it is altitude above the Earth surface.
                // If dist >= WGS84_RADIUS, it is absolute radial distance from Earth center.
                let (altitude, radial_distance) = match options.as_ref().and_then(|o| o.distance) {
                    Some(dist) => {
                        if dist >= WGS84_RADIUS {
                            (dist - WGS84_RADIUS, dist)
                        } else {
                            (dist, WGS84_RADIUS + dist)
                        }
                    }
                    None => {
                        let cur_radial = self.camera.distance;
                        let cur_alt = (cur_radial - WGS84_RADIUS).max(10.0);
                        (cur_alt, cur_radial)
                    }
                };

                let auto_threshold = self.auto_switch_altitude.unwrap_or(0.0) as f32;

                // In Auto-switch mode, if requested altitude <= threshold (e.g. <= 50,000 m),
                // smoothly transition to Planar mode at the target geographic coordinate!
                if auto_threshold > 0.0 && altitude <= auto_threshold {
                    let heading = options.as_ref().and_then(|o| o.heading).unwrap_or(0.0);
                    let tilt = options.as_ref().and_then(|o| o.tilt).unwrap_or(45.0);
                    self.transition_to_planar_at_geo_with_pose(
                        geo.latitude,
                        geo.longitude,
                        altitude.clamp(100.0, 50_000.0),
                        heading,
                        tilt,
                    );
                    self.events.push(MapEvent::CameraMoved);
                    return;
                }

                // Otherwise, stay in Globe mode orbiting around Earth center at radial_distance
                let (pitch, yaw) = crate::gis::crs::geo_to_globe_camera_angles(&geo);

                if animate {
                    self.camera.target = Vec3::ZERO;
                    self.camera.target_lookat = Vec3::ZERO;
                    self.camera.normalize_yaw();
                    self.camera.target_pitch = pitch;
                    self.camera.target_yaw = yaw;
                    self.camera.target_distance = radial_distance;
                } else {
                    self.camera.target = Vec3::ZERO;
                    self.camera.target_lookat = Vec3::ZERO;
                    self.camera.pitch = pitch;
                    self.camera.target_pitch = pitch;
                    self.camera.yaw = yaw;
                    self.camera.normalize_yaw();
                    self.camera.target_yaw = self.camera.yaw;
                    self.camera.distance = radial_distance;
                    self.camera.target_distance = radial_distance;
                    self.camera.snap_smoothing();
                }
            }
        }

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
        self.camera.target_yaw = 0.0;
        self.events.push(MapEvent::CameraMoved);
    }

    pub fn reset_view(&mut self) {
        self.camera = Camera::default();
        self.events.push(MapEvent::CameraMoved);
    }

    // --- GIS Layers ---

    /// Appends an operational layer to the map.
    ///
    /// Accepts concrete layer types (e.g. `FeatureLayer`, `SceneLayer`, `TileLayer`)
    /// or a pre-boxed `Box<dyn Layer>`.
    pub fn add_layer(&mut self, layer: impl crate::gis::layer::IntoLayer) -> usize {
        let mut layer = layer.into_layer();
        layer.on_origin_changed(&self.scene.origin);
        self.layers.push(layer);
        let idx = self.layers.len() - 1;
        self.reload_all_gpu_meshes();
        idx
    }

    /// Removes an operational layer by its unique ID.
    /// Cleans up GPU resources, collider entries, and internal buffers.
    pub fn remove_layer(&mut self, id: &str) -> bool {
        if let Some(pos) = self.layers.iter().position(|l| l.id() == id) {
            let mut removed = self.layers.remove(pos);
            if let Some(renderer) = &mut self.renderer {
                let mut gpu_ctx = LayerGpuContext {
                    renderer,
                    collider: &mut self.collider,
                    origin: &self.scene.origin,
                    projection_mode: self.projection_mode,
                };
                removed.destroy(&mut gpu_ctx);
            }
            self.reload_all_gpu_meshes();
            true
        } else {
            false
        }
    }

    /// Removes all operational layers and cleans up their GPU resources.
    pub fn clear_layers(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            let mut gpu_ctx = LayerGpuContext {
                renderer,
                collider: &mut self.collider,
                origin: &self.scene.origin,
                projection_mode: self.projection_mode,
            };
            for mut layer in self.layers.drain(..) {
                layer.destroy(&mut gpu_ctx);
            }
        } else {
            self.layers.clear();
        }
        self.reload_all_gpu_meshes();
    }

    /// Retrieves an immutable reference to a layer by ID downcast to concrete type `L`.
    pub fn get_layer<L: 'static>(&self, id: &str) -> Option<&L> {
        self.layers
            .iter()
            .find(|l| l.id() == id)
            .and_then(|l| l.as_any().downcast_ref::<L>())
    }

    /// Retrieves a mutable reference to a layer by ID downcast to concrete type `L`.
    pub fn get_layer_mut<L: 'static>(&mut self, id: &str) -> Option<&mut L> {
        self.layers
            .iter_mut()
            .find(|l| l.id() == id)
            .and_then(|l| l.as_any_mut().downcast_mut::<L>())
    }

    pub fn load_geojson(&mut self, geojson_str: &str, layer_name: &str) -> Result<usize, String> {
        let dataset = crate::gis::geojson_loader::parse_geojson(geojson_str, Some(self.scene.origin))?;
        let mut layer = FeatureLayer::new(
            format!("layer_{}", self.layers.len()),
            layer_name.to_string(),
            LayerType::Buildings,
            [0.85, 0.88, 0.92, 1.0],
        );
        layer.features = dataset.features;
        self.layers.push(Box::new(layer));
        let idx = self.layers.len() - 1;
        self.reload_all_gpu_meshes();
        Ok(idx)
    }

    /// Rebuilds extruded 3D meshes for a specific layer if missing
    pub fn rebuild_layer_feature_meshes(&mut self, layer_idx: usize) {
        if layer_idx >= self.layers.len() {
            return;
        }
        let origin = self.scene.origin;
        if let Some(feat_layer) = self.layers[layer_idx].as_any_mut().downcast_mut::<FeatureLayer>() {
            for feature in &mut feat_layer.features {
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
    }

    /// Re-uploads and syncs all layer feature meshes and colliders to the 3D renderer
    pub fn reload_all_gpu_meshes(&mut self) {
        self.scene.nodes.clear();
        self.collider.clear();

        for i in 0..self.layers.len() {
            let needs_rebuild = if let Some(feat_layer) = self.layers[i].as_any().downcast_ref::<FeatureLayer>() {
                feat_layer.features.iter().any(|f| {
                    !f.geo_polygons.is_empty() && f.mesh.is_none()
                })
            } else {
                false
            };
            if needs_rebuild {
                self.rebuild_layer_feature_meshes(i);
            }
        }

        // 1. Populate SceneCollider and SceneNodes for spatial queries and picking
        for layer in &self.layers {
            if !layer.visible() {
                continue;
            }
            if let Some(feat_layer) = layer.as_any().downcast_ref::<FeatureLayer>() {
                for feature in &feat_layer.features {
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
                            feat_layer.id.clone(),
                            Some(feature.id.clone()),
                            0,
                            aabb_min,
                            aabb_max,
                        );
                        node.is_visible = feat_layer.visible;
                        node.shadow_color = feature.shadow_color;
                        self.scene.nodes.push(node);
                    }
                }
            }
        }

        // 2. Upload to GPU RenderEngine if present
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_meshes();
            renderer.clear_custom_shadow_colors();

            for layer in &self.layers {
                if !layer.visible() {
                    continue;
                }
                if let Some(feat_layer) = layer.as_any().downcast_ref::<FeatureLayer>() {
                    let mut raw_meshes_casters = Vec::new();
                    let mut raw_meshes_ground = Vec::new();

                    for feature in &feat_layer.features {
                        if let Some(mesh) = &feature.mesh {
                            let mut aabb_min = Vec3::splat(f32::INFINITY);
                            let mut aabb_max = Vec3::splat(f32::NEG_INFINITY);
                            for p in &mesh.positions {
                                let v = Vec3::from_array(*p);
                                aabb_min = aabb_min.min(v);
                                aabb_max = aabb_max.max(v);
                            }
                            let mut feat_tint = feat_layer.color_tint;
                            if let Some(c) = feature.color {
                                feat_tint = c;
                            }
                            feat_tint[3] *= feat_layer.opacity;
                            let is_edge_enabled = feat_layer.edge_enabled && feature.edge_enabled;
                            let is_flat = feature.height <= 0.25;

                            if is_flat {
                                raw_meshes_ground.push((mesh, feat_tint, is_edge_enabled, aabb_min, aabb_max));
                            } else {
                                raw_meshes_casters.push((mesh, feat_tint, is_edge_enabled, aabb_min, aabb_max));
                            }
                        }
                    }

                    let chunk_size = 10000;
                    let should_cast = feat_layer.cast_shadows;
                    for chunk in raw_meshes_casters.chunks(chunk_size) {
                        let mut chunk_min = Vec3::splat(f32::INFINITY);
                        let mut chunk_max = Vec3::splat(f32::NEG_INFINITY);
                        let mut chunk_items = Vec::with_capacity(chunk.len());
                        for (m, col, edge_en, aabb_m1, aabb_m2) in chunk {
                            chunk_min = chunk_min.min(*aabb_m1);
                            chunk_max = chunk_max.max(*aabb_m2);
                            chunk_items.push((*m, *col, *edge_en));
                        }
                        renderer.load_batched_chunk(&chunk_items, feat_layer.shadow_color, should_cast, chunk_min, chunk_max);
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
                        renderer.load_batched_chunk(&chunk_items, feat_layer.shadow_color, false, chunk_min, chunk_max);
                    }
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
        let mesh_hit: Option<Vec3> = self.collider.cast_ray_segment(p0, p1).map(|(_, pt, _)| pt);

        // 2. Terrain or ground hit
        let ground_hit: Option<Vec3> = if self.terrain_mgr.is_enabled {
            let origin = self.scene.origin;
            let terrain = &self.terrain_mgr;
            ray.intersect_terrain_bisection(|x: f32, z: f32| -> f32 {
                let geo = origin.local_to_geo(Vec3::new(x, 0.0, z));
                let geodetic_elev = terrain
                    .sample_elevation(geo.latitude, geo.longitude)
                    .unwrap_or(origin.origin.elevation as f32) as f64;
                let local_surface = origin.lat_lon_to_local(geo.latitude, geo.longitude, geodetic_elev);
                local_surface.y
            }, -2000.0f32)
        } else {
            ray.intersect_ground_plane(0.0f32)
        };

        match (mesh_hit, ground_hit) {
            (Some(m), Some(g)) => {
                let d_m = (m - ray.origin).length_squared();
                let d_g = (g - ray.origin).length_squared();
                if d_m < d_g {
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
            if !layer.visible() {
                continue;
            }
            if let Some(feat_layer) = layer.as_any().downcast_ref::<FeatureLayer>() {
                if let Some(feat) = feat_layer.features.iter().find(|f| f.id == feat_id) {
                    return Some((feat.clone(), hit_pt));
                }
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

/// Target destination for camera navigation (`map.goto`).
///
/// Accepts `[longitude, latitude]` arrays or tuples matching GIS standards
/// (GeoJSON RFC 7946, Esri `view.goTo([lon, lat])`), explicit `GeoCoord`,
/// Target destination for camera navigation (`map.goto`).
///
/// Accepts `[longitude, latitude]` (2 elements) or `[longitude, latitude, elevation]` (3 elements),
/// `(longitude, latitude)` / `(longitude, latitude, elevation)` tuples, explicit `GeoCoord`,
/// local 3D Cartesian coordinates (`Vec3`, `[x, y, z]`), or `()` to preserve the current target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GoToTarget {
    /// Preserves the current camera target focus point.
    Current,
    /// Geographic coordinate: `longitude` and `latitude` in decimal degrees, optional `elevation` in meters.
    Geo {
        longitude: f64,
        latitude: f64,
        elevation: Option<f64>,
    },
    /// Local 3D Cartesian coordinates in meters (Planar ENU mode only).
    Local(Vec3),
}

impl GoToTarget {
    /// Creates a geographic target from longitude and latitude in degrees.
    #[inline]
    pub fn lon_lat(longitude: f64, latitude: f64) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: None,
        }
    }

    /// Creates a geographic target from longitude, latitude, and elevation in meters.
    #[inline]
    pub fn lon_lat_elev(longitude: f64, latitude: f64, elevation: f64) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: Some(elevation),
        }
    }

    /// Creates a geographic target from latitude and longitude in degrees.
    #[inline]
    pub fn lat_lon(latitude: f64, longitude: f64) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: None,
        }
    }

    /// Creates a geographic target from a `GeoCoord`.
    #[inline]
    pub fn from_geo(geo: GeoCoord) -> Self {
        Self::Geo {
            longitude: geo.longitude,
            latitude: geo.latitude,
            elevation: Some(geo.elevation),
        }
    }

    /// Creates a local 3D target point for Planar mode.
    #[inline]
    pub fn local(point: Vec3) -> Self {
        Self::Local(point)
    }
}

impl From<()> for GoToTarget {
    /// Preserves current target position.
    #[inline]
    fn from(_: ()) -> Self {
        Self::Current
    }
}

impl From<[f64; 2]> for GoToTarget {
    /// Converts `[longitude, latitude]` array into a `GoToTarget`.
    #[inline]
    fn from([longitude, latitude]: [f64; 2]) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: None,
        }
    }
}

impl From<[f32; 2]> for GoToTarget {
    /// Converts `[longitude, latitude]` array into a `GoToTarget`.
    #[inline]
    fn from([longitude, latitude]: [f32; 2]) -> Self {
        Self::Geo {
            longitude: longitude as f64,
            latitude: latitude as f64,
            elevation: None,
        }
    }
}

impl From<(f64, f64)> for GoToTarget {
    /// Converts `(longitude, latitude)` tuple into a `GoToTarget`.
    #[inline]
    fn from((longitude, latitude): (f64, f64)) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: None,
        }
    }
}

impl From<[f64; 3]> for GoToTarget {
    /// Converts `[longitude, latitude, elevation]` array into a geographic `GoToTarget`.
    #[inline]
    fn from([longitude, latitude, elevation]: [f64; 3]) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: Some(elevation),
        }
    }
}

impl From<(f64, f64, f64)> for GoToTarget {
    /// Converts `(longitude, latitude, elevation)` tuple into a geographic `GoToTarget`.
    #[inline]
    fn from((longitude, latitude, elevation): (f64, f64, f64)) -> Self {
        Self::Geo {
            longitude,
            latitude,
            elevation: Some(elevation),
        }
    }
}

impl From<GeoCoord> for GoToTarget {
    #[inline]
    fn from(geo: GeoCoord) -> Self {
        Self::Geo {
            longitude: geo.longitude,
            latitude: geo.latitude,
            elevation: Some(geo.elevation),
        }
    }
}

impl From<&GeoCoord> for GoToTarget {
    #[inline]
    fn from(geo: &GeoCoord) -> Self {
        Self::Geo {
            longitude: geo.longitude,
            latitude: geo.latitude,
            elevation: Some(geo.elevation),
        }
    }
}

impl From<Vec3> for GoToTarget {
    #[inline]
    fn from(pt: Vec3) -> Self {
        Self::Local(pt)
    }
}

impl From<[f32; 3]> for GoToTarget {
    /// Converts `[x, y, z]` local coordinate array into a `GoToTarget`.
    #[inline]
    fn from([x, y, z]: [f32; 3]) -> Self {
        Self::Local(Vec3::new(x, y, z))
    }
}

/// Camera options for `map.goto(target, options)`.
///
/// Any option left as `None` preserves the camera's current value without modification.
/// For Globe mode, `heading` and `tilt` are ignored, and camera orientation is uniquely
/// calculated to center directly nadir over the target coordinate on the Earth sphere.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GoToOptions {
    /// Viewing distance in meters.
    /// In Planar mode: viewing distance from target ground point to camera eye.
    /// In Globe mode: distance from Earth center to camera eye (e.g. 18_000_000.0).
    /// If None: current camera distance is preserved.
    pub distance: Option<f32>,

    /// Compass heading in degrees (0.0 = North, 90.0 = East, 180.0 = South, 270.0 = West).
    /// Ignored in Globe mode.
    /// If None: current heading/yaw is preserved.
    pub heading: Option<f32>,

    /// Tilt angle in degrees (0.0 = top-down 2D nadir view, 45.0 = 3D oblique perspective, 85.0 = near horizon).
    /// Ignored in Globe mode.
    /// If None: current tilt/pitch is preserved.
    pub tilt: Option<f32>,

    /// Whether to animate the camera smoothly to the destination.
    /// Defaults to `true`. When set to `false`, the camera immediately snaps/teleports.
    pub animate: Option<bool>,
}

impl GoToOptions {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates options specifying only viewing distance in meters.
    #[inline]
    pub fn distance(distance: f32) -> Self {
        Self {
            distance: Some(distance),
            heading: None,
            tilt: None,
            animate: None,
        }
    }

    /// Creates options for instant teleportation (no animation).
    #[inline]
    pub fn immediate() -> Self {
        Self {
            distance: None,
            heading: None,
            tilt: None,
            animate: Some(false),
        }
    }

    /// Creates options for smoothly animated transition.
    #[inline]
    pub fn animated() -> Self {
        Self {
            distance: None,
            heading: None,
            tilt: None,
            animate: Some(true),
        }
    }

    /// Sets viewing distance in meters.
    #[inline]
    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = Some(distance);
        self
    }

    /// Sets compass heading in degrees (0° = North, 90° = East). Ignored in Globe mode.
    #[inline]
    pub fn with_heading(mut self, heading: f32) -> Self {
        self.heading = Some(heading);
        self
    }

    /// Sets tilt angle in degrees (0° = nadir top-down, 45° = oblique, 85° = horizon). Ignored in Globe mode.
    #[inline]
    pub fn with_tilt(mut self, tilt: f32) -> Self {
        self.tilt = Some(tilt);
        self
    }

    /// Sets pitch angle in degrees (90° = nadir top-down, 0° = horizon).
    #[inline]
    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.tilt = Some((90.0 - pitch).clamp(0.0, 90.0));
        self
    }

    /// Sets whether the transition should be smoothly animated (default: true).
    /// Set to `false` for instant teleportation without animation.
    #[inline]
    pub fn with_animate(mut self, animate: bool) -> Self {
        self.animate = Some(animate);
        self
    }

    /// Returns whether animation is enabled (defaults to true if None).
    #[inline]
    pub fn is_animated(&self) -> bool {
        self.animate.unwrap_or(true)
    }
}

impl From<f32> for GoToOptions {
    #[inline]
    fn from(distance: f32) -> Self {
        Self::distance(distance)
    }
}

impl From<f64> for GoToOptions {
    #[inline]
    fn from(distance: f64) -> Self {
        Self::distance(distance as f32)
    }
}

/// Helper trait allowing `goto` to accept `None`, `GoToOptions`, `Option<GoToOptions>`, or numeric distances.
pub trait IntoGoToOptions {
    fn into_goto_options(self) -> Option<GoToOptions>;
}

impl IntoGoToOptions for Option<GoToOptions> {
    #[inline]
    fn into_goto_options(self) -> Option<GoToOptions> {
        self
    }
}

impl IntoGoToOptions for GoToOptions {
    #[inline]
    fn into_goto_options(self) -> Option<GoToOptions> {
        Some(self)
    }
}

impl IntoGoToOptions for f32 {
    #[inline]
    fn into_goto_options(self) -> Option<GoToOptions> {
        Some(GoToOptions::distance(self))
    }
}

impl IntoGoToOptions for f64 {
    #[inline]
    fn into_goto_options(self) -> Option<GoToOptions> {
        Some(GoToOptions::distance(self as f32))
    }
}

impl IntoGoToOptions for () {
    #[inline]
    fn into_goto_options(self) -> Option<GoToOptions> {
        None
    }
}
