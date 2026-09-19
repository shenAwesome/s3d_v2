use crate::compute::parallel::*;
use crate::gis::crs::ProjectOrigin;
use crate::solar::datetime_state::SolarDateTimeState;
use crate::solar::shadow_analysis::SceneCollider;
use crate::solar::sun_calc::calculate_solar_position;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolboxToolMode {
    None,
    LineOfSight,
    ElevationProfile,
    Viewshed,
    SkyViewFactor,
    ShadowImpact,
    CutAndFill,
    /// Multi-hour Shadow Range Union: accumulates shadow footprints 9am–3pm
    ShadowRangeUnion,
}

impl Default for ToolboxToolMode {
    fn default() -> Self {
        Self::None
    }
}

// ==========================================
// 1. Line of Sight (LOS)
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SightlineAnalysis {
    pub id: usize,
    pub observer_pt: Vec3,
    pub target_pt: Vec3,
    pub observer_height: f32,
    pub target_height: f32,
    pub is_visible: bool,
    pub obstruction_pt: Option<Vec3>,
    pub obstruction_dist: Option<f32>,
    pub obstruction_feature: Option<String>,
    pub total_distance: f32,
    pub horizontal_dist: f32,
    pub vertical_diff: f32,
    pub azimuth_deg: f32,
    pub pitch_deg: f32,
}

impl SightlineAnalysis {
    pub fn compute(
        id: usize,
        observer_pt: Vec3,
        target_pt: Vec3,
        observer_height: f32,
        target_height: f32,
        collider: &SceneCollider,
    ) -> Self {
        let obs = observer_pt + Vec3::new(0.0, observer_height, 0.0);
        let tgt = target_pt + Vec3::new(0.0, target_height, 0.0);

        let delta = tgt - obs;
        let total_distance = delta.length();
        let horizontal_dist = (delta.x * delta.x + delta.z * delta.z).sqrt();
        let vertical_diff = delta.y;

        let az = delta.x.atan2(-delta.z).to_degrees();
        let azimuth_deg = if az < 0.0 { az + 360.0 } else { az };
        let pitch_deg = delta.y.atan2(horizontal_dist.max(0.001)).to_degrees();

        let (is_visible, obstruction_pt, obstruction_dist, obstruction_feature) =
            if let Some((hit_dist, hit_pt, feat_id)) = collider.cast_ray_segment(obs, tgt) {
                (false, Some(hit_pt), Some(hit_dist), Some(feat_id))
            } else {
                (true, None, None, None)
            };

        Self {
            id,
            observer_pt,
            target_pt,
            observer_height,
            target_height,
            is_visible,
            obstruction_pt,
            obstruction_dist,
            obstruction_feature,
            total_distance,
            horizontal_dist,
            vertical_diff,
            azimuth_deg,
            pitch_deg,
        }
    }

    pub fn observer_eye_pt(&self) -> Vec3 {
        self.observer_pt + Vec3::new(0.0, self.observer_height, 0.0)
    }

