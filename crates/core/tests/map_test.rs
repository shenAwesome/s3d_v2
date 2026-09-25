use std::sync::Arc;
use s3d_core::gis::map::{Basemap, Ground, LayerCollection, Map};
use s3d_core::gis::layer::{
    ElevationLayer, FeatureLayer, GraphicsLayer,
    GroupLayer, Layer, LayerType, TileLayer,
};
use s3d_core::gis::graphic::Graphic;
use s3d_core::gis::geometry::SpatialReference;

#[test]
fn test_default_map_creation() {
    let map = Map::new();
    assert_eq!(map.title, "Untitled Map");
    assert_eq!(map.spatial_reference, SpatialReference::Wgs84);
    assert!(map.basemap.is_some());
    assert_eq!(map.layers.len(), 0);
}

#[test]
fn test_basemap_presets() {
    let osm = Basemap::osm();
    assert_eq!(osm, Basemap::OpenStreetMap);
    assert_eq!(osm.display_name(), "OpenStreetMap Standard");
    assert!(osm.tile_url(0, 0, 0).unwrap().contains("openstreetmap.org"));

    let satellite = Basemap::esri_imagery();
    assert_eq!(satellite, Basemap::EsriImagery);
    assert!(satellite.tile_url(0, 0, 0).unwrap().contains("World_Imagery"));

    let streets = Basemap::esri_streets();
    assert_eq!(streets, Basemap::EsriStreet);

    let topo = Basemap::esri_topo();
    assert_eq!(topo, Basemap::EsriTopo);

    let custom = Basemap::custom("https://my.server/{z}/{x}/{y}.png");
    assert_eq!(custom.tile_url(1, 2, 3).unwrap(), "https://my.server/1/2/3.png");
}

#[test]
fn test_layer_collection_crud_and_reorder() {
    let collection = LayerCollection::new();
    assert!(collection.is_empty());
    assert_eq!(collection.revision(), 0);

    let l1 = Arc::new(TileLayer::new("layer1", "https://example.com/{z}/{x}/{y}.png"));
    let l2 = Arc::new(TileLayer::new("layer2", "https://example.com/{z}/{x}/{y}.png"));
    let l3 = Arc::new(TileLayer::new("layer3", "https://example.com/{z}/{x}/{y}.png"));

    collection.add(l1.clone());
    collection.add(l2.clone());
    collection.add(l3.clone());

    assert_eq!(collection.len(), 3);
    assert_eq!(collection.revision(), 3);
    assert_eq!(collection.get(0).unwrap().id(), "layer1");
    assert_eq!(collection.get(1).unwrap().id(), "layer2");
    assert_eq!(collection.get(2).unwrap().id(), "layer3");

    // Lookup by ID
    assert!(collection.find_by_id("layer2").is_some());
    assert!(collection.find_by_id("non_existent").is_none());

    // Reorder: move index 0 ("layer1") to index 2 (end)
    let reordered = collection.reorder(0, 2);
    assert!(reordered);
    assert_eq!(collection.get(0).unwrap().id(), "layer2");
    assert_eq!(collection.get(1).unwrap().id(), "layer3");
    assert_eq!(collection.get(2).unwrap().id(), "layer1");

    // Remove by ID
    let removed = collection.remove_by_id("layer3");
    assert!(removed.is_some());
    assert_eq!(collection.len(), 2);
    assert_eq!(collection.get(0).unwrap().id(), "layer2");
    assert_eq!(collection.get(1).unwrap().id(), "layer1");

    // Clear
    collection.clear();
    assert!(collection.is_empty());
}

#[test]
fn test_ground_surface_and_elevation() {
    let mut ground = Ground::new();
    assert_eq!(ground.elevation_exaggeration, 1.0);
    assert!(ground.layers.is_empty());

    let dem_layer = Arc::new(ElevationLayer::mapbox_terrain_rgb(
        "mapbox_dem",
        "https://api.mapbox.com/v4/mapbox.terrain-rgb/{z}/{x}/{y}.pngraw",
    ));
    ground.add_layer(dem_layer.clone());
    assert_eq!(ground.layers.len(), 1);
    assert_eq!(ground.layers.get(0).unwrap().id(), "mapbox_dem");
    assert_eq!(ground.layers.get(0).unwrap().layer_type(), LayerType::Elevation);

    ground.elevation_exaggeration = 2.0;
    assert_eq!(ground.elevation_exaggeration, 2.0);

    let removed = ground.remove_layer("mapbox_dem");
    assert!(removed.is_some());
    assert!(ground.layers.is_empty());
}

