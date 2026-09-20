use glam::{Mat4, Vec3};
use crate::gis::crs::{GeoCoord, ProjectOrigin};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,       // Radians around Y axis
    pub pitch: f32,     // Radians above XZ plane
    pub distance: f32,  // Distance from target
    pub fov_y: f32,     // Vertical FOV in radians
    pub z_near: f32,
    pub z_far: f32,
    pub target_distance: f32, // Smooth interpolation target for distance
    pub target_lookat: Vec3,  // Smooth interpolation target for look-at point
    pub target_yaw: f32,      // Smooth interpolation target for yaw
    pub target_pitch: f32,    // Smooth interpolation target for pitch
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.0, // 0.0 deg looking North
            pitch: std::f32::consts::FRAC_PI_6, // 30 deg above ground
            distance: 250.0,
            fov_y: 45.0f32.to_radians(),
            z_near: 1.0,
            z_far: 10000.0,
            target_distance: 250.0,
            target_lookat: Vec3::ZERO,
            target_yaw: 0.0,
            target_pitch: std::f32::consts::FRAC_PI_6,
        }
    }
}

/// Target parameters for positioning and orienting the camera (matching Esri SceneView camera options).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTarget {
    pub latitude: f64,
    pub longitude: f64,
    pub distance: f32,
    /// Compass heading in degrees (0.0 = North, 90.0 = East, 180.0 = South, 270.0 = West). Default: 0.0 (North).
    pub heading: f32,
    /// Tilt angle in degrees (0.0 = top-down 2D nadir view, 45.0 = 3D oblique perspective, 80.0 = horizon). Default: 0.0.
    pub tilt: f32,
    /// Optional direct local 3D target focus point (e.g. for planar coordinates). If None, latitude/longitude is used.
    pub target_point: Option<Vec3>,
}

impl Default for CameraTarget {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            distance: 1000.0,
            heading: 0.0,
            tilt: 0.0,
            target_point: None,
        }
    }
}

impl Camera {
    pub fn new(target: Vec3, distance: f32) -> Self {
        Self {
            target,
            distance,
            target_distance: distance,
            target_lookat: target,
            target_yaw: 0.0,
            target_pitch: std::f32::consts::FRAC_PI_6,
            ..Default::default()
        }
    }

    /// Zooms the camera to look at a 3D local target point at a specified viewing distance (in meters).
    pub fn zoom_to(&mut self, target: Vec3, distance: f32) {
        self.target = target;
        self.target_lookat = target;
        self.target_distance = distance.max(1.0);
        self.distance = self.target_distance;
    }

    /// Zooms the camera to a geographic target with specified distance, heading, and tilt (in degrees).
    /// Heading: 0° = North, 90° = East.
    /// Tilt: 0° = straight top-down nadir (pitch ~89°/pi/2), 45° = 3D oblique perspective (pitch 45°), 80° = near horizon.
    pub fn zoom_to_target(&mut self, origin: &ProjectOrigin, target: CameraTarget) {
        let local = if let Some(pt) = target.target_point {
            pt
        } else {
            origin.lat_lon_to_local(target.latitude, target.longitude, 0.0)
        };
        self.target = local;
        self.target_lookat = local;
        self.target_distance = target.distance.max(1.0);
        self.distance = self.target_distance;

        // Convert heading (0° North, 90° East) to camera yaw (radians)
        // In our coordinate system: yaw = 0 is North (looking towards -Z), yaw > 0 turns CCW / East
        self.yaw = target.heading.to_radians();
        self.normalize_yaw();
        self.target_yaw = self.yaw;

        // Convert tilt (0° top-down nadir, 90° horizon) to camera pitch (radians above ground)
        // Pitch: ~1.54 rad (~88.5°) is top-down, ~0.035 rad is horizon
        let pitch_deg = (90.0 - target.tilt).clamp(2.0, 88.5);
        self.pitch = pitch_deg.to_radians();
        self.target_pitch = self.pitch;
    }

