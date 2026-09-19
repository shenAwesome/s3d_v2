use s3d_core::gis::extrusion::extrude_polygon;
use glam::Vec2;

#[test]
fn test_building_footprint_extrusion() {
    // A 20m x 30m building rectangle
    let footprint = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(20.0, 0.0),
        Vec2::new(20.0, 30.0),
        Vec2::new(0.0, 30.0),
        Vec2::new(0.0, 0.0),
    ];
    let rings = vec![footprint];

    let base_elev = 5.0;
    let height = 25.0;

    let mesh = extrude_polygon(&rings, base_elev, height).expect("Extrusion must succeed");

    // Must have vertices, normals, UVs and indices
    assert!(!mesh.positions.is_empty());
    assert_eq!(mesh.positions.len(), mesh.normals.len());
    assert_eq!(mesh.positions.len(), mesh.uvs.len());
    assert!(!mesh.indices.is_empty());
    assert_eq!(mesh.indices.len() % 3, 0, "Indices must form complete triangles");

    // Check elevation range: min Y should be base_elev (5.0), max Y should be base_elev + height (30.0)
    let min_y = mesh.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = mesh.positions.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);

    assert!((min_y - 5.0).abs() < 1e-4, "Expected min Y = 5.0, got {}", min_y);
    assert!((max_y - 30.0).abs() < 1e-4, "Expected max Y = 30.0, got {}", max_y);

    // Verify all vertex indices are within bounds
    let max_idx = *mesh.indices.iter().max().unwrap() as usize;
    assert!(max_idx < mesh.positions.len(), "Index out of bounds");
}
