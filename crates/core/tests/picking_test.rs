use s3d_core::engine::map_engine::MapEngine;
use s3d_core::spatial::picking::Ray;
use glam::Vec3;

#[test]
fn test_ground_plane_intersection() {
    let map = MapEngine::default();

    // Ray starting at (0, 100, 0) pointing straight down (0, -1, 0)
    let ray = Ray {
        origin: Vec3::new(10.0, 100.0, -25.0),
        direction: Vec3::new(0.0, -1.0, 0.0),
    };

    let hit = map.intersect_ground(&ray).expect("Ray must hit ground plane");
    assert!((hit.x - 10.0).abs() < 1e-4);
    assert!((hit.y - 0.0).abs() < 1e-4);
    assert!((hit.z - (-25.0)).abs() < 1e-4);

    // Ray pointing away from ground (upward) should NOT hit
    let upward_ray = Ray {
        origin: Vec3::new(0.0, 10.0, 0.0),
        direction: Vec3::new(0.0, 1.0, 0.0),
    };
    assert!(map.intersect_ground(&upward_ray).is_none());
}

#[test]
fn test_screen_to_ray_center() {
    let map = MapEngine::default();
    let ray = map.screen_to_ray(400.0, 300.0, 800.0, 600.0);

    // Center screen ray direction should point forward along camera orientation
    assert!(ray.direction.length() > 0.99 && ray.direction.length() < 1.01);
}
