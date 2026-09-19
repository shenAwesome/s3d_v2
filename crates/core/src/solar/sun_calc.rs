use chrono::{DateTime, Datelike, Timelike, Utc};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Output of solar position calculation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolarPosition {
    /// Azimuth angle in degrees clockwise from North (0° = North, 90° = East, 180° = South, 270° = West)
    pub azimuth_deg: f32,
    /// Elevation angle above horizon in degrees [-90°, +90°]
    pub elevation_deg: f32,
    /// Zenith angle from straight up in degrees [0°, 180°]
    pub zenith_deg: f32,
    /// Unit vector pointing toward the sun in Engine 3D space (+X=East, +Y=Up, -Z=North)
    pub sun_direction: Vec3,
    /// Is the sun currently above the horizon?
    pub is_daylight: bool,
    /// Approximate sunrise hour in local time [0..24]
    pub sunrise_hour: f32,
    /// Approximate sunset hour in local time [0..24]
    pub sunset_hour: f32,
    /// Solar noon hour in local time [0..24]
    pub solar_noon_hour: f32,
}

/// Calculate high-accuracy solar position given geographic location and timestamp
pub fn calculate_solar_position(
    latitude_deg: f64,
    longitude_deg: f64,
    datetime: &DateTime<Utc>,
    timezone_offset_hours: f64,
) -> SolarPosition {
    let year = datetime.year() as f64;
    let month = datetime.month() as f64;
    let day = datetime.day() as f64;
    let hour = datetime.hour() as f64;
    let minute = datetime.minute() as f64;
    let second = datetime.second() as f64;

    // Convert to local time with offset
    let local_fractional_hour = hour + minute / 60.0 + second / 3600.0 + timezone_offset_hours;
    let local_fractional_hour = (local_fractional_hour % 24.0 + 24.0) % 24.0;

    // Julian Date calculation
    let a = ((14.0 - month) / 12.0).floor();
    let y = year + 4800.0 - a;
    let m = month + 12.0 * a - 3.0;

    let mut jd = day + ((153.0 * m + 2.0) / 5.0).floor() + 365.0 * y + (y / 4.0).floor()
        - (y / 100.0).floor() + (y / 400.0).floor() - 32045.0;
    let ut_hour = hour + minute / 60.0 + second / 3600.0;
    jd += (ut_hour - 12.0) / 24.0;

    // Julian Centuries since J2000.0
    let t = (jd - 2451545.0) / 36525.0;

    // Geometric Mean Longitude of the Sun (deg)
    let l0 = (280.46646 + t * (36000.76983 + t * 0.0003032)) % 360.0;
    let l0 = (l0 + 360.0) % 360.0;

    // Geometric Mean Anomaly of the Sun (deg)
    let m_ano = 357.52911 + t * (35999.05029 - 0.0001537 * t);
    let m_rad = m_ano.to_radians();

    // Eccentricity of Earth's Orbit
    let e = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);

    // Sun Equation of Center (deg)
    let c = m_rad.sin() * (1.914602 - t * (0.004817 + 0.000014 * t))
        + (2.0 * m_rad).sin() * (0.019993 - 0.000101 * t)
        + (3.0 * m_rad).sin() * 0.000289;

    // Sun True Longitude (deg)
    let sun_true_long = l0 + c;

    // Sun Apparent Longitude (deg)
    let omega = 125.04 - 1934.136 * t;
    let lambda = sun_true_long - 0.00569 - 0.00478 * omega.to_radians().sin();

    // Mean Obliquity of the Ecliptic (deg)
    let eps0 = 23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.0) / 60.0;
    let eps = eps0 + 0.00256 * omega.to_radians().cos();
    let eps_rad = eps.to_radians();
    let lambda_rad = lambda.to_radians();

    // Sun Declination (deg)
    let sin_delta = eps_rad.sin() * lambda_rad.sin();
    let delta_rad = sin_delta.asin();

    // Equation of Time (minutes)
    let y_tan = (eps_rad / 2.0).tan().powi(2);
    let l0_rad = l0.to_radians();
    let eq_time = 4.0 * (y_tan * (2.0 * l0_rad).sin()
        - 2.0 * e * m_rad.sin()
        + 4.0 * e * y_tan * m_rad.sin() * (2.0 * l0_rad).cos()
        - 0.5 * y_tan * y_tan * (4.0 * l0_rad).sin()
        - 1.25 * e * e * (2.0 * m_rad).sin()).to_degrees();

    // Solar Noon & Sunrise/Sunset
    let solar_noon_ut = (720.0 - 4.0 * longitude_deg - eq_time) / 60.0;
    let solar_noon_local = ((solar_noon_ut + timezone_offset_hours) % 24.0 + 24.0) % 24.0;

    let lat_rad = latitude_deg.to_radians();
    let cos_ha_sunset = (-0.8333f64.to_radians().sin() - lat_rad.sin() * delta_rad.sin())
        / (lat_rad.cos() * delta_rad.cos());

    let (sunrise_hour, sunset_hour) = if cos_ha_sunset > 1.0 {
        // Polar night (sun never rises)
        (0.0, 0.0)
    } else if cos_ha_sunset < -1.0 {
        // Midnight sun (sun never sets)
        (0.0, 24.0)
    } else {
        let ha_sunset_deg = cos_ha_sunset.acos().to_degrees();
        let half_day_hours = ha_sunset_deg / 15.0;
        let sr = ((solar_noon_local - half_day_hours) % 24.0 + 24.0) % 24.0;
        let ss = ((solar_noon_local + half_day_hours) % 24.0 + 24.0) % 24.0;
        (sr as f32, ss as f32)
    };

    // True Solar Time (minutes)
    let time_offset = eq_time + 4.0 * longitude_deg - 60.0 * timezone_offset_hours;
    let tst = (local_fractional_hour * 60.0 + time_offset) % 1440.0;
    let tst = (tst + 1440.0) % 1440.0;

    // Hour Angle (deg)
    let ha_deg = if tst / 4.0 < 0.0 {
        tst / 4.0 + 180.0
    } else {
        tst / 4.0 - 180.0
    };
    let ha_rad = ha_deg.to_radians();

    // Solar Zenith & Elevation Angle
    let cos_zenith = lat_rad.sin() * delta_rad.sin() + lat_rad.cos() * delta_rad.cos() * ha_rad.cos();
    let zenith_rad = cos_zenith.clamp(-1.0, 1.0).acos();
    let zenith_deg = zenith_rad.to_degrees();
    let elevation_deg = 90.0 - zenith_deg;

    // Solar Azimuth Angle (measured clockwise from North)
    let sin_azimuth = -(ha_rad.sin() * delta_rad.cos()) / zenith_rad.sin().max(1e-6);
    let cos_azimuth = (delta_rad.sin() * lat_rad.cos() - delta_rad.cos() * lat_rad.sin() * ha_rad.cos())
        / zenith_rad.sin().max(1e-6);
    let mut azimuth_deg = sin_azimuth.atan2(cos_azimuth).to_degrees();
    if azimuth_deg < 0.0 {
        azimuth_deg += 360.0;
    }

    let is_daylight = elevation_deg > -0.5;

    // Unit vector pointing toward Sun in Engine 3D space:
    // +X = East, +Y = Up, -Z = North
    // Accumulate in f64 for precision, cast to f32 at the last step.
    let az_rad_f64 = azimuth_deg.to_radians();
    let el_rad_f64 = elevation_deg.to_radians();

    let sx = az_rad_f64.sin() * el_rad_f64.cos();
    let sy = el_rad_f64.sin();
    let sz = -az_rad_f64.cos() * el_rad_f64.cos();

    let sun_direction = Vec3::new(sx as f32, sy as f32, sz as f32).normalize_or_zero();

    SolarPosition {
        azimuth_deg: azimuth_deg as f32,
        elevation_deg: elevation_deg as f32,
        zenith_deg: zenith_deg as f32,
        sun_direction,
        is_daylight,
        sunrise_hour,
        sunset_hour,
        solar_noon_hour: solar_noon_local as f32,
    }
}