#[test]
fn test_operational_feature_layer() {
    let mut feature_layer = FeatureLayer::feature_layer("melbourne_cbd", "Melbourne CBD");
    assert_eq!(feature_layer.id(), "melbourne_cbd");
    assert_eq!(feature_layer.title(), "Melbourne CBD");
    assert_eq!(feature_layer.layer_type(), LayerType::Vectors);
    assert!(feature_layer.visible());
    assert_eq!(feature_layer.opacity(), 1.0);

    feature_layer.set_opacity(0.75);
    assert_eq!(feature_layer.opacity(), 0.75);

    feature_layer.set_visible(false);
    assert!(!feature_layer.visible());

    // Add a 3D extruded box feature
    feature_layer.add_box_feature(
        "box_1",
        "Rialto Tower",
        glam::Vec3::new(-50.0, 0.0, -50.0),
        glam::Vec3::new(50.0, 251.0, 50.0),
        [0.2, 0.5, 0.8, 1.0],
    );
    assert_eq!(feature_layer.features.len(), 1);
    assert_eq!(feature_layer.features[0].name, "Rialto Tower");
    assert!(feature_layer.features[0].mesh.is_some());
}

#[test]
fn test_graphics_layer() {
    let mut graphics_layer = GraphicsLayer::new("pins", "Map Pins");
    assert_eq!(graphics_layer.id(), "pins");
    assert_eq!(graphics_layer.title(), "Map Pins");
    assert_eq!(graphics_layer.layer_type(), LayerType::Graphics);
    assert_eq!(graphics_layer.graphics().len(), 0);

    let graphic = Graphic::point("pin_1", 144.9631, -37.8136, 0.0);
    graphics_layer.add(graphic);
    assert_eq!(graphics_layer.graphics().len(), 1);
    assert_eq!(graphics_layer.graphics()[0].id, "pin_1");

    let removed = graphics_layer.remove("pin_1");
    assert!(removed.is_some());
    assert_eq!(graphics_layer.graphics().len(), 0);
}

#[test]
fn test_group_layer() {
    let group = GroupLayer::new("infrastructure", "City Infrastructure");
    assert_eq!(group.id(), "infrastructure");
    assert_eq!(group.title(), "City Infrastructure");
    assert_eq!(group.layer_type(), LayerType::Group);

    let roads = Arc::new(FeatureLayer::feature_layer("roads", "Road Network"));
    let bridges = Arc::new(FeatureLayer::feature_layer("bridges", "Bridges"));

    group.layers.add(roads);
    group.layers.add(bridges);
    assert_eq!(group.layers.len(), 2);
    assert_eq!(group.layers.get(0).unwrap().id(), "roads");
    assert_eq!(group.layers.get(1).unwrap().id(), "bridges");
}

#[test]
fn test_map_creation_and_layers() {
    let mut map = Map::new();
    map.basemap = Some(Basemap::esri_imagery());
    assert_eq!(map.basemap, Some(Basemap::esri_imagery()));

    map.add_layer(FeatureLayer::feature_layer("buildings", "3D Buildings"));
    map.add_layer(GraphicsLayer::new("sketches", "Measurement Markup"));

    assert_eq!(map.layers.len(), 2);
    assert_eq!(map.layers[0].id(), "buildings");
    assert_eq!(map.layers[1].id(), "sketches");
}