    /// Zooms and re-centers the camera on a geographic coordinate (WGS84 lat, lon, elev)
    /// relative to the given project origin, setting the viewing distance in meters.
    pub fn zoom_to_geo(
        &mut self,
        origin: &ProjectOrigin,
        lat: f64,
        lon: f64,
        elevation: f64,
        distance: f32,
    ) {
        let local = origin.lat_lon_to_local(lat, lon, elevation);
        self.zoom_to(local, distance);
    }

    /// Computes the current geographic coordinate (lat, lon, elev) of the camera's target focus point.
    pub fn target_geo(&self, origin: &ProjectOrigin) -> GeoCoord {
        origin.local_to_geo(self.target)
    }

    pub fn eye_position(&self) -> Vec3 {
        let cos_pitch = self.pitch.cos();
        let sin_pitch = self.pitch.sin();
        let cos_yaw = self.yaw.cos();
        let sin_yaw = self.yaw.sin();

        let offset = Vec3::new(
            self.distance * cos_pitch * sin_yaw,
            self.distance * sin_pitch,
            self.distance * cos_pitch * cos_yaw,
        );

        self.target + offset
    }

    /// Returns the orthonormal camera basis vectors `(right, up, forward)` for the current camera orientation
    pub fn basis_vectors(&self) -> (Vec3, Vec3, Vec3) {
        Self::basis_vectors_for(self.yaw, self.pitch)
    }

    /// Returns the orthonormal camera basis vectors `(right, up, forward)` for arbitrary `(yaw, pitch)`
    pub fn basis_vectors_for(yaw: f32, pitch: f32) -> (Vec3, Vec3, Vec3) {
        let cos_pitch = pitch.cos();
        let sin_pitch = pitch.sin();
        let cos_yaw = yaw.cos();
        let sin_yaw = yaw.sin();

        let forward = Vec3::new(
            -cos_pitch * sin_yaw,
            -sin_pitch,
            -cos_pitch * cos_yaw,
        ).normalize_or_zero();

        let right = Vec3::new(cos_yaw, 0.0, -sin_yaw);
        let up = right.cross(forward).normalize_or_zero();
        (right, if up.length_squared() > 0.5 { up } else { Vec3::Y }, forward)
    }

    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.eye_position();
        let (_right, up, _forward) = self.basis_vectors();

