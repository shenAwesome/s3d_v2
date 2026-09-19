use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub base_color: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [0.82, 0.84, 0.88, 1.0], // Clean architectural concrete / off-white
            roughness: 0.5,
            metallic: 0.05,
        }
    }
}

impl Material {
    pub fn building_default() -> Self {
        Self {
            base_color: [0.85, 0.86, 0.90, 1.0],
            roughness: 0.45,
            metallic: 0.1,
        }
    }

    pub fn building_selected() -> Self {
        Self {
            base_color: [0.25, 0.65, 1.0, 1.0], // Electric blue highlight
            roughness: 0.3,
            metallic: 0.2,
        }
    }

    pub fn ground_grid() -> Self {
        Self {
            base_color: [0.18, 0.20, 0.24, 1.0], // Dark canvas ground
            roughness: 0.9,
            metallic: 0.0,
        }
    }

    pub fn measurement_line() -> Self {
        Self {
            base_color: [1.0, 0.75, 0.1, 1.0], // Bright amber
            roughness: 0.2,
            metallic: 0.0,
        }
    }

    pub fn shadow_probe_sun() -> Self {
        Self {
            base_color: [0.2, 0.9, 0.3, 1.0], // Green for sunlit
            roughness: 0.2,
            metallic: 0.0,
        }
    }

    pub fn shadow_probe_shadow() -> Self {
        Self {
            base_color: [0.95, 0.25, 0.25, 1.0], // Red for shadowed
            roughness: 0.2,
            metallic: 0.0,
        }
    }
}
