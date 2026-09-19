use s3d_core::solar::sun_calc::calculate_solar_position;
use chrono::{TimeZone, Utc};

#[test]
fn test_solar_position_melbourne_noon() {
    // Melbourne (-37.8136, 144.9631) on Dec 21 (Summer Solstice in Southern Hemisphere)
    // Local noon ~ 13:00 AEDT (UTC+11), so UTC is ~ 02:00
    let utc = Utc.with_ymd_and_hms(2025, 12, 21, 2, 0, 0).unwrap();
    let pos = calculate_solar_position(-37.8136, 144.9631, &utc, 11.0);

    // At summer solstice noon in Melbourne:
    // Sun elevation should be high (> 70 degrees)
    assert!(
        pos.elevation_deg > 70.0,
        "Summer noon elevation should be > 70°, got {:.1}°",
        pos.elevation_deg
    );

    // Sun direction vector should have positive Y (pointing above horizon)
    assert!(
        pos.sun_direction.y > 0.9,
        "Sun direction Y should be strongly positive, got {:.2}",
        pos.sun_direction.y
    );
}

#[test]
fn test_solar_position_night() {
    // Midnight in Melbourne
    let utc = Utc.with_ymd_and_hms(2025, 6, 21, 14, 0, 0).unwrap(); // 00:00 AEST
    let pos = calculate_solar_position(-37.8136, 144.9631, &utc, 10.0);

    // Sun should be below the horizon
    assert!(
        pos.elevation_deg < 0.0,
        "Night sun elevation should be below horizon (< 0°), got {:.1}°",
        pos.elevation_deg
    );
}