        Mat4::look_at_rh(eye, self.target, if up.length_squared() > 0.5 { up } else { Vec3::Y })
    }

    pub fn proj_matrix(&self, aspect_ratio: f32) -> Mat4 {
        const WGS84_RADIUS: f32 = 6_378_137.0;
        let (dynamic_znear, dynamic_zfar) = if self.distance > WGS84_RADIUS {
            // Globe / Orbital space: compute actual altitude above Earth surface
            let alt = (self.distance - WGS84_RADIUS).max(5.0);
            let znear = (alt * 0.01).clamp(0.5, 50_000.0);
            let zfar = (self.distance * 4.0).clamp(100_000.0, 200_000_000.0);
            (znear, zfar)
        } else {
            // Planar / Local architectural scale: when tilted towards horizon, extend far plane to Earth horizon (~120-150 km)
            let znear = (self.distance * 0.005).clamp(0.5, 25.0);
            let sin_pitch = self.pitch.abs().sin().clamp(0.08, 1.0);
            let horizon_extent = (15_000.0 / sin_pitch).clamp(35_000.0, 150_000.0);
            let zfar = (self.distance * (25.0 / sin_pitch)).clamp(horizon_extent, 200_000.0);
            (znear, zfar)
        };
        Mat4::perspective_rh(self.fov_y, aspect_ratio.max(0.01), dynamic_znear, dynamic_zfar)
    }

    pub const MIN_PITCH_PLANAR: f32 = 0.035; // ~2.0 degrees above horizon
    pub const MAX_PITCH_PLANAR: f32 = 1.553; // ~89.0 degrees (strict top-down, no flipping)

    pub fn view_proj_matrix(&self, aspect_ratio: f32) -> Mat4 {
        self.proj_matrix(aspect_ratio) * self.view_matrix()
    }

    /// Normalizes camera yaw to [-PI, PI]
    pub fn normalize_yaw(&mut self) {
        while self.yaw > std::f32::consts::PI {
            self.yaw -= std::f32::consts::TAU;
        }
        while self.yaw < -std::f32::consts::PI {
            self.yaw += std::f32::consts::TAU;
        }
    }

    /// Orbit (Rotate & Tilt) around target
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        let sensitivity = 0.005;
        self.yaw -= delta_x * sensitivity;
        self.normalize_yaw();
        self.pitch = (self.pitch + delta_y * sensitivity).clamp(-1.54, 1.54);
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
    }

    /// Orbit in Planar ground mode (constrained between horizon ~2° and top-down ~89°, stays top-down without flipping)
    pub fn orbit_planar(&mut self, delta_x: f32, delta_y: f32) {
        let sensitivity = 0.005;
        self.yaw -= delta_x * sensitivity;
        self.normalize_yaw();
        self.pitch = (self.pitch + delta_y * sensitivity).clamp(Self::MIN_PITCH_PLANAR, Self::MAX_PITCH_PLANAR);
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
    }

    /// Orbit in Planar mode around an arbitrary 3D anchor point on the ground/building
    /// Smoothly rotates around anchor and tilts with strict clamping at top-down, staying top-down without flipping!
    pub fn orbit_around_point_planar(&mut self, anchor: Vec3, delta_x: f32, delta_y: f32) {
        let sensitivity = 0.005;
        let delta_yaw = -delta_x * sensitivity;
        let delta_pitch = delta_y * sensitivity;

        // 1. Yaw rotation around vertical axis passing through anchor
        if delta_yaw.abs() > 1e-6 {
            let rot_y = Mat4::from_rotation_y(delta_yaw);
            self.target = anchor + rot_y.transform_vector3(self.target - anchor);
            self.target_lookat = self.target;
            self.yaw += delta_yaw;
            self.normalize_yaw();
            self.target_yaw = self.yaw;
        }

        // 2. Pitch rotation with strict clamping at top-down (max 89.0°) and horizon (min 2.0°)
        // Tilting up stops smoothly at top-down and STAYS top-down without flipping or inverting!
        self.pitch = (self.pitch + delta_pitch).clamp(Self::MIN_PITCH_PLANAR, Self::MAX_PITCH_PLANAR);
        self.target_pitch = self.pitch;
    }

    /// Orbit (Rotate & Tilt) around an arbitrary 3D anchor point without any visual jump
    pub fn orbit_around_point(&mut self, anchor: Vec3, delta_x: f32, delta_y: f32) {
        self.orbit_around_point_planar(anchor, delta_x, delta_y);
    }

    /// Pan camera across ground plane (Esri Web Map standard)
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let pan_speed = (self.distance * 0.0018).max(0.05);

        let cos_yaw = self.yaw.cos();
        let sin_yaw = self.yaw.sin();

        // Right vector on ground plane (perpendicular to view direction)
        let right = Vec3::new(cos_yaw, 0.0, -sin_yaw);
        // Forward vector on ground plane (in view direction towards target)
        let forward_ground = Vec3::new(-sin_yaw, 0.0, -cos_yaw);

        let delta = (-right * delta_x + forward_ground * delta_y) * pan_speed;
        self.target += delta;
        self.target_lookat += delta;
    }

    /// Snaps all smooth interpolation targets immediately to current values
    pub fn snap_smoothing(&mut self) {
        self.target_distance = self.distance;
        self.target_lookat = self.target;
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
    }

    /// Snaps current camera state immediately to target values (instantly completes any ongoing animation)
    pub fn snap_to_target(&mut self) {
        self.distance = self.target_distance;
        self.target = self.target_lookat;
        self.yaw = self.target_yaw;
        self.pitch = self.target_pitch;
    }

    /// Zoom in / out smoothly (updates target distance)
    pub fn zoom(&mut self, delta: f32) {
        let factor = (1.0 - delta * 0.0015).clamp(0.1, 3.0);
        self.target_distance = (self.target_distance * factor).clamp(2.0, 100_000_000.0);
    }

    /// Zoom in Globe Mode: smoothly scales camera altitude above Earth surface while keeping globe centered
    pub fn zoom_globe(&mut self, delta: f32) {
        const WGS84_RADIUS: f32 = 6_378_137.0;
        let cur_alt = (self.target_distance - WGS84_RADIUS).max(10.0);
        let steps = (delta / 50.0).clamp(-4.0, 4.0);
        let factor = 0.74f32.powf(steps);
        let new_alt = (cur_alt * factor).clamp(10.0, 80_000_000.0);
        self.target_distance = WGS84_RADIUS + new_alt;
        self.target = Vec3::ZERO;
        self.target_lookat = Vec3::ZERO;
    }

    /// Zoom in Planar Mode (smooth zoom from street level up to full world view)
    pub fn zoom_planar(&mut self, delta: f32) {
        let steps = (delta / 50.0).clamp(-4.0, 4.0);
        let factor = 0.74f32.powf(steps);
        self.target_distance = (self.target_distance * factor).clamp(2.0, 80_000_000.0);
    }

    /// Advances smooth zoom damping each frame. Returns true if camera is still animating.
    pub fn update_smooth_zoom(&mut self, dt: f32) -> bool {
        let mut animating = false;
        let decay = 1.0 - (-15.0 * dt).exp();

        // 1. Smooth distance damping with scale-adaptive threshold
        let dist_threshold = (self.target_distance * 1e-5).max(0.01);
        let dist_diff = (self.target_distance - self.distance).abs();
        if dist_diff > dist_threshold {
            let next_distance = self.distance + (self.target_distance - self.distance) * decay;
            if (self.target_distance - next_distance).abs() <= dist_threshold || next_distance == self.distance {
                self.distance = self.target_distance;
            } else {
                self.distance = next_distance;
            }
            animating = true;
        } else if self.distance != self.target_distance {
            self.distance = self.target_distance;
        }

        // 2. Smooth target look-at position damping with scale-adaptive threshold
        let target_threshold = (self.target_lookat.length() * 1e-5).max(0.01);
        let target_diff = (self.target_lookat - self.target).length();
        if target_diff > target_threshold {
            let next_target = self.target + (self.target_lookat - self.target) * decay;
            if (self.target_lookat - next_target).length() <= target_threshold || next_target == self.target {
                self.target = self.target_lookat;
            } else {
                self.target = next_target;
            }
            animating = true;
        } else if self.target != self.target_lookat {
            self.target = self.target_lookat;
        }

        // 3. Smooth yaw damping (shortest angular distance on circle [-PI, PI])
        let mut yaw_diff = self.target_yaw - self.yaw;
        while yaw_diff > std::f32::consts::PI {
            yaw_diff -= std::f32::consts::TAU;
        }
        while yaw_diff < -std::f32::consts::PI {
            yaw_diff += std::f32::consts::TAU;
        }
        if yaw_diff.abs() > 0.0005 {
            self.yaw += yaw_diff * decay;
            self.normalize_yaw();

            let mut remaining = self.target_yaw - self.yaw;
            while remaining > std::f32::consts::PI {
                remaining -= std::f32::consts::TAU;
            }
            while remaining < -std::f32::consts::PI {
                remaining += std::f32::consts::TAU;
            }
            if remaining.abs() <= 0.0005 {
                self.yaw = self.target_yaw;
            }
            animating = true;
        } else if self.yaw != self.target_yaw {
            self.yaw = self.target_yaw;
        }

        // 4. Smooth pitch damping
        let pitch_diff = (self.target_pitch - self.pitch).abs();
        if pitch_diff > 0.0005 {
            self.pitch += (self.target_pitch - self.pitch) * decay;
            if (self.target_pitch - self.pitch).abs() <= 0.0005 {
                self.pitch = self.target_pitch;
            }
            animating = true;
        } else if self.pitch != self.target_pitch {
            self.pitch = self.target_pitch;
        }

        animating
    }

    /// Checks if camera is currently undergoing smooth damping transitions
    pub fn is_animating(&self) -> bool {
        let mut yaw_diff = self.target_yaw - self.yaw;
        while yaw_diff > std::f32::consts::PI {
            yaw_diff -= std::f32::consts::TAU;
        }
        while yaw_diff < -std::f32::consts::PI {
            yaw_diff += std::f32::consts::TAU;
        }

        let dist_threshold = (self.target_distance * 1e-5).max(0.01);
        let target_threshold = (self.target_lookat.length() * 1e-5).max(0.01);

        (self.target_distance - self.distance).abs() > dist_threshold
            || (self.target_lookat - self.target).length() > target_threshold
            || yaw_diff.abs() > 0.0005
            || (self.target_pitch - self.pitch).abs() > 0.0005
    }

    /// Focus on a specific bounding sphere
    pub fn focus_on(&mut self, center: Vec3, radius: f32) {
        self.target = center;
        self.target_lookat = center;
        let dist = (radius * 2.5).max(20.0);
        self.distance = dist;
        self.target_distance = dist;
    }

    /// View preset: Top-Down 2D
    pub fn set_view_top_down(&mut self) {
        self.yaw = 0.0;
        self.pitch = 1.52;
        self.target_yaw = 0.0;
        self.target_pitch = 1.52;
    }

    /// View preset: Isometric 3D
    pub fn set_view_isometric(&mut self) {
        self.yaw = -std::f32::consts::FRAC_PI_4;
        self.pitch = 35.264f32.to_radians();
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
    }

    /// View preset: Looking North
    pub fn set_view_north(&mut self) {
        self.yaw = std::f32::consts::PI; // Looking towards -Z (North)
        self.pitch = std::f32::consts::FRAC_PI_6;
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
    }

    /// View preset: Looking South
    pub fn set_view_south(&mut self) {
        self.yaw = 0.0; // Looking towards +Z (South)
        self.pitch = std::f32::consts::FRAC_PI_6;
        self.target_yaw = self.yaw;
        self.target_pitch = self.pitch;
    }
}