    pub fn target_focal_pt(&self) -> Vec3 {
        self.target_pt + Vec3::new(0.0, self.target_height, 0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineOfSightState {
    /// In-progress observer point (Click 1 placed, waiting for Click 2)
    pub pending_observer_pt: Option<Vec3>,
    pub observer_height: f32,
    pub target_height: f32,
    pub analyses: Vec<SightlineAnalysis>,
    pub next_id: usize,

    // Backward-compatibility mirrors of latest analysis
    pub observer_pt: Option<Vec3>,
    pub target_pt: Option<Vec3>,
    pub is_visible: bool,
    pub obstruction_pt: Option<Vec3>,
    pub obstruction_dist: Option<f32>,
    pub obstruction_feature: Option<String>,
    pub total_distance: f32,
    pub horizontal_dist: f32,
    pub vertical_diff: f32,
    pub azimuth_deg: f32,
    pub pitch_deg: f32,
}

impl Default for LineOfSightState {
    fn default() -> Self {
        Self {
            pending_observer_pt: None,
            observer_height: 1.7, // Standard adult eye level
            target_height: 0.0,
            analyses: Vec::new(),
            next_id: 0,
            observer_pt: None,
            target_pt: None,
            is_visible: true,
            obstruction_pt: None,
            obstruction_dist: None,
            obstruction_feature: None,
            total_distance: 0.0,
            horizontal_dist: 0.0,
            vertical_diff: 0.0,
            azimuth_deg: 0.0,
            pitch_deg: 0.0,
        }
    }
}

impl LineOfSightState {
    pub fn observer_eye_pt(&self) -> Option<Vec3> {
        self.pending_observer_pt
            .or(self.observer_pt)
            .map(|p| p + Vec3::new(0.0, self.observer_height, 0.0))
    }

    pub fn target_focal_pt(&self) -> Option<Vec3> {
        self.target_pt.map(|p| p + Vec3::new(0.0, self.target_height, 0.0))
    }

    pub fn add_analysis(&mut self, observer_pt: Vec3, target_pt: Vec3, collider: &SceneCollider) {
        self.next_id += 1;
        let analysis = SightlineAnalysis::compute(
            self.next_id,
            observer_pt,
            target_pt,
            self.observer_height,
            self.target_height,
            collider,
        );
        self.sync_with_analysis(&analysis);
        self.analyses.push(analysis);
    }

    pub fn remove_analysis(&mut self, id: usize) {
        self.analyses.retain(|a| a.id != id);
        if let Some(last) = self.analyses.last().cloned() {
            self.sync_with_analysis(&last);
        } else {
            self.observer_pt = None;
            self.target_pt = None;
            self.is_visible = true;
            self.obstruction_pt = None;
            self.obstruction_dist = None;
            self.obstruction_feature = None;
            self.total_distance = 0.0;
            self.horizontal_dist = 0.0;
            self.vertical_diff = 0.0;
        }
    }

    pub fn recompute_all(&mut self, collider: &SceneCollider) {
        let obs_h = self.observer_height;
        let tgt_h = self.target_height;
        self.analyses.par_iter_mut().for_each(|a| {
            let recomputed = SightlineAnalysis::compute(
                a.id,
                a.observer_pt,
                a.target_pt,
                obs_h,
                tgt_h,
                collider,
            );
            *a = recomputed;
        });
        if let Some(last) = self.analyses.last().cloned() {
            self.sync_with_analysis(&last);
        }
    }

    fn sync_with_analysis(&mut self, analysis: &SightlineAnalysis) {
        self.observer_pt = Some(analysis.observer_pt);
        self.target_pt = Some(analysis.target_pt);
        self.is_visible = analysis.is_visible;
        self.obstruction_pt = analysis.obstruction_pt;
        self.obstruction_dist = analysis.obstruction_dist;
        self.obstruction_feature = analysis.obstruction_feature.clone();
        self.total_distance = analysis.total_distance;
        self.horizontal_dist = analysis.horizontal_dist;
        self.vertical_diff = analysis.vertical_diff;
        self.azimuth_deg = analysis.azimuth_deg;
        self.pitch_deg = analysis.pitch_deg;
    }

    pub fn compute(&mut self, collider: &SceneCollider) {
        let (Some(obs_base), Some(tgt_base)) = (self.observer_pt, self.target_pt) else {
            return;
        };
        self.add_analysis(obs_base, tgt_base, collider);
    }

    pub fn clear(&mut self) {
        self.pending_observer_pt = None;
        self.observer_pt = None;
        self.target_pt = None;
        self.is_visible = true;
        self.obstruction_pt = None;
        self.obstruction_dist = None;
        self.obstruction_feature = None;
        self.total_distance = 0.0;
        self.horizontal_dist = 0.0;
        self.vertical_diff = 0.0;
        self.azimuth_deg = 0.0;
        self.pitch_deg = 0.0;
        self.analyses.clear();
    }
}

// ==========================================
// 2. Elevation & Urban Profile
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSamplePoint {
    pub distance_along_m: f32,
    pub pos_3d: Vec3,
    pub ground_elevation: f32,
    pub building_top_elevation: Option<f32>,
    pub building_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationProfileState {
    pub start_pt: Option<Vec3>,
    pub end_pt: Option<Vec3>,
    pub num_samples: usize,
    pub samples: Vec<ProfileSamplePoint>,
    pub total_length_m: f32,
    pub min_elevation: f32,
    pub max_elevation: f32,
    pub max_building_peak: f32,
    pub elevation_gain: f32,
    pub elevation_loss: f32,
    pub max_slope_pct: f32,
    pub avg_slope_pct: f32,
}

impl Default for ElevationProfileState {
    fn default() -> Self {
        Self {
            start_pt: None,
            end_pt: None,
            num_samples: 64,
            samples: Vec::new(),
            total_length_m: 0.0,
            min_elevation: 0.0,
            max_elevation: 0.0,
            max_building_peak: 0.0,
            elevation_gain: 0.0,
            elevation_loss: 0.0,
            max_slope_pct: 0.0,
            avg_slope_pct: 0.0,
        }
    }
}

impl ElevationProfileState {
    pub fn compute(&mut self, collider: &SceneCollider) {
        let (Some(p0), Some(p1)) = (self.start_pt, self.end_pt) else {
            return;
        };

        let diff = p1 - p0;
        self.total_length_m = (diff.x * diff.x + diff.z * diff.z).sqrt();
        if self.total_length_m < 1.0 {
            return;
        }

        let n = self.num_samples.clamp(16, 256);
        let total_length = self.total_length_m;

        let samples: Vec<ProfileSamplePoint> = (0..n)
            .into_par_iter()
            .map(|i| {
                let frac = i as f32 / (n - 1) as f32;
                let current_pos = p0 + diff * frac;
                let dist_along = total_length * frac;
                let ground_y = current_pos.y;

                let (bldg_top, bldg_name) = if let Some((top_y, feat_id)) = collider.sample_building_height_at(current_pos.x, current_pos.z) {
                    if top_y > ground_y + 1.0 {
                        (Some(top_y), Some(feat_id))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                ProfileSamplePoint {
                    distance_along_m: dist_along,
                    pos_3d: current_pos,
                    ground_elevation: ground_y,
                    building_top_elevation: bldg_top,
                    building_name: bldg_name,
                }
            })
            .collect();

        let mut min_elev = f32::INFINITY;
        let mut max_elev = f32::NEG_INFINITY;
        let mut max_peak = f32::NEG_INFINITY;
        let mut gain = 0.0f32;
        let mut loss = 0.0f32;
        let mut max_slope = 0.0f32;
        let mut total_slope = 0.0f32;

        let mut prev_ground = 0.0f32;
        for (i, s) in samples.iter().enumerate() {
            let ground_y = s.ground_elevation;
            min_elev = min_elev.min(ground_y);
            max_elev = max_elev.max(ground_y);
            if let Some(top_y) = s.building_top_elevation {
                max_peak = max_peak.max(top_y);
            }

            if i > 0 {
                let d_dist = total_length / (n - 1) as f32;
                let d_elev = ground_y - prev_ground;
                if d_elev > 0.0 {
                    gain += d_elev;
                } else {
                    loss += d_elev.abs();
                }
                let slope_pct = (d_elev.abs() / d_dist.max(0.1)) * 100.0;
                max_slope = max_slope.max(slope_pct);
                total_slope += slope_pct;
            }
            prev_ground = ground_y;
        }

        self.samples = samples;
        self.min_elevation = if min_elev.is_finite() { min_elev } else { 0.0 };
        self.max_elevation = if max_elev.is_finite() { max_elev } else { 0.0 };
        self.max_building_peak = if max_peak.is_finite() { max_peak } else { self.max_elevation };
        self.elevation_gain = gain;
        self.elevation_loss = loss;
        self.max_slope_pct = max_slope;
        self.avg_slope_pct = if n > 1 { total_slope / (n - 1) as f32 } else { 0.0 };
    }

    pub fn export_csv(&self) -> String {
        let mut csv = String::from("Distance_m,Ground_Elevation_m,Building_Top_Elevation_m,Building_ID,Local_X,Local_Z\n");
        for s in &self.samples {
            let bldg_elev = s.building_top_elevation.map(|v| format!("{:.2}", v)).unwrap_or_default();
            let bldg_name = s.building_name.as_deref().unwrap_or_default();
            csv.push_str(&format!(
                "{:.2},{:.2},{},{},{:.2},{:.2}\n",
                s.distance_along_m,
                s.ground_elevation,
                bldg_elev,
                bldg_name,
                s.pos_3d.x,
                s.pos_3d.z
            ));
        }
        csv
    }

    pub fn clear(&mut self) {
        self.start_pt = None;
        self.end_pt = None;
        self.samples.clear();
        self.total_length_m = 0.0;
        self.min_elevation = 0.0;
        self.max_elevation = 0.0;
        self.max_building_peak = 0.0;
    }
}

// ==========================================
// 3. Radial Viewshed Analysis (360° / FOV)
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewshedRay {
    pub azimuth_deg: f32,
    pub hit_dist: f32,
    pub hit_pt: Vec3,
    pub is_blocked: bool,
    pub blocked_feature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewshedState {
    pub observer_pt: Option<Vec3>,
    pub observer_height: f32,
    pub radius: f32,
    pub fov_deg: f32,
    pub azimuth_deg: f32,
    pub ray_count: usize,
    pub rays: Vec<ViewshedRay>,
    pub visible_area_m2: f32,
    pub total_area_m2: f32,
    pub visibility_ratio: f32,
}

impl Default for ViewshedState {
    fn default() -> Self {
        Self {
            observer_pt: None,
            observer_height: 1.7,
            radius: 300.0,
            fov_deg: 360.0,
            azimuth_deg: 0.0,
            ray_count: 72,
            rays: Vec::new(),
            visible_area_m2: 0.0,
            total_area_m2: 0.0,
            visibility_ratio: 1.0,
        }
    }
}

impl ViewshedState {
    pub fn observer_eye_pt(&self) -> Option<Vec3> {
        self.observer_pt.map(|p| p + Vec3::new(0.0, self.observer_height, 0.0))
    }

    pub fn compute(&mut self, collider: &SceneCollider) {
        let Some(obs_base) = self.observer_pt else {
            return;
        };

        let obs = obs_base + Vec3::new(0.0, self.observer_height, 0.0);
        let n = self.ray_count.clamp(16, 180);
        self.rays.clear();
        self.rays.reserve(n);

        let half_fov = self.fov_deg * 0.5;
        let is_full_circle = self.fov_deg >= 359.0;

        let start_angle = if is_full_circle { 0.0 } else { self.azimuth_deg - half_fov };
        let step = if is_full_circle { 360.0 / n as f32 } else { self.fov_deg / (n - 1).max(1) as f32 };

        let radius = self.radius;

        let rays: Vec<ViewshedRay> = (0..n)
            .into_par_iter()
            .map(|i| {
                let az = start_angle + i as f32 * step;
                let az_rad = az.to_radians();

                let dir = Vec3::new(az_rad.sin(), 0.0, -az_rad.cos()).normalize();
                let target_endpoint = obs + dir * radius;

                let (hit_dist, hit_pt, is_blocked, feat) = if let Some((d, pt, feat_id)) = collider.cast_ray_segment(obs, target_endpoint) {
                    (d, pt, true, Some(feat_id))
                } else {
                    (radius, target_endpoint, false, None)
                };

                ViewshedRay {
                    azimuth_deg: az,
                    hit_dist,
                    hit_pt,
                    is_blocked,
                    blocked_feature: feat,
                }
            })
            .collect();

        let mut sum_triangle_area = 0.0f32;
        let d_theta = step.to_radians();
        let sin_d_theta = d_theta.sin();
        for i in 1..rays.len() {
            let tri_area = 0.5 * rays[i - 1].hit_dist * rays[i].hit_dist * sin_d_theta;
            sum_triangle_area += tri_area;
        }

        self.rays = rays;

        self.total_area_m2 = PI * self.radius * self.radius * (self.fov_deg / 360.0);
        self.visible_area_m2 = sum_triangle_area.min(self.total_area_m2);
        self.visibility_ratio = if self.total_area_m2 > 0.0 { (self.visible_area_m2 / self.total_area_m2).clamp(0.0, 1.0) } else { 0.0 };
    }

    pub fn clear(&mut self) {
        self.observer_pt = None;
        self.rays.clear();
        self.visible_area_m2 = 0.0;
        self.total_area_m2 = 0.0;
    }
}

// ==========================================
// 4. Sky View Factor (SVF) & Daylight Canyon
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyViewRay {
    pub dir: Vec3,
    pub is_open: bool,
    pub hit_dist: Option<f32>,
    pub polar_r: f32,
    pub polar_angle_rad: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyViewFactorState {
    pub probe_pt: Option<Vec3>,
    pub eye_height: f32,
    pub sample_count: usize,
    pub svf: f32,
    pub rays: Vec<SkyViewRay>,
    pub canyon_category: String,
}

impl Default for SkyViewFactorState {
    fn default() -> Self {
        Self {
            probe_pt: None,
            eye_height: 1.5,
            sample_count: 96,
            svf: 1.0,
            rays: Vec::new(),
            canyon_category: "Open Landscape".to_string(),
        }
    }
}

impl SkyViewFactorState {
    pub fn compute(&mut self, collider: &SceneCollider) {
        let Some(base_pt) = self.probe_pt else {
            return;
        };

        let origin = base_pt + Vec3::new(0.0, self.eye_height, 0.0);
        let n = self.sample_count.clamp(32, 256);
        let max_ray_dist = 500.0f32;
        let golden_ratio = (1.0 + 5.0f32.sqrt()) / 2.0;

        let rays: Vec<SkyViewRay> = (0..n)
            .into_par_iter()
            .map(|i| {
                let theta = (1.0 - (i as f32 + 0.5) / n as f32).acos();
                let phi = 2.0 * PI * (i as f32) / golden_ratio;

                let cos_theta = theta.cos().max(0.0);
                let sin_theta = theta.sin();

                let dir = Vec3::new(
                    sin_theta * phi.cos(),
                    cos_theta,
                    -sin_theta * phi.sin(),
                ).normalize();

                let target = origin + dir * max_ray_dist;
                let (is_open, hit_dist) = if let Some((d, _, _)) = collider.cast_ray_segment(origin, target) {
                    (false, Some(d))
                } else {
                    (true, None)
                };

                let polar_r = (theta / (PI * 0.5)).clamp(0.0, 1.0);

                SkyViewRay {
                    dir,
                    is_open,
                    hit_dist,
                    polar_r,
                    polar_angle_rad: phi,
                }
            })
            .collect();

        let mut sum_open_weight = 0.0f32;
        let mut sum_total_weight = 0.0f32;
        for (i, ray) in rays.iter().enumerate() {
            let theta = (1.0 - (i as f32 + 0.5) / n as f32).acos();
            let weight = theta.cos().max(0.0);
            sum_total_weight += weight;
            if ray.is_open {
                sum_open_weight += weight;
            }
        }

        self.rays = rays;

        self.svf = if sum_total_weight > 0.0 { (sum_open_weight / sum_total_weight).clamp(0.0, 1.0) } else { 1.0 };

        self.canyon_category = if self.svf >= 0.75 {
            "Open Landscape (High Daylight)".to_string()
        } else if self.svf >= 0.50 {
            "Moderate Urban (Good Daylight)".to_string()
        } else if self.svf >= 0.30 {
            "Deep Urban Canyon (Constrained)".to_string()
        } else {
            "Severely Occluded Courtyard".to_string()
        };
    }

    pub fn clear(&mut self) {
        self.probe_pt = None;
        self.rays.clear();
        self.svf = 1.0;
    }
}

// ==========================================
// 5. Solar Rights & Shadow Impact Checker
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlySunStatus {
    pub hour: f32,
    pub time_str: String,
    pub elevation_deg: f32,
    pub azimuth_deg: f32,
    pub is_daylight: bool,
    pub is_in_sunlight: bool,
    pub occluding_feature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowImpactState {
    pub target_pt: Option<Vec3>,
    pub date_preset: usize, // 0 = Winter Solstice, 1 = Summer Solstice, 2 = Equinox, 3 = Current Date
    pub start_hour: f32,
    pub end_hour: f32,
    pub compliance_threshold_hours: f32,
    pub hourly_status: Vec<HourlySunStatus>,
    pub total_sunlight_hours: f32,
    pub max_consecutive_hours: f32,
    pub is_compliant: bool,
}

impl Default for ShadowImpactState {
    fn default() -> Self {
        Self {
            target_pt: None,
            date_preset: 0, // Winter Solstice
            start_hour: 9.0,
            end_hour: 15.0,
            compliance_threshold_hours: 2.0,
            hourly_status: Vec::new(),
            total_sunlight_hours: 0.0,
            max_consecutive_hours: 0.0,
            is_compliant: false,
        }
    }
}

impl ShadowImpactState {
    pub fn compute(&mut self, collider: &SceneCollider, origin: &ProjectOrigin, dt_state: &SolarDateTimeState) {
        let Some(pt) = self.target_pt else {
            return;
        };

        let (sim_year, sim_month, sim_day) = match self.date_preset {
            0 => (dt_state.year, 6, 21),  // Winter Solstice / Jun 21
            1 => (dt_state.year, 12, 21), // Summer Solstice / Dec 21
            2 => (dt_state.year, 3, 21),  // Autumn Equinox / Mar 21
            _ => (dt_state.year, dt_state.month, dt_state.day),
        };

        self.hourly_status.clear();
        let step_minutes = 30.0f32;
        let total_steps = (((self.end_hour - self.start_hour) * 60.0 / step_minutes) as usize) + 1;
        let start_hour = self.start_hour;

        let geo = origin.local_to_geo(pt);

        let statuses: Vec<HourlySunStatus> = (0..total_steps)
            .into_par_iter()
            .map(|i| {
                let cur_minute_of_day = start_hour * 60.0 + i as f32 * step_minutes;
                let hour = cur_minute_of_day / 60.0;
                let hr_int = hour.floor() as u32;
                let min_int = (cur_minute_of_day % 60.0).round() as u32;
                let time_str = format!("{:02}:{:02}", hr_int, min_int);

                let mut test_dt = dt_state.clone();
                test_dt.year = sim_year;
                test_dt.month = sim_month;
                test_dt.day = sim_day;
                test_dt.hour = hr_int;
                test_dt.minute = min_int;
                test_dt.second = 0;

                let utc_dt = test_dt.to_utc_datetime();
                let solar_pos = calculate_solar_position(geo.latitude, geo.longitude, &utc_dt, dt_state.timezone_offset_hours);

                let is_daylight = solar_pos.is_daylight && solar_pos.elevation_deg > 2.0;
                let mut is_in_sunlight = false;
                let mut occluder = None;

                if is_daylight {
                    let sun_dir = solar_pos.sun_direction;
                    if let Some(occ) = collider.is_point_occluded(pt, sun_dir, 1000.0) {
                        occluder = Some(occ);
                    } else {
                        is_in_sunlight = true;
                    }
                }

                HourlySunStatus {
                    hour,
                    time_str,
                    elevation_deg: solar_pos.elevation_deg,
                    azimuth_deg: solar_pos.azimuth_deg,
                    is_daylight,
                    is_in_sunlight,
                    occluding_feature: occluder,
                }
            })
            .collect();

        let mut sun_hours = 0.0f32;
        let mut cur_consecutive = 0.0f32;
        let mut max_consecutive = 0.0f32;
        let step_hr = step_minutes / 60.0;

        for s in &statuses {
            if s.is_in_sunlight {
                sun_hours += step_hr;
                cur_consecutive += step_hr;
                max_consecutive = max_consecutive.max(cur_consecutive);
            } else {
                cur_consecutive = 0.0;
            }
        }

        self.hourly_status = statuses;
        self.total_sunlight_hours = sun_hours;
        self.max_consecutive_hours = max_consecutive;
        self.is_compliant = self.total_sunlight_hours >= self.compliance_threshold_hours;
    }

    pub fn clear(&mut self) {
        self.target_pt = None;
        self.hourly_status.clear();
        self.total_sunlight_hours = 0.0;
    }
}

// ==========================================
// 6. Cut & Fill Earthwork Volume Estimator
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutAndFillState {
    pub center_pt: Option<Vec3>,
    pub pad_width: f32,
    pub pad_length: f32,
    pub pad_elevation: f32,
    pub grid_resolution: f32,
    pub cut_volume_m3: f32,
    pub fill_volume_m3: f32,
    pub net_volume_m3: f32,
    pub max_cut_depth_m: f32,
    pub max_fill_depth_m: f32,
    pub area_m2: f32,
}

impl Default for CutAndFillState {
    fn default() -> Self {
        Self {
            center_pt: None,
            pad_width: 50.0,
            pad_length: 50.0,
            pad_elevation: 0.0,
            grid_resolution: 2.5,
            cut_volume_m3: 0.0,
            fill_volume_m3: 0.0,
            net_volume_m3: 0.0,
            max_cut_depth_m: 0.0,
            max_fill_depth_m: 0.0,
            area_m2: 2500.0,
        }
    }
}

impl CutAndFillState {
    pub fn compute(&mut self, polygon: Option<&[Vec2]>) {
        let Some(center) = self.center_pt else {
            return;
        };

        let res = self.grid_resolution.clamp(1.0, 10.0);
        let cell_area = res * res;

        let pad_elevation = self.pad_elevation;
        let terrain_y = center.y;

        let (total_cut, total_fill, max_cut, max_fill, sampled_area) = if let Some(poly) = polygon {
            if poly.len() >= 3 {
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_z = f32::INFINITY;
                let mut max_z = f32::NEG_INFINITY;
                for p in poly {
                    min_x = min_x.min(p.x);
                    max_x = max_x.max(p.x);
                    min_z = min_z.min(p.y);
                    max_z = max_z.max(p.y);
                }

                let nx = ((max_x - min_x) / res).ceil() as usize;
                let nz = ((max_z - min_z) / res).ceil() as usize;

                let row_results: Vec<(f32, f32, f32, f32, f32)> = (0..nx)
                    .into_par_iter()
                    .map(|ix| {
                        let x = min_x + (ix as f32 + 0.5) * res;
                        let mut row_cut = 0.0f32;
                        let mut row_fill = 0.0f32;
                        let mut row_max_cut = 0.0f32;
                        let mut row_max_fill = 0.0f32;
                        let mut row_area = 0.0f32;

                        for iz in 0..nz {
                            let z = min_z + (iz as f32 + 0.5) * res;
                            if point_in_polygon_2d(Vec2::new(x, z), poly) {
                                row_area += cell_area;
                                let diff = terrain_y - pad_elevation;
                                if diff > 0.0 {
                                    row_cut += diff * cell_area;
                                    row_max_cut = row_max_cut.max(diff);
                                } else {
                                    let fill = diff.abs();
                                    row_fill += fill * cell_area;
                                    row_max_fill = row_max_fill.max(fill);
                                }
                            }
                        }
                        (row_cut, row_fill, row_max_cut, row_max_fill, row_area)
                    })
                    .collect();

                let mut t_cut = 0.0f32;
                let mut t_fill = 0.0f32;
                let mut m_cut = 0.0f32;
                let mut m_fill = 0.0f32;
                let mut s_area = 0.0f32;
                for (r_cut, r_fill, rm_cut, rm_fill, r_area) in row_results {
                    t_cut += r_cut;
                    t_fill += r_fill;
                    m_cut = m_cut.max(rm_cut);
                    m_fill = m_fill.max(rm_fill);
                    s_area += r_area;
                }
                (t_cut, t_fill, m_cut, m_fill, s_area)
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0)
            }
        } else {
            let half_w = self.pad_width * 0.5;
            let _half_l = self.pad_length * 0.5;
            let nx = (self.pad_width / res).ceil() as usize;
            let nz = (self.pad_length / res).ceil() as usize;

            let row_results: Vec<(f32, f32, f32, f32, f32)> = (0..nx)
                .into_par_iter()
                .map(|ix| {
                    let _x = center.x - half_w + (ix as f32 + 0.5) * res;
                    let mut row_cut = 0.0f32;
                    let mut row_fill = 0.0f32;
                    let mut row_max_cut = 0.0f32;
                    let mut row_max_fill = 0.0f32;
                    let mut row_area = 0.0f32;

                    for _iz in 0..nz {
                        row_area += cell_area;
                        let diff = terrain_y - pad_elevation;
                        if diff > 0.0 {
                            row_cut += diff * cell_area;
                            row_max_cut = row_max_cut.max(diff);
                        } else {
                            let fill = diff.abs();
                            row_fill += fill * cell_area;
                            row_max_fill = row_max_fill.max(fill);
                        }
                    }
                    (row_cut, row_fill, row_max_cut, row_max_fill, row_area)
                })
                .collect();

            let mut t_cut = 0.0f32;
            let mut t_fill = 0.0f32;
            let mut m_cut = 0.0f32;
            let mut m_fill = 0.0f32;
            let mut s_area = 0.0f32;
            for (r_cut, r_fill, rm_cut, rm_fill, r_area) in row_results {
                t_cut += r_cut;
                t_fill += r_fill;
                m_cut = m_cut.max(rm_cut);
                m_fill = m_fill.max(rm_fill);
                s_area += r_area;
            }
            (t_cut, t_fill, m_cut, m_fill, s_area)
        };

        self.area_m2 = sampled_area;
        self.cut_volume_m3 = total_cut;
        self.fill_volume_m3 = total_fill;
        self.net_volume_m3 = total_cut - total_fill;
        self.max_cut_depth_m = max_cut;
        self.max_fill_depth_m = max_fill;
    }

    pub fn clear(&mut self) {
        self.center_pt = None;
        self.cut_volume_m3 = 0.0;
        self.fill_volume_m3 = 0.0;
        self.net_volume_m3 = 0.0;
    }
}

fn point_in_polygon_2d(pt: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let vi = poly[i];
        let vj = poly[j];
        if ((vi.y > pt.y) != (vj.y > pt.y)) && (pt.x < (vj.x - vi.x) * (pt.y - vi.y) / (vj.y - vi.y) + vi.x) {
            inside = !inside;
        }
    }
    inside
}

// ==========================================
// 7. Multi-hour Shadow Range Union (ResCode 9am–3pm)
// ==========================================

/// One hourly time step in the shadow range union analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowRangeStep {
    /// Local time hour (9.0 = 9:00am, 15.0 = 3:00pm)
    pub hour: f32,
    pub time_str: String,
    pub elevation_deg: f32,
    pub azimuth_deg: f32,
    pub is_daylight: bool,
    /// Sun direction used for this step
    pub sun_direction: [f32; 3],
}

/// Grid-based shadow footprint accumulator for multi-hour union analysis.
///
/// Rasterises the scene's shadows at each ResCode hourly step onto a flat 2D
/// XZ grid, then sums per-cell hit counts so that the viewport overlay can
/// render a heat-gradient from "never shadowed" (transparent) through to
/// "shadowed every hour" (deep amber/red).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowRangeUnionState {
    /// Is the computation currently active / results visible?
    pub is_active: bool,

    /// Date preset: 0 = Winter Solstice, 1 = Summer Solstice, 2 = Equinox (Sep 22), 3 = Current Date
    pub date_preset: usize,

    /// Start/end hours for the window (default 9 to 15 for ResCode)
    pub start_hour: f32,
    pub end_hour: f32,

    /// Grid resolution in metres (smaller = finer, slower)
    pub grid_resolution: f32,

    /// Bounding box centre and half-extents in ENU local space
    pub grid_center: Vec3,
    pub grid_half_extent: f32,

    /// Rasterised grid: (grid_nx × grid_nz) cells,
    /// each cell stores the number of hourly steps where it is in shadow (0..=n_steps).
    pub grid_nx: usize,
    pub grid_nz: usize,
    pub shadow_hit_counts: Vec<u8>,  // length = grid_nx * grid_nz
    pub n_steps: usize,              // number of time steps evaluated

    /// Computed hourly solar positions (for display in UI legend)
    pub steps: Vec<ShadowRangeStep>,

    /// Is the analysis still running (async guard to prevent re-trigger)?
    pub computing: bool,
}

impl Default for ShadowRangeUnionState {
    fn default() -> Self {
        Self {
            is_active: false,
            date_preset: 2,       // Equinox (Sep 22) is the ResCode standard date
            start_hour: 9.0,
            end_hour: 15.0,
            grid_resolution: 2.0,
            grid_center: Vec3::ZERO,
            grid_half_extent: 100.0,
            grid_nx: 0,
            grid_nz: 0,
            shadow_hit_counts: Vec::new(),
            n_steps: 0,
            steps: Vec::new(),
            computing: false,
        }
    }
}

impl ShadowRangeUnionState {
    /// Compute the multi-hour shadow range union synchronously.
    ///
    /// For each hour in `[start_hour, end_hour]` (inclusive, 1-hour step):
    ///   1. Compute sun direction.
    ///   2. For each grid cell centre, cast a shadow ray upward toward the sun.
    ///   3. Increment hit count when a scene mesh occludes that cell.
    pub fn compute(
        &mut self,
        collider: &SceneCollider,
        origin: &ProjectOrigin,
        dt_state: &SolarDateTimeState,
    ) {
        if collider.mesh_colliders.is_empty() {
            self.shadow_hit_counts.clear();
            self.steps.clear();
            self.n_steps = 0;
            return;
        }

        self.computing = true;

        let (sim_year, sim_month, sim_day) = match self.date_preset {
            0 => (dt_state.year, 6, 21),   // Winter Solstice
            1 => (dt_state.year, 12, 21),  // Summer Solstice
            2 => (dt_state.year, 9, 22),   // Equinox (Sep 22 — ResCode standard)
            _ => (dt_state.year, dt_state.month, dt_state.day),
        };

        // Build list of hourly time steps
        let geo = origin.origin;
        let n_steps_raw = ((self.end_hour - self.start_hour).round() as usize) + 1;
        let mut steps: Vec<ShadowRangeStep> = Vec::with_capacity(n_steps_raw);
        for i in 0..n_steps_raw {
            let hour = self.start_hour + i as f32;
            let hr_int = hour.floor() as u32;
            let mut test_dt = dt_state.clone();
            test_dt.year = sim_year;
            test_dt.month = sim_month;
            test_dt.day = sim_day;
            test_dt.hour = hr_int;
            test_dt.minute = 0;
            test_dt.second = 0;
            let utc_dt = test_dt.to_utc_datetime();
            let solar_pos = calculate_solar_position(geo.latitude, geo.longitude, &utc_dt, dt_state.timezone_offset_hours);
            steps.push(ShadowRangeStep {
                hour,
                time_str: format!("{:02}:00", hr_int),
                elevation_deg: solar_pos.elevation_deg,
                azimuth_deg: solar_pos.azimuth_deg,
                is_daylight: solar_pos.is_daylight,
                sun_direction: solar_pos.sun_direction.to_array(),
            });
        }

        let daylight_steps: Vec<&ShadowRangeStep> = steps.iter().filter(|s| s.is_daylight && s.elevation_deg > 2.0).collect();
        let n_daylight = daylight_steps.len();
        self.n_steps = n_daylight;

        // Rasterise grid
        let res = self.grid_resolution.clamp(1.0, 20.0);
        let half = self.grid_half_extent.clamp(20.0, 500.0);
        let cx = self.grid_center.x;
        let cz = self.grid_center.z;
        let ground_y = self.grid_center.y;

        let n_side = ((half * 2.0 / res).ceil() as usize).clamp(4, 400);
        self.grid_nx = n_side;
        self.grid_nz = n_side;
        let total_cells = n_side * n_side;

        // Collect sun directions for daylight steps
        let sun_dirs: Vec<glam::Vec3> = daylight_steps.iter()
            .map(|s| glam::Vec3::from_array(s.sun_direction))
            .collect();

        // Parallel rasterisation across grid rows
        use crate::compute::parallel::IntoParallelIterator;
        let row_counts: Vec<Vec<u8>> = (0..n_side)
            .into_par_iter()
            .map(|iz| {
                let z = cz - half + (iz as f32 + 0.5) * res;
                let mut row = vec![0u8; n_side];
                for ix in 0..n_side {
                    let x = cx - half + (ix as f32 + 0.5) * res;
                    let test_pt = glam::Vec3::new(x, ground_y + 0.1, z);
                    let mut hits = 0u8;
                    for &sun_dir in &sun_dirs {
                        if collider.is_point_occluded(test_pt, sun_dir, 500.0).is_some() {
                            hits = hits.saturating_add(1);
                        }
                    }
                    row[ix] = hits;
                }
                row
            })
            .collect();

        let mut counts = Vec::with_capacity(total_cells);
        for row in row_counts {
            counts.extend_from_slice(&row);
        }

        self.shadow_hit_counts = counts;
        self.steps = steps;
        self.computing = false;
        self.is_active = true;
    }

    pub fn clear(&mut self) {
        self.is_active = false;
        self.shadow_hit_counts.clear();
        self.steps.clear();
        self.n_steps = 0;
        self.computing = false;
    }

    /// Returns the shadow fraction (0.0 = never shadowed, 1.0 = always shadowed) for the cell at (ix, iz)
    pub fn shadow_fraction_at(&self, ix: usize, iz: usize) -> f32 {
        if self.n_steps == 0 || ix >= self.grid_nx || iz >= self.grid_nz {
            return 0.0;
        }
        let idx = iz * self.grid_nx + ix;
        if idx >= self.shadow_hit_counts.len() {
            return 0.0;
        }
        self.shadow_hit_counts[idx] as f32 / self.n_steps as f32
    }
}


// ==========================================
// Main Toolbox Engine
// ==========================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolboxEngine {
    pub open_tool: Option<ToolboxToolMode>,
    pub active_tool: ToolboxToolMode,
    pub los: LineOfSightState,
    pub profile: ElevationProfileState,
    pub viewshed: ViewshedState,
    pub svf: SkyViewFactorState,
    pub shadow_impact: ShadowImpactState,
    pub cut_and_fill: CutAndFillState,
    pub shadow_range_union: ShadowRangeUnionState,
}

impl ToolboxEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tool(&mut self, tool: ToolboxToolMode) {
        self.active_tool = tool;
    }

    pub fn cancel_pending(&mut self) {
        self.los.pending_observer_pt = None;
        if self.profile.end_pt.is_none() {
            self.profile.start_pt = None;
        }
    }

    pub fn clear_active(&mut self) {
        match self.active_tool {
            ToolboxToolMode::LineOfSight => self.los.clear(),
            ToolboxToolMode::ElevationProfile => self.profile.clear(),
            ToolboxToolMode::Viewshed => self.viewshed.clear(),
            ToolboxToolMode::SkyViewFactor => self.svf.clear(),
            ToolboxToolMode::ShadowImpact => self.shadow_impact.clear(),
            ToolboxToolMode::CutAndFill => self.cut_and_fill.clear(),
            ToolboxToolMode::ShadowRangeUnion => self.shadow_range_union.clear(),
            ToolboxToolMode::None => {}
        }
    }

    pub fn add_point(
        &mut self,
        pt: Vec3,
        collider: &SceneCollider,
        origin: &ProjectOrigin,
        dt_state: &SolarDateTimeState,
        active_polygon: Option<&[Vec2]>,
    ) -> bool {
        match self.active_tool {
            ToolboxToolMode::LineOfSight => {
                if let Some(obs) = self.los.pending_observer_pt.take() {
                    self.los.add_analysis(obs, pt, collider);
                    true
                } else {
                    self.los.pending_observer_pt = Some(pt);
                    false
                }
            }
            ToolboxToolMode::ElevationProfile => {
                if self.profile.start_pt.is_none() {
                    self.profile.start_pt = Some(pt);
                    self.profile.samples.clear();
                    false
                } else {
                    self.profile.end_pt = Some(pt);
                    self.profile.compute(collider);
                    true
                }
            }
            ToolboxToolMode::Viewshed => {
                self.viewshed.observer_pt = Some(pt);
                self.viewshed.compute(collider);
                true
            }
            ToolboxToolMode::SkyViewFactor => {
                self.svf.probe_pt = Some(pt);
                self.svf.compute(collider);
                true
            }
            ToolboxToolMode::ShadowImpact => {
                self.shadow_impact.target_pt = Some(pt);
                self.shadow_impact.compute(collider, origin, dt_state);
                true
            }
            ToolboxToolMode::CutAndFill => {
                self.cut_and_fill.center_pt = Some(pt);
                self.cut_and_fill.compute(active_polygon);
                true
            }
            ToolboxToolMode::ShadowRangeUnion => {
                // Clicking the viewport centres the grid on the clicked point
                self.shadow_range_union.grid_center = pt;
                self.shadow_range_union.compute(collider, origin, dt_state);
                true
            }
            ToolboxToolMode::None => false,
        }
    }
}

