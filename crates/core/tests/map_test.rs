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
    assert!(map.tables.is_empty());

    // By default, OSM basemap is present in all_layers()
    let layers = map.all_layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].layer_type(), LayerType::Tile);
    assert_eq!(layers[0].title(), "OpenStreetMap");
}

#[test]
fn test_basemap_presets() {
    let osm = Basemap::osm();
    assert_eq!(osm.id, "osm");
    assert_eq!(osm.base_layers.len(), 1);
    assert_eq!(osm.base_layers.get(0).unwrap().title(), "OpenStreetMap");

    let satellite = Basemap::esri_imagery();
    assert_eq!(satellite.id, "esri_imagery");
    assert_eq!(satellite.base_layers.len(), 1);
    assert_eq!(satellite.base_layers.get(0).unwrap().title(), "Esri World Imagery (Satellite)");

    let streets = Basemap::esri_streets();
    assert_eq!(streets.id, "esri_streets");
    assert_eq!(streets.base_layers.len(), 1);

    let topo = Basemap::esri_topo();
    assert_eq!(topo.id, "esri_topo");
    assert_eq!(topo.base_layers.len(), 1);

    let blank = Basemap::none();
    assert_eq!(blank.id, "none");
    assert_eq!(blank.base_layers.len(), 0);
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
fn test_map_visual_stacking_order() {
    // 1. Map with Satellite Basemap
    let mut map = Map::with_basemap(Basemap::esri_imagery());

    // 2. Ground Elevation
    map.ground.add_layer(Arc::new(ElevationLayer::mapbox_terrain_rgb(
        "dem",
        "https://example.com/dem/{z}/{x}/{y}.png",
    )));

    // 3. Operational Vector & Graphics Layers
    map.add_layer(Arc::new(FeatureLayer::feature_layer("buildings", "3D Buildings")));
    map.add_layer(Arc::new(GraphicsLayer::new("sketches", "Measurement Markup")));

    // 4. Basemap Reference Layer (labels drawn on top)
    if let Some(basemap) = &mut map.basemap {
        basemap.reference_layers.add(Arc::new(TileLayer::new(
            "labels",
            "https://server.arcgisonline.com/.../{z}/{y}/{x}",
        )));
    }

    // Verify all_layers() returns strictly in bottom-to-top order:
    // [0] esri_imagery (basemap base)
    // [1] dem (ground elevation)
    // [2] buildings (operational)
    // [3] sketches (operational)
    // [4] labels (basemap reference / overlay)
    let layers = map.all_layers();
    assert_eq!(layers.len(), 5);
    assert_eq!(layers[0].id(), "esri_imagery_tiles");
    assert_eq!(layers[1].id(), "dem");
    assert_eq!(layers[2].id(), "buildings");
    assert_eq!(layers[3].id(), "sketches");
    assert_eq!(layers[4].id(), "labels");
}

#[test]
fn test_map_builder_and_auto_switch_viewing_mode() {
    use s3d_core::gis::map::ViewingMode;
    use s3d_core::MapEngine;

    // 1. Build Map with ViewingMode::Auto
    let map = Map::builder()
        .title("Auto Switch Scene")
        .basemap(Basemap::osm())
        .viewing_mode(ViewingMode::Auto { threshold_altitude: 60_000.0 })
        .build();

    assert_eq!(map.title, "Auto Switch Scene");
    assert_eq!(map.viewing_mode, ViewingMode::Auto { threshold_altitude: 60_000.0 });

    // 2. Initialize MapEngine from Map and verify threshold is synced
    let mut engine = MapEngine::from_map(map);
    assert_eq!(engine.auto_switch_altitude, Some(60_000.0));

    // 3. Test runtime adjustment
    engine.set_auto_projection_switch(Some(45_000.0));
    assert_eq!(engine.auto_switch_altitude, Some(45_000.0));

    // 4. Test transition to Globe
    engine.transition_to_globe();
    assert_eq!(engine.projection_mode, s3d_core::ProjectionMode::GlobeECEF);

    // 5. Test transition back to Planar: default must land with Heading 0° and Tilt 45°
    engine.transition_to_planar_at_geo(-37.8136, 144.9631, 2500.0);
    assert_eq!(engine.projection_mode, s3d_core::ProjectionMode::PlanarENU);
    assert_eq!(engine.camera.yaw, 0.0, "Heading must be 0.0 (North)");
    assert_eq!(engine.camera.target_yaw, 0.0);
    
    // Tilt 45° implies pitch = 45° (pi/4 rad)
    let expected_pitch = 45.0f32.to_radians();
    assert!((engine.camera.pitch - expected_pitch).abs() < 1e-5, "Tilt must be 45° (pitch ~ 0.785 rad)");
    assert!((engine.camera.target_pitch - expected_pitch).abs() < 1e-5);

    // 6. Test transition with custom pose (e.g. Heading 90° East, Tilt 30°)
    engine.transition_to_globe();
    engine.transition_to_planar_at_geo_with_pose(-37.8136, 144.9631, 2500.0, 90.0, 30.0);
    assert_eq!(engine.projection_mode, s3d_core::ProjectionMode::PlanarENU);
    assert!((engine.camera.yaw - 90.0f32.to_radians()).abs() < 1e-5, "Heading must be 90.0 (East)");
    let expected_tilt_pitch = (90.0f32 - 30.0f32).to_radians(); // 60 deg pitch
    assert!((engine.camera.pitch - expected_tilt_pitch).abs() < 1e-5, "Tilt must be 30° (pitch = 60°)");
}

#[test]
fn test_map_origin_xyz_configuration() {
    use s3d_core::gis::map::ViewingMode;
    use s3d_core::MapEngine;

    // 1. Build map with origin [x: lon, y: lat, z: elevation]
    let map = Map::builder()
        .basemap(Basemap::osm())
        .origin([144.9631, -37.8136, 10.0])
        .viewing_mode(ViewingMode::Auto { threshold_altitude: 50_000.0 })
        .build();

    assert_eq!(map.origin.longitude, 144.9631);
    assert_eq!(map.origin.latitude, -37.8136);
    assert_eq!(map.origin.elevation, 10.0);

    // 2. MapEngine::from_map inherits map.origin automatically
    let mut engine = MapEngine::from_map(map);
    assert_eq!(engine.scene.origin.origin.longitude, 144.9631);
    assert_eq!(engine.scene.origin.origin.latitude, -37.8136);
    assert_eq!(engine.scene.origin.origin.elevation, 10.0);

    // 3. Engine set_origin accepts [x, y, z] directly
    engine.set_origin([151.2093, -33.8688, 0.0]); // Sydney CBD
    assert_eq!(engine.scene.origin.origin.longitude, 151.2093);
    assert_eq!(engine.scene.origin.origin.latitude, -33.8688);
}

#[test]
fn test_polymorphic_layers_lifecycle() {
    use s3d_core::MapEngine;
    use s3d_core::gis::layer::{FeatureLayer, IntegratedMeshLayer, SceneLayer};

    let mut engine = MapEngine::default();
    assert_eq!(engine.layers.len(), 0);

    // 1. Add FeatureLayer
    let feat_layer = FeatureLayer::new("buildings", "Melbourne Buildings", LayerType::Buildings, [1.0, 1.0, 1.0, 1.0]);
    let idx1 = engine.add_layer(feat_layer);
    assert_eq!(idx1, 0);
    assert_eq!(engine.layers.len(), 1);

    // 2. Add SceneLayer (I3S)
    let scene_layer = SceneLayer::new("i3s_cbd", "Melbourne CBD 3D", "https://example.com/i3s/SceneServer/layers/0");
    let idx2 = engine.add_layer(scene_layer);
    assert_eq!(idx2, 1);
    assert_eq!(engine.layers.len(), 2);

    // 3. Add IntegratedMeshLayer (3D Tiles)
    let mesh_layer = IntegratedMeshLayer::new("mesh_3dtiles", "Geelong 3D Tiles", "https://example.com/tileset.json");
    let idx3 = engine.add_layer(mesh_layer);
    assert_eq!(idx3, 2);
    assert_eq!(engine.layers.len(), 3);

    // 4. Downcasting via get_layer and get_layer_mut
    assert!(engine.get_layer::<FeatureLayer>("buildings").is_some());
    assert!(engine.get_layer::<SceneLayer>("i3s_cbd").is_some());
    assert!(engine.get_layer::<IntegratedMeshLayer>("mesh_3dtiles").is_some());
    assert!(engine.get_layer::<FeatureLayer>("i3s_cbd").is_none());

    // Mutate via downcast
    if let Some(layer) = engine.get_layer_mut::<SceneLayer>("i3s_cbd") {
        layer.set_opacity(0.5);
        assert_eq!(layer.opacity(), 0.5);
    }

    // 5. Remove layer
    assert!(engine.remove_layer("i3s_cbd"));
    assert_eq!(engine.layers.len(), 2);
    assert!(engine.get_layer::<SceneLayer>("i3s_cbd").is_none());
    assert!(!engine.remove_layer("i3s_cbd")); // already removed

    // 6. Clear layers
    engine.clear_layers();
    assert_eq!(engine.layers.len(), 0);
}

#[test]
fn test_terrain_syntax_and_classes() {
    use s3d_core::gis::terrain::{AwsTerrain, EsriTerrain, Terrain, TerrainProvider};
    use s3d_core::engine::MapEngine;

    let mut engine = MapEngine::default();

    // 1. Terrain is None by default
    assert!(engine.terrain.is_none());
    assert!(!engine.terrain_mgr.is_enabled);

    // 2. Setting AwsTerrain directly
    engine.terrain = Some(AwsTerrain::new().with_exaggeration(1.5).into());
    engine.sync_terrain_if_changed();

    assert!(engine.terrain.is_some());
    assert!(engine.terrain_mgr.is_enabled);
    assert_eq!(engine.terrain_mgr.provider, TerrainProvider::AwsTerrarium);
    assert_eq!(engine.terrain_mgr.height_exaggeration, 1.5);

    // 3. Setting EsriTerrain directly
    engine.terrain = Some(EsriTerrain::new().with_exaggeration(2.0).into());
    engine.sync_terrain_if_changed();

    assert!(engine.terrain.is_some());
    assert!(engine.terrain_mgr.is_enabled);
    assert_eq!(engine.terrain_mgr.provider, TerrainProvider::EsriTerrain3D);
    assert_eq!(engine.terrain_mgr.height_exaggeration, 2.0);

    // 4. Setting to None disables terrain
    engine.terrain = None;
    engine.sync_terrain_if_changed();

    assert!(engine.terrain.is_none());
    assert!(!engine.terrain_mgr.is_enabled);

    // 5. Using Terrain::aws() and Terrain::esri() constructors
    let t_aws = Terrain::aws().with_exaggeration(3.0);
    assert_eq!(t_aws.provider(), TerrainProvider::AwsTerrarium);
    assert_eq!(t_aws.exaggeration(), 3.0);

    let t_esri = Terrain::esri().with_exaggeration(0.5);
    assert_eq!(t_esri.provider(), TerrainProvider::EsriTerrain3D);
    assert_eq!(t_esri.exaggeration(), 0.5);

    // 6. engine.set_terrain helper
    engine.set_terrain(AwsTerrain::new().with_exaggeration(1.2));
    assert!(engine.terrain.is_some());
    assert!(engine.terrain_mgr.is_enabled);
    assert_eq!(engine.terrain_mgr.height_exaggeration, 1.2);

    engine.set_terrain(None);
    assert!(engine.terrain.is_none());
    assert!(!engine.terrain_mgr.is_enabled);
}


