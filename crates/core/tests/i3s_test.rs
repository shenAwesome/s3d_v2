use s3d_core::gis::crs::{GeoCoord, ProjectOrigin};
use s3d_core::gis::i3s::decoder::I3SGeometryDecoder;
use s3d_core::gis::i3s::spec::*;
use s3d_core::gis::i3s::I3SManager;

#[test]
fn test_live_layer_metadata_deserialization() {
    let json_str = include_str!(r"C:\Users\shenz\.gemini\antigravity\brain\4662649a-13b5-45c8-902e-1c0428524b34\scratch\live_layer_0.json");
    let layer: Result<I3SSceneLayer, _> = serde_json::from_str(json_str);
    match layer {
        Ok(l) => {
            assert_eq!(l.name.as_deref(), Some("AB_Melbourne_WM"));
        }
        Err(e) => {
            panic!("Failed to parse live layer metadata: {}", e);
        }
    }
}

#[test]
fn test_live_nodepage_deserialization() {
    let json_str = include_str!(r"C:\Users\shenz\.gemini\antigravity\brain\4662649a-13b5-45c8-902e-1c0428524b34\scratch\live_page_0.json");
    let page: Result<I3SNodePage, _> = serde_json::from_str(json_str);
    match page {
        Ok(p) => {
            assert_eq!(p.nodes.len(), 64);
            assert_eq!(p.nodes[0].index, 0);
        }
        Err(e) => {
            panic!("Failed to parse live nodepage: {}", e);
        }
    }
}

#[test]
fn test_live_geometry_decoding() {
    let geom_bytes = include_bytes!(r"C:\Users\shenz\.gemini\antigravity\brain\4662649a-13b5-45c8-902e-1c0428524b34\scratch\geom_6.bin");
    let geom_info = I3SGeometryInfo {
        definition: 0,
        resource: 6,
        vertex_count: 61071,
        feature_count: Some(113),
    };
    let geom_buf_def = I3SGeometryBufferDef {
        offset: Some(8),
        position: None,
        normal: None,
        uv0: None,
        color: None,
        feature_id: None,
        face_range: None,
    };
    // Node 7 obb center
    let obb_center = [144.97202295493415, -37.813348480379702, 152.68052402790636];
    let origin = ProjectOrigin::new(-37.8136, 144.9631, 0.0);

    let decoded = I3SGeometryDecoder::decode(
        geom_bytes,
        &geom_info,
        Some(&geom_buf_def),
        obb_center,
        true,
        &origin,
        [1.0, 1.0, 1.0, 1.0],
    ).expect("Geometry decode failed");

    assert_eq!(decoded.raw_mesh.positions.len(), 61071);
    assert_eq!(decoded.features.len(), 113);
}

#[test]
fn test_live_manager_traversal() {
    let origin = ProjectOrigin::new(-37.8136, 144.9631, 0.0);
    let mut manager = I3SManager::new(origin);
    manager.is_enabled = true;

    let layer_meta: I3SSceneLayer = serde_json::from_str(
        include_str!(r"C:\Users\shenz\.gemini\antigravity\brain\4662649a-13b5-45c8-902e-1c0428524b34\scratch\live_layer_0.json")
    ).unwrap();
    manager.layer_metadata = Some(layer_meta);

    let page_0: I3SNodePage = serde_json::from_str(
        include_str!(r"C:\Users\shenz\.gemini\antigravity\brain\4662649a-13b5-45c8-902e-1c0428524b34\scratch\live_page_0.json")
    ).unwrap();

    for node in page_0.nodes {
        manager.node_cache.insert(node.index, node);
    }
    manager.cached_pages.insert(0);

    // Camera looking at Melbourne CBD: target = (0, 50, 0), distance = 1200, heading = -40, pitch = 50
    let mut cam = s3d_core::renderer::camera::Camera::new(glam::Vec3::new(0.0, 50.0, 0.0), 1200.0);
    cam.yaw = -40.0f32.to_radians();
    cam.pitch = 50.0f32.to_radians();
    cam.fov_y = 45.0f32.to_radians();

    let visible = manager.calculate_visible_nodes(&cam, 1000.0, 700.0, &origin);
    println!("Visible nodes count: {}", visible.len());
    println!("Visible nodes: {:?}", visible);
    assert!(!visible.is_empty(), "Visible nodes should not be empty!");
}
