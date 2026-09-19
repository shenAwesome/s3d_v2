use crate::solar::shadow_analysis::SceneCollider;
use glam::{Mat4, Vec3, Vec4};

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
        }
    }

    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// Intersect horizontal ground plane at y = elevation
    pub fn intersect_ground_plane(&self, ground_elevation: f32) -> Option<Vec3> {
        if self.direction.y.abs() < 1e-6 {
            return None;
        }

        let t = (ground_elevation - self.origin.y) / self.direction.y;
        if t >= 0.0 {
            Some(self.point_at(t))
        } else {
            None
        }
    }

    /// Intersect arbitrary continuous height field H(x, z) using guaranteed-convergent interval bisection
    pub fn intersect_terrain_bisection<F>(&self, height_fn: F, floor_y: f32) -> Option<Vec3>
    where
        F: Fn(f32, f32) -> f32,
    {
        if self.direction.y >= -1e-4 {
            return None;
        }

        let t_min = 0.0f32;
        let t_max = ((floor_y - self.origin.y) / self.direction.y).max(t_min + 1.0);

        let mut t_lo = t_min;
        let mut t_hi = t_max;

        for _ in 0..24 {
            let t_mid = (t_lo + t_hi) * 0.5;
            let pt = self.point_at(t_mid);
            let terrain_y = height_fn(pt.x, pt.z);
            if pt.y > terrain_y {
                t_lo = t_mid;
            } else {
                t_hi = t_mid;
            }
        }

        let final_t = (t_lo + t_hi) * 0.5;
        Some(self.point_at(final_t))
    }

    /// Intersect 3D sphere centered at `center` with `radius` (returns near front hit point)
    pub fn intersect_sphere(&self, center: Vec3, radius: f32) -> Option<Vec3> {
        let oc = self.origin - center;
        let b = oc.dot(self.direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - c;

        if discriminant < 0.0 {
            return None;
        }

        let sqrt_d = discriminant.sqrt();
        let t1 = -b - sqrt_d;
        let t2 = -b + sqrt_d;

        let t = if t1 > 0.0 {
            t1
        } else if t2 > 0.0 {
            t2
        } else {
            return None;
        };

        Some(self.point_at(t))
    }
}

#[derive(Debug, Clone)]
pub struct PickResult {
    pub point: Vec3,
    pub distance: f32,
    pub feature_id: Option<String>,
}

/// Compute 3D ray from 2D screen coordinates
pub fn screen_to_ray(
    screen_pos: glam::Vec2,
    viewport_size: glam::Vec2,
    view_proj: Mat4,
) -> Ray {
    let ndc_x = (2.0 * screen_pos.x / viewport_size.x) - 1.0;
    let ndc_y = 1.0 - (2.0 * screen_pos.y / viewport_size.y);

    let inv_view_proj = view_proj.inverse();

    let near_clip = Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_clip = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);

    let mut near_world = inv_view_proj * near_clip;
    near_world /= near_world.w;

    let mut far_world = inv_view_proj * far_clip;
    far_world /= far_world.w;

    let origin = Vec3::new(near_world.x, near_world.y, near_world.z);
    let direction = (Vec3::new(far_world.x, far_world.y, far_world.z) - origin).normalize_or_zero();

    Ray::new(origin, direction)
}

/// Pick closest object or ground in the scene with AABB spatial pre-filtering
pub fn pick_scene(
    ray: &Ray,
    collider: &SceneCollider,
    ground_elevation: f32,
) -> Option<PickResult> {
    let mut closest_hit: Option<PickResult> = None;
    let mut closest_t = f32::INFINITY;

    let inv_ray_dir = Vec3::new(
        if ray.direction.x.abs() > 1e-6 { 1.0 / ray.direction.x } else { 1e6 },
        if ray.direction.y.abs() > 1e-6 { 1.0 / ray.direction.y } else { 1e6 },
        if ray.direction.z.abs() > 1e-6 { 1.0 / ray.direction.z } else { 1e6 },
    );

    // Test mesh colliders with AABB pre-culling
    for mesh in &collider.mesh_colliders {
        if !mesh.intersect_ray_aabb(ray.origin, inv_ray_dir, closest_t) {
            continue;
        }

        for tri in &mesh.triangles {
            if let Some(t) = tri.intersect_ray(ray.origin, ray.direction, 0.01, closest_t) {
                closest_t = t;
                closest_hit = Some(PickResult {
                    point: ray.point_at(t),
                    distance: t,
                    feature_id: Some(tri.feature_id.clone()),
                });
            }
        }
    }

    // Also test ground plane if no building hit closer
    if let Some(ground_pt) = ray.intersect_ground_plane(ground_elevation) {
        let t_ground = (ground_pt - ray.origin).length();
        if t_ground < closest_t {
            closest_hit = Some(PickResult {
                point: ground_pt,
                distance: t_ground,
                feature_id: None,
            });
        }
    }

    closest_hit
}