/// Smooth flight animation for spinning the globe and transitioning to Planar CAD mode
#[derive(Debug, Clone)]
pub struct GlobeFlightState {
    pub target_geo: crate::gis::crs::GeoCoord,
    pub target_planar_dist: f32,
    pub phase: FlightPhase,
}

#[derive(Debug, Clone)]
pub enum FlightPhase {
    /// Phase 1: Rotating the globe along the shortest spherical arc to center target_geo
    Spinning {
        start_pitch: f32,
        start_yaw: f32,
        target_pitch: f32,
        target_yaw_diff: f32,
        start_dist: f32,
        elapsed: f32,
        duration: f32,
    },
    /// Phase 2: Diving from orbit towards the Earth's surface
    Descent {
        start_dist: f32,
        end_dist: f32,
        elapsed: f32,
        duration: f32,
    },
    /// Phase 3: Settling in Planar mode from top-down down to the desired 3D perspective framing
    PlanarLanding {
        start_dist: f32,
        target_dist: f32,
        start_pitch: f32,
        target_pitch: f32,
        start_yaw: f32,
        target_yaw: f32,
        elapsed: f32,
        duration: f32,
    },
}

impl GlobeFlightState {
    pub fn new_spin_and_zoom(
        camera: &Camera,
        target_geo: crate::gis::crs::GeoCoord,
        target_planar_dist: f32,
    ) -> Self {
        let (target_pitch, target_yaw) = crate::gis::crs::geo_to_globe_camera_angles(&target_geo);
        let start_pitch = camera.pitch;
        let start_yaw = camera.yaw;

        // Calculate shortest angular distance around yaw
        let mut target_yaw_diff = target_yaw - start_yaw;
        while target_yaw_diff > std::f32::consts::PI {
            target_yaw_diff -= std::f32::consts::TAU;
        }
        while target_yaw_diff < -std::f32::consts::PI {
            target_yaw_diff += std::f32::consts::TAU;
        }

        let angular_dist = target_yaw_diff.abs() + (target_pitch - start_pitch).abs();
        let spin_duration = (angular_dist * 0.35 + 0.65).clamp(0.7, 1.25);

        Self {
            target_geo,
            target_planar_dist: target_planar_dist.clamp(150.0, 15_000.0),
            phase: FlightPhase::Spinning {
                start_pitch,
                start_yaw,
                target_pitch,
                target_yaw_diff,
                start_dist: camera.distance,
                elapsed: 0.0,
                duration: spin_duration,
            },
        }
    }
}


