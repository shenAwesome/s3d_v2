use crate::gis::crs::ProjectOrigin;
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementMode {
    Select,
    Distance,
    Area,
    Height,
    ShadowProbe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeasurementResult {
    Distance {
        p1: Vec3,
        p2: Vec3,
        distance_3d: f32,
        horizontal_dist: f32,
        vertical_diff: f32,
        geodesic_dist_m: f64,
    },
    Area {
        points: Vec<Vec3>,
        area_m2: f64,
        perimeter_m: f32,
    },
    Height {
        bottom: Vec3,
        top: Vec3,
        height_m: f32,
    },
}

#[derive(Debug, Clone, Default)]
pub struct MeasurementEngine {
    pub active_mode: Option<MeasurementMode>,
    pub points: Vec<Vec3>,
    pub current_result: Option<MeasurementResult>,
}

impl MeasurementEngine {
    pub fn new() -> Self {
        Self {
            active_mode: None,
            points: Vec::new(),
            current_result: None,
        }
    }

    pub fn set_mode(&mut self, mode: Option<MeasurementMode>) {
        self.active_mode = mode;
        self.points.clear();
        self.current_result = None;
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.current_result = None;
    }

    pub fn add_point(&mut self, point: Vec3, origin: &ProjectOrigin) {
        match self.active_mode {
            Some(MeasurementMode::Distance) => {
                self.points.push(point);
                if self.points.len() == 2 {
                    let p1 = self.points[0];
                    let p2 = self.points[1];

                    let d3d = (p2 - p1).length();
                    let d_horiz = ((p2.x - p1.x).powi(2) + (p2.z - p1.z).powi(2)).sqrt();
                    let d_vert = (p2.y - p1.y).abs();

                    let geo1 = origin.local_to_geo(p1);
                    let geo2 = origin.local_to_geo(p2);
                    let geodesic = geo1.distance_to(&geo2);

                    self.current_result = Some(MeasurementResult::Distance {
                        p1,
                        p2,
                        distance_3d: d3d,
                        horizontal_dist: d_horiz,
                        vertical_diff: d_vert,
                        geodesic_dist_m: geodesic,
                    });
                } else if self.points.len() > 2 {
                    self.points = vec![point];
                    self.current_result = None;
                }
            }
            Some(MeasurementMode::Height) => {
                self.points.push(point);
                if self.points.len() == 2 {
                    let mut p1 = self.points[0];
                    let mut p2 = self.points[1];
                    if p1.y > p2.y {
                        std::mem::swap(&mut p1, &mut p2);
                    }
                    let height = (p2.y - p1.y).abs();

                    self.current_result = Some(MeasurementResult::Height {
                        bottom: p1,
                        top: p2,
                        height_m: height,
                    });
                } else if self.points.len() > 2 {
                    self.points = vec![point];
                    self.current_result = None;
                }
            }
            Some(MeasurementMode::Area) => {
                self.points.push(point);
                if self.points.len() >= 3 {
                    let area = calculate_polygon_area_2d(&self.points);
                    let perimeter = calculate_polygon_perimeter_2d(&self.points);
                    self.current_result = Some(MeasurementResult::Area {
                        points: self.points.clone(),
                        area_m2: area,
                        perimeter_m: perimeter,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Calculate 2D polygon area on horizontal XZ plane using Shoelace formula.
/// Returns area in f64 for VCAT-grade accuracy (±0.01 m²).
pub fn calculate_polygon_area_2d(points: &[Vec3]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }

    let mut area = 0.0f64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += (points[i].x as f64) * (points[j].z as f64);
        area -= (points[j].x as f64) * (points[i].z as f64);
    }

    area.abs() * 0.5
}

/// Calculate 2D perimeter length
pub fn calculate_polygon_perimeter_2d(points: &[Vec3]) -> f32 {
    let n = points.len();
    if n < 2 {
        return 0.0;
    }

    let mut len = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        let dx = points[j].x - points[i].x;
        let dz = points[j].z - points[i].z;
        len += (dx * dx + dz * dz).sqrt();
    }

    len
}

