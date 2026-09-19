use crate::scene::material::Material;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: String,
    pub name: String,
    pub layer_id: String,
    pub feature_id: Option<String>,
    pub transform: Mat4,
    pub material: Material,
    pub mesh_index: usize,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub is_visible: bool,
    pub shadow_color: [f32; 4],
}

impl SceneNode {
    pub fn new(
        id: String,
        name: String,
        layer_id: String,
        feature_id: Option<String>,
        mesh_index: usize,
        aabb_min: Vec3,
        aabb_max: Vec3,
    ) -> Self {
        Self {
            id,
            name,
            layer_id,
            feature_id,
            transform: Mat4::IDENTITY,
            material: Material::building_default(),
            mesh_index,
            aabb_min,
            aabb_max,
            is_visible: true,
            shadow_color: crate::gis::layer::DEFAULT_SHADOW_COLOR,
        }
    }
}