/// 3D Plane represented in Hessian normal form: dot(normal, p) + distance = 0
#[derive(Debug, Clone, Copy)]
pub struct FrustumPlane {
    pub normal: Vec3,
    pub distance: f32,
}

impl FrustumPlane {
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        let len = (a * a + b * b + c * c).sqrt();
        if len > 1e-6 {
            let inv_len = 1.0 / len;
            Self {
                normal: Vec3::new(a * inv_len, b * inv_len, c * inv_len),
                distance: d * inv_len,
            }
        } else {
            Self {
                normal: Vec3::Y,
                distance: 0.0,
            }
        }
    }

    /// Signed distance from point to plane (positive = inside frustum halfspace)
    #[inline(always)]
    pub fn signed_distance(&self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.distance
    }
}

/// 6-plane Camera View Frustum for 3D Tile Culling & LOD Calculation
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [FrustumPlane; 6], // Left, Right, Bottom, Top, Near, Far
}

impl Frustum {
    /// Extracts the 6 frustum planes from a combined View-Projection matrix (WebGPU depth range [0, 1])
    pub fn from_view_proj(vp: Mat4) -> Self {
        // Extract matrix rows
        let r0 = glam::Vec4::new(vp.x_axis.x, vp.y_axis.x, vp.z_axis.x, vp.w_axis.x);
        let r1 = glam::Vec4::new(vp.x_axis.y, vp.y_axis.y, vp.z_axis.y, vp.w_axis.y);
        let r2 = glam::Vec4::new(vp.x_axis.z, vp.y_axis.z, vp.z_axis.z, vp.w_axis.z);
        let r3 = glam::Vec4::new(vp.x_axis.w, vp.y_axis.w, vp.z_axis.w, vp.w_axis.w);

        // Left:   w + x >= 0
        let left = FrustumPlane::new(r3.x + r0.x, r3.y + r0.y, r3.z + r0.z, r3.w + r0.w);
        // Right:  w - x >= 0
        let right = FrustumPlane::new(r3.x - r0.x, r3.y - r0.y, r3.z - r0.z, r3.w - r0.w);
        // Bottom: w + y >= 0
        let bottom = FrustumPlane::new(r3.x + r1.x, r3.y + r1.y, r3.z + r1.z, r3.w + r1.w);
        // Top:    w - y >= 0
        let top = FrustumPlane::new(r3.x - r1.x, r3.y - r1.y, r3.z - r1.z, r3.w - r1.w);
        // Near (WebGPU 0..1): z >= 0
        let near = FrustumPlane::new(r2.x, r2.y, r2.z, r2.w);
        // Far (WebGPU 0..1):  w - z >= 0
        let far = FrustumPlane::new(r3.x - r2.x, r3.y - r2.y, r3.z - r2.z, r3.w - r2.w);

        Self {
            planes: [left, right, bottom, top, near, far],
        }
    }