#[test]
fn test_map_viewing_mode_and_auto_switch() {
    use s3d_core::gis::map::ViewingMode;

    // 1. Direct Map property initialization
    let mut map = Map::new();
    map.title = "Auto Switch Scene".to_string();
    map.basemap = Some(Basemap::osm());
    map.viewing_mode = ViewingMode::Auto { threshold_altitude: 60_000.0 };
    map.sync_viewing_mode_if_changed();

    assert_eq!(map.title, "Auto Switch Scene");
    assert_eq!(map.auto_switch_altitude, Some(60_000.0));

    // 2. Test runtime adjustment
    map.auto_switch_altitude = Some(45_000.0);
    assert_eq!(map.auto_switch_altitude, Some(45_000.0));

    // 3. Test transition to Globe
    map.transition_to_globe();
    assert_eq!(map.projection_mode, s3d_core::ProjectionMode::GlobeECEF);

    // 4. Test transition back to Planar: default must land with Heading 0° and Tilt 45°
    map.transition_to_planar_at_geo_with_pose(-37.8136, 144.9631, 2500.0, 0.0, 45.0);
    assert_eq!(map.projection_mode, s3d_core::ProjectionMode::PlanarENU);
    assert_eq!(map.camera.yaw, 0.0, "Heading must be 0.0 (North)");
    assert_eq!(map.camera.target_yaw, 0.0);
    
    // Tilt 45° implies pitch = 45° (pi/4 rad)
    let expected_pitch = 45.0f32.to_radians();
    assert!((map.camera.pitch - expected_pitch).abs() < 1e-5, "Tilt must be 45° (pitch ~ 0.785 rad)");
    assert!((map.camera.target_pitch - expected_pitch).abs() < 1e-5);

    // 5. Test transition with custom pose (e.g. Heading 90° East, Tilt 30°)
    map.transition_to_globe();
    map.transition_to_planar_at_geo_with_pose(-37.8136, 144.9631, 2500.0, 90.0, 30.0);
    assert_eq!(map.projection_mode, s3d_core::ProjectionMode::PlanarENU);
    assert!((map.camera.yaw - 90.0f32.to_radians()).abs() < 1e-5, "Heading must be 90.0 (East)");
    let expected_tilt_pitch = (90.0f32 - 30.0f32).to_radians(); // 60 deg pitch
    assert!((map.camera.pitch - expected_tilt_pitch).abs() < 1e-5, "Tilt must be 30° (pitch = 60°)");
}

#[test]
fn test_map_origin_xyz_configuration() {
    use s3d_core::gis::map::ViewingMode;

    // 1. Build map with origin [x: lon, y: lat, z: elevation]
    let mut map = Map::from_origin([144.9631, -37.8136, 10.0]);
    map.basemap = Some(Basemap::osm());
    map.viewing_mode = ViewingMode::Auto { threshold_altitude: 50_000.0 };

    assert_eq!(map.origin().longitude, 144.9631);
    assert_eq!(map.origin().latitude, -37.8136);
    assert_eq!(map.origin().elevation, 10.0);

    // 2. Map set_origin accepts [x, y, z] directly
    map.set_origin([151.2093, -33.8688, 0.0]); // Sydney CBD
    assert_eq!(map.origin().longitude, 151.2093);
    assert_eq!(map.origin().latitude, -33.8688);
}

#[test]
fn test_polymorphic_layers_lifecycle() {
    use s3d_core::Map;
    use s3d_core::gis::layer::{FeatureLayer, IntegratedMeshLayer, SceneLayer};

    let mut map = Map::default();
    assert_eq!(map.layers.len(), 0);

    // 1. Add FeatureLayer
    let feat_layer = FeatureLayer::new("buildings", "Melbourne Buildings", LayerType::Buildings, [1.0, 1.0, 1.0, 1.0]);
    let idx1 = map.add_layer(feat_layer);
    assert_eq!(idx1, 0);
    assert_eq!(map.layers.len(), 1);

    // 2. Add SceneLayer (I3S)
    let scene_layer = SceneLayer::new("i3s_cbd", "Melbourne CBD 3D", "https://example.com/i3s/SceneServer/layers/0");
    let idx2 = map.add_layer(scene_layer);
    assert_eq!(idx2, 1);
    assert_eq!(map.layers.len(), 2);

    // 3. Add IntegratedMeshLayer (3D Tiles)
    let mesh_layer = IntegratedMeshLayer::new("mesh_3dtiles", "Geelong 3D Tiles", "https://example.com/tileset.json");
    let idx3 = map.add_layer(mesh_layer);
    assert_eq!(idx3, 2);
    assert_eq!(map.layers.len(), 3);

    // 4. Downcasting via get_layer and get_layer_mut
    assert!(map.get_layer::<FeatureLayer>("buildings").is_some());
    assert!(map.get_layer::<SceneLayer>("i3s_cbd").is_some());
    assert!(map.get_layer::<IntegratedMeshLayer>("mesh_3dtiles").is_some());
    assert!(map.get_layer::<FeatureLayer>("i3s_cbd").is_none());

    // Mutate via downcast
    if let Some(layer) = map.get_layer_mut::<SceneLayer>("i3s_cbd") {
        layer.set_opacity(0.5);
        assert_eq!(layer.opacity(), 0.5);
    }

    // 5. Remove layer
    assert!(map.remove_layer("i3s_cbd"));
    assert_eq!(map.layers.len(), 2);
    assert!(map.get_layer::<SceneLayer>("i3s_cbd").is_none());
    assert!(!map.remove_layer("i3s_cbd")); // already removed

    // 6. Clear layers
    map.clear_layers();
    assert_eq!(map.layers.len(), 0);
}

