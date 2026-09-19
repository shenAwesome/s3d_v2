use s3d_core::gis::geojson_loader::parse_geojson;
use s3d_core::gis::crs::{ProjectOrigin, GeoCoord};

#[test]
fn test_parse_sample_buildings_geojson() {
    let geojson_str = include_str!("../../../assets/sample_buildings.geojson");
    let origin = ProjectOrigin::from_geo(GeoCoord::new(-37.8136, 144.9631, 0.0));

    let dataset = parse_geojson(geojson_str, Some(origin)).expect("Failed to parse sample buildings GeoJSON");

    assert!(!dataset.features.is_empty(), "Dataset must contain features");

    for feature in &dataset.features {
        assert!(!feature.name.is_empty());
        assert!(feature.height > 0.0, "Building height must be positive, got {}", feature.height);
        assert!(!feature.geo_polygons.is_empty(), "Building must have polygon coordinates");
    }
}