    /// Fast Axis-Aligned Bounding Box (AABB) vs Frustum intersection test
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Find the positive vertex (the corner in the direction of the plane normal)
            let px = if plane.normal.x >= 0.0 { max.x } else { min.x };
            let py = if plane.normal.y >= 0.0 { max.y } else { min.y };
            let pz = if plane.normal.z >= 0.0 { max.z } else { min.z };

            if plane.signed_distance(Vec3::new(px, py, pz)) < 0.0 {
                return false; // Entire box is completely outside this plane
            }
        }
        true
    }

    /// Bounding Sphere vs Frustum intersection test
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.signed_distance(center) < -radius {
                return false;
            }
        }
        true
    }

    /// Fast Oriented Bounding Box (OBB) vs Frustum intersection test using Separating Axis Theorem (SAT).
    ///
    /// `center`: center of the OBB in world space.
    /// `axes`: 3 normalized orthogonal axis vectors of the OBB.
    /// `half_size`: half-extents along each axis [hx, hy, hz].
    pub fn intersects_obb(&self, center: Vec3, axes: [Vec3; 3], half_size: Vec3) -> bool {
        for plane in &self.planes {
            let r_eff = half_size.x * plane.normal.dot(axes[0]).abs()
                + half_size.y * plane.normal.dot(axes[1]).abs()
                + half_size.z * plane.normal.dot(axes[2]).abs();
            if plane.signed_distance(center) < -r_eff {
                return false;
            }
        }
        true
    }
}