#[test]
fn test_terrain_syntax_and_classes() {
    use s3d_core::gis::terrain::{AwsTerrain, EsriTerrain, Terrain, TerrainProvider};
    use s3d_core::Map;

    let mut map = Map::default();

    // 1. Terrain is None by default
    assert!(map.terrain.is_none());
    assert!(!map.terrain_mgr.is_enabled);

    // 2. Setting AwsTerrain directly
    map.terrain = Some(AwsTerrain::new().with_exaggeration(1.5).into());
    map.sync_terrain_if_changed();

    assert!(map.terrain.is_some());
    assert!(map.terrain_mgr.is_enabled);
    assert_eq!(map.terrain_mgr.provider, TerrainProvider::AwsTerrarium);
    assert_eq!(map.terrain_mgr.height_exaggeration, 1.5);

    // 3. Setting EsriTerrain directly
    map.terrain = Some(EsriTerrain::new().with_exaggeration(2.0).into());
    map.sync_terrain_if_changed();

    assert!(map.terrain.is_some());
    assert!(map.terrain_mgr.is_enabled);
    assert_eq!(map.terrain_mgr.provider, TerrainProvider::EsriTerrain3D);
    assert_eq!(map.terrain_mgr.height_exaggeration, 2.0);

    // 4. Setting to None disables terrain
    map.terrain = None;
    map.sync_terrain_if_changed();

    assert!(map.terrain.is_none());
    assert!(!map.terrain_mgr.is_enabled);

    // 5. Using Terrain::aws() and Terrain::esri() constructors
    let t_aws = Terrain::aws().with_exaggeration(3.0);
    assert_eq!(t_aws.provider(), TerrainProvider::AwsTerrarium);
    assert_eq!(t_aws.exaggeration(), 3.0);

    let t_esri = Terrain::esri().with_exaggeration(0.5);
    assert_eq!(t_esri.provider(), TerrainProvider::EsriTerrain3D);
    assert_eq!(t_esri.exaggeration(), 0.5);

    // 6. Direct map.terrain assignment
    map.terrain = Some(AwsTerrain::new().with_exaggeration(1.2).into());
    map.sync_terrain_if_changed();
    assert!(map.terrain.is_some());
    assert!(map.terrain_mgr.is_enabled);
    assert_eq!(map.terrain_mgr.height_exaggeration, 1.2);

    map.terrain = None;
    map.sync_terrain_if_changed();
    assert!(map.terrain.is_none());
    assert!(!map.terrain_mgr.is_enabled);
}

#[test]
fn test_basemap_syntax_and_switching() {
    let mut map = Map::default();

    // Default is OSM
    assert_eq!(map.basemap, Some(Basemap::osm()));

    // Switch to Esri Imagery
    map.basemap = Some(Basemap::esri_imagery());
    map.sync_basemap_if_changed();
    assert_eq!(map.basemap, Some(Basemap::esri_imagery()));

    // Switch to Esri Streets
    map.basemap = Some(Basemap::esri_streets());
    map.sync_basemap_if_changed();
    assert_eq!(map.basemap, Some(Basemap::esri_streets()));

    // Switch to None (grid mode)
    map.basemap = None;
    map.sync_basemap_if_changed();
    assert_eq!(map.basemap, None);
}
