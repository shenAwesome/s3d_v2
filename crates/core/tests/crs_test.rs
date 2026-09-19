use s3d_core::gis::crs::{GeoCoord, ProjectOrigin, haversine_distance, wgs84_to_web_mercator, web_mercator_to_wgs84};
use glam::Vec3;

#[test]
fn test_melbourne_origin_roundtrip() {
    let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
    let origin = ProjectOrigin::from_geo(melbourne);

    // Origin converted to local coordinates must be approximately (0, 0, 0)
    let local = origin.geo_to_local(&melbourne);
    assert!(local.length() < 1e-4, "Origin in local coords must be (0,0,0), got {:?}", local);

    // Test a point 500m East, 100m North, 25m Up
    let offset_pt = Vec3::new(500.0, 25.0, -100.0); // local ENU: X=East, Y=Up, -Z=North
    let geo_pt = origin.local_to_geo(offset_pt);

    // Roundtrip back to local
    let roundtrip_local = origin.geo_to_local(&geo_pt);
    assert!((roundtrip_local.x - offset_pt.x).abs() < 1e-3, "X mismatch: {}", roundtrip_local.x);
    assert!((roundtrip_local.y - offset_pt.y).abs() < 1e-3, "Y mismatch: {}", roundtrip_local.y);
    assert!((roundtrip_local.z - offset_pt.z).abs() < 1e-3, "Z mismatch: {}", roundtrip_local.z);
}

#[test]
fn test_geodesic_distance_melbourne_to_sydney() {
    let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
    let sydney = GeoCoord::new(-33.8688, 151.2093, 0.0);

    let dist_km = haversine_distance(
        melbourne.latitude,
        melbourne.longitude,
        sydney.latitude,
        sydney.longitude,
    ) / 1000.0;

    // Great circle distance between Melbourne and Sydney is ~713-714 km
    assert!(
        (dist_km - 714.0).abs() < 5.0,
        "Distance should be ~714 km, got {:.2} km", dist_km
    );
}

#[test]
fn test_web_mercator_roundtrip() {
    let lat = -37.8136;
    let lon = 144.9631;

    let (mx, my) = wgs84_to_web_mercator(lat, lon);
    let (back_lat, back_lon) = web_mercator_to_wgs84(mx, my);

    assert!((back_lat - lat).abs() < 1e-6);
    assert!((back_lon - lon).abs() < 1e-6);
}
