use crate::gis::crs::ProjectOrigin;
use crate::gis::extrusion::RawMeshData;
use crate::solar::datetime_state::SolarDateTimeState;
use crate::solar::sun_calc::calculate_solar_position;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct ShadowProbeResult {
    pub point: Vec3,
    pub is_in_shadow: bool,
    pub is_daylight: bool,
    pub solar_elevation_deg: f32,
    pub solar_azimuth_deg: f32,
    pub occluding_feature: Option<String>,
    pub direct_sunlight_hours: f32,
    pub total_analyzed_hours: f32,
    pub shadow_fraction: f32,
}

/// Simple triangle representation for analytical ray casting
#[derive(Debug, Clone)]
pub struct ColliderTriangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub feature_id: String,
}

impl ColliderTriangle {
    /// Möller–Trumbore ray-triangle intersection
    pub fn intersect_ray(&self, ray_origin: Vec3, ray_dir: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let h = ray_dir.cross(edge2);
        let a = edge1.dot(h);

        if a.abs() < 1e-6 {
            return None; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = ray_origin - self.v0;
        let u = f * s.dot(h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * ray_dir.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);
        if t >= t_min && t <= t_max {
            Some(t)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshCollider {
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub feature_id: String,
    pub triangles: Vec<ColliderTriangle>,
}

impl MeshCollider {
    /// Fast slab ray-AABB intersection test
    pub fn intersect_ray_aabb(&self, ray_origin: Vec3, inv_ray_dir: Vec3, max_t: f32) -> bool {
        let t1 = (self.aabb_min.x - ray_origin.x) * inv_ray_dir.x;
        let t2 = (self.aabb_max.x - ray_origin.x) * inv_ray_dir.x;
        let t3 = (self.aabb_min.y - ray_origin.y) * inv_ray_dir.y;
        let t4 = (self.aabb_max.y - ray_origin.y) * inv_ray_dir.y;
        let t5 = (self.aabb_min.z - ray_origin.z) * inv_ray_dir.z;
        let t6 = (self.aabb_max.z - ray_origin.z) * inv_ray_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        if tmax < 0.0 || tmin > tmax || tmin > max_t {
            return false;
        }
        true
    }
}

/// Scene collider mesh storage for analytical ray queries with AABB acceleration
#[derive(Debug, Clone, Default)]
pub struct SceneCollider {
    pub mesh_colliders: Vec<MeshCollider>,
}

impl SceneCollider {
    pub fn clear(&mut self) {
        self.mesh_colliders.clear();
    }

    pub fn remove_feature(&mut self, feature_id: &str) {
        self.mesh_colliders.retain(|m| m.feature_id != feature_id);
    }

    pub fn remove_features_with_prefix(&mut self, prefix: &str) {
        self.mesh_colliders.retain(|m| !m.feature_id.starts_with(prefix));
    }

    pub fn add_mesh(&mut self, mesh: &RawMeshData, feature_id: &str) {
        let mut aabb_min = Vec3::splat(f32::INFINITY);
        let mut aabb_max = Vec3::splat(f32::NEG_INFINITY);
        let mut triangles = Vec::with_capacity(mesh.indices.len() / 3);

        for chunk in mesh.indices.chunks_exact(3) {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;

            if i0 < mesh.positions.len() && i1 < mesh.positions.len() && i2 < mesh.positions.len() {
                let v0 = Vec3::from_array(mesh.positions[i0]);
                let v1 = Vec3::from_array(mesh.positions[i1]);
                let v2 = Vec3::from_array(mesh.positions[i2]);

                aabb_min = aabb_min.min(v0).min(v1).min(v2);
                aabb_max = aabb_max.max(v0).max(v1).max(v2);

                triangles.push(ColliderTriangle {
                    v0,
                    v1,
                    v2,
                    feature_id: feature_id.to_string(),
                });
            }
        }

        if !triangles.is_empty() {
            self.mesh_colliders.push(MeshCollider {
                aabb_min,
                aabb_max,
                feature_id: feature_id.to_string(),
                triangles,
            });
        }
    }

    /// Add arbitrary triangle mesh geometry from 3D position vectors and index buffer
    pub fn add_positions_and_indices(&mut self, positions: &[Vec3], indices: &[u32], feature_id: &str) {
        let mut aabb_min = Vec3::splat(f32::INFINITY);
        let mut aabb_max = Vec3::splat(f32::NEG_INFINITY);
        let mut triangles = Vec::with_capacity(indices.len() / 3);

        for chunk in indices.chunks_exact(3) {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;

            if i0 < positions.len() && i1 < positions.len() && i2 < positions.len() {
                let v0 = positions[i0];
                let v1 = positions[i1];
                let v2 = positions[i2];

                aabb_min = aabb_min.min(v0).min(v1).min(v2);
                aabb_max = aabb_max.max(v0).max(v1).max(v2);

                triangles.push(ColliderTriangle {
                    v0,
                    v1,
                    v2,
                    feature_id: feature_id.to_string(),
                });
            }
        }

        if !triangles.is_empty() {
            self.mesh_colliders.push(MeshCollider {
                aabb_min,
                aabb_max,
                feature_id: feature_id.to_string(),
                triangles,
            });
        }
    }

    /// Add a decoded OGC 3D Tiles mesh to the scene collider for raycasting, selection, and spatial analysis
    pub fn add_threedtile_mesh(&mut self, mesh: &crate::gis::threedtiles::manager::DecodedThreeDTileMesh, feature_id: &str) {
        if mesh.features.is_empty() {
            self.add_positions_and_indices(&mesh.positions, &mesh.indices, feature_id);
        } else {
            for (idx, feat) in mesh.features.iter().enumerate() {
                let sub_id = format!("{}_bldg_{}", feature_id, idx);
                let pos_vecs: Vec<Vec3> = feat.raw_mesh.positions.iter().map(|p| Vec3::from_array(*p)).collect();
                self.add_positions_and_indices(&pos_vecs, &feat.raw_mesh.indices, &sub_id);
            }
        }
    }

    /// Cast a shadow ray towards the sun to test if a point is in shadow
    pub fn is_point_occluded(&self, point: Vec3, sun_dir: Vec3, max_dist: f32) -> Option<String> {
        if sun_dir.y <= 0.0 {
            return Some("Sun is below horizon".to_string()); // Night time / below horizon
        }

        let ray_origin = point + Vec3::new(0.0, 0.05, 0.0); // Epsilon offset above surface
        let inv_sun_dir = Vec3::new(
            if sun_dir.x.abs() > 1e-6 { 1.0 / sun_dir.x } else { 1e6 },
            if sun_dir.y.abs() > 1e-6 { 1.0 / sun_dir.y } else { 1e6 },
            if sun_dir.z.abs() > 1e-6 { 1.0 / sun_dir.z } else { 1e6 },
        );

        let mut closest_t = f32::INFINITY;
        let mut occluder = None;

        for mesh in &self.mesh_colliders {
            if !mesh.intersect_ray_aabb(ray_origin, inv_sun_dir, max_dist) {
                continue;
            }

            for tri in &mesh.triangles {
                if let Some(t) = tri.intersect_ray(ray_origin, sun_dir, 0.01, max_dist) {
                    if t < closest_t {
                        closest_t = t;
                        occluder = Some(tri.feature_id.clone());
                    }
                }
            }
        }

        occluder
    }

    /// Cast an arbitrary line segment from p0 to p1 to find the first collision with scene meshes.
    /// Returns Some((hit_distance_from_p0, hit_point_3d, feature_id)) if obstructed.
    pub fn cast_ray_segment(&self, p0: Vec3, p1: Vec3) -> Option<(f32, Vec3, String)> {
        let diff = p1 - p0;
        let max_dist = diff.length();
        if max_dist < 0.1 {
            return None;
        }

        let dir = diff / max_dist;
        let inv_dir = Vec3::new(
            if dir.x.abs() > 1e-6 { 1.0 / dir.x } else { 1e6 },
            if dir.y.abs() > 1e-6 { 1.0 / dir.y } else { 1e6 },
            if dir.z.abs() > 1e-6 { 1.0 / dir.z } else { 1e6 },
        );

        let mut closest_t = f32::INFINITY;
        let mut hit_feature = None;

        // Skip 5cm near start and end to avoid self-intersecting observer/target surfaces
        let t_min = 0.05f32;
        let t_max = (max_dist - 0.05).max(t_min);

        for mesh in &self.mesh_colliders {
            if !mesh.intersect_ray_aabb(p0, inv_dir, t_max) {
                continue;
            }

            for tri in &mesh.triangles {
                if let Some(t) = tri.intersect_ray(p0, dir, t_min, t_max) {
                    if t < closest_t {
                        closest_t = t;
                        hit_feature = Some(tri.feature_id.clone());
                    }
                }
            }
        }

        if let Some(feat_id) = hit_feature {
            let hit_pt = p0 + dir * closest_t;
            Some((closest_t, hit_pt, feat_id))
        } else {
            None
        }
    }

    /// Sample the top building elevation (highest collision Y) at coordinates (x, z)
    pub fn sample_building_height_at(&self, x: f32, z: f32) -> Option<(f32, String)> {
        let top_origin = Vec3::new(x, 2000.0, z);
        let down_dir = Vec3::new(0.0, -1.0, 0.0);
        let max_dist = 2500.0;

        let inv_dir = Vec3::new(1e6, -1.0, 1e6);
        let mut highest_y = f32::NEG_INFINITY;
        let mut top_feature = None;

        for mesh in &self.mesh_colliders {
            if !mesh.intersect_ray_aabb(top_origin, inv_dir, max_dist) {
                continue;
            }

            for tri in &mesh.triangles {
                if let Some(t) = tri.intersect_ray(top_origin, down_dir, 0.0, max_dist) {
                    let hit_y = top_origin.y - t;
                    if hit_y > highest_y {
                        highest_y = hit_y;
                        top_feature = Some(tri.feature_id.clone());
                    }
                }
            }
        }

        if let Some(feat_id) = top_feature {
            Some((highest_y, feat_id))
        } else {
            None
        }
    }

    /// Perform comprehensive shadow and daily sunlight exposure analysis for a given point
    pub fn analyze_point_shadow(
        &self,
        point: Vec3,
        origin: &ProjectOrigin,
        dt_state: &SolarDateTimeState,
    ) -> ShadowProbeResult {
        let utc_dt = dt_state.to_utc_datetime();
        let current_sun = calculate_solar_position(
            origin.origin.latitude,
            origin.origin.longitude,
            &utc_dt,
            dt_state.timezone_offset_hours,
        );

        let occluder = self.is_point_occluded(point, current_sun.sun_direction, 1000.0);
        let is_in_shadow = occluder.is_some() || !current_sun.is_daylight;

        // Daily sunlight exposure simulation (steps from 06:00 to 18:00 every 30 minutes)
        let mut sim_dt = dt_state.clone();
        let mut sun_steps = 0;
        let mut daylight_steps = 0;
        let total_steps = 24; // 12 hours with 30 min intervals

        for step in 0..total_steps {
            let hour_f = 6.0 + (step as f32) * 0.5;
            sim_dt.set_time_of_day_fractional(hour_f);
            let step_utc = sim_dt.to_utc_datetime();
            let step_sun = calculate_solar_position(
                origin.origin.latitude,
                origin.origin.longitude,
                &step_utc,
                sim_dt.timezone_offset_hours,
            );

            if step_sun.is_daylight {
                daylight_steps += 1;
                if self.is_point_occluded(point, step_sun.sun_direction, 1000.0).is_none() {
                    sun_steps += 1;
                }
            }
        }

        let direct_sunlight_hours = (sun_steps as f32) * 0.5;
        let total_analyzed_hours = (daylight_steps as f32) * 0.5;
        let shadow_fraction = if daylight_steps > 0 {
            1.0 - (sun_steps as f32 / daylight_steps as f32)
        } else {
            1.0
        };

        ShadowProbeResult {
            point,
            is_in_shadow,
            is_daylight: current_sun.is_daylight,
            solar_elevation_deg: current_sun.elevation_deg,
            solar_azimuth_deg: current_sun.azimuth_deg,
            occluding_feature: occluder,
            direct_sunlight_hours,
            total_analyzed_hours,
            shadow_fraction,
        }
    }
}

