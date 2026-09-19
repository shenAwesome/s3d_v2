use crate::gis::crs::ProjectOrigin;
use crate::scene::node::SceneNode;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct Scene {
    pub origin: ProjectOrigin,
    pub nodes: Vec<SceneNode>,
    pub selected_node_id: Option<String>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl Scene {
    pub fn new(origin: ProjectOrigin) -> Self {
        Self {
            origin,
            nodes: Vec::new(),
            selected_node_id: None,
            bounds_min: Vec3::splat(-100.0),
            bounds_max: Vec3::splat(100.0),
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.selected_node_id = None;
        self.bounds_min = Vec3::splat(-100.0);
        self.bounds_max = Vec3::splat(100.0);
    }

    pub fn update_bounds(&mut self) {
        if self.nodes.is_empty() {
            self.bounds_min = Vec3::splat(-100.0);
            self.bounds_max = Vec3::splat(100.0);
            return;
        }

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for node in &self.nodes {
            if node.is_visible {
                min = min.min(node.aabb_min);
                max = max.max(node.aabb_max);
            }
        }

        if min.x.is_finite() && max.x.is_finite() {
            // Expand slightly
            let padding = Vec3::new(20.0, 5.0, 20.0);
            self.bounds_min = min - padding;
            self.bounds_max = max + padding;
        }
    }

    pub fn get_center(&self) -> Vec3 {
        (self.bounds_min + self.bounds_max) * 0.5
    }

    pub fn get_radius(&self) -> f32 {
        (self.bounds_max - self.bounds_min).length() * 0.5
    }
}
