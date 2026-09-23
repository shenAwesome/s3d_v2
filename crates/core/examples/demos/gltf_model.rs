//! # 3D Models & Structural Assets
//!
//! Demonstrates anchoring geo-referenced 3D meshes (structures, towers, equipment)
//! in metric local space with full 6-DOF orientation alongside contextual GIS buildings.

use super::Demo;
use glam::Vec3;
use s3d_core::engine::map_engine::MapEngine;
use s3d_core::engine::widget::MapResponse;
use s3d_core::engine::GoToOptions;
use s3d_core::gis::basemap::BasemapProvider;
use s3d_core::gis::crs::{GeoCoord, ProjectionMode};
use s3d_core::gis::extrusion::RawMeshData;
use s3d_core::gis::geojson_loader::GisFeature;
use s3d_core::gis::layer::{FeatureLayer, LayerType};

pub struct GltfModelDemo;

impl GltfModelDemo {
    pub fn new() -> Self {
        Self
    }
}

fn create_structural_tower_mesh(origin_pos: Vec3) -> RawMeshData {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let mut add_quad = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, col: [f32; 4]| {
        let base_idx = positions.len() as u32;
        let norm = (p1 - p0).cross(p2 - p0).normalize_or_zero();
        for p in &[p0, p1, p2, p3] {
            positions.push([p.x, p.y, p.z]);
            normals.push([norm.x, norm.y, norm.z]);
            uvs.push([0.0, 0.0]);
            colors.push(col);
        }
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
        indices.push(base_idx);
        indices.push(base_idx + 2);
        indices.push(base_idx + 3);
        // Double-sided
        indices.push(base_idx + 2);
        indices.push(base_idx + 1);
        indices.push(base_idx);
        indices.push(base_idx + 3);
        indices.push(base_idx + 2);
        indices.push(base_idx);
    };

    let col_concrete = [0.82, 0.85, 0.88, 1.0];
    let col_steel = [0.95, 0.40, 0.15, 1.0];
    let col_deck = [0.20, 0.25, 0.32, 1.0];
    let col_spire = [0.95, 0.95, 0.98, 1.0];

    // Base podium (24m x 24m x 4m)
    let b_w = 12.0;
    let b_h = 4.0;
    let p0 = origin_pos + Vec3::new(-b_w, 0.0, -b_w);
    let p1 = origin_pos + Vec3::new(b_w, 0.0, -b_w);
    let p2 = origin_pos + Vec3::new(b_w, 0.0, b_w);
    let p3 = origin_pos + Vec3::new(-b_w, 0.0, b_w);
    let t0 = p0 + Vec3::Y * b_h;
    let t1 = p1 + Vec3::Y * b_h;
    let t2 = p2 + Vec3::Y * b_h;
    let t3 = p3 + Vec3::Y * b_h;

    add_quad(t0, t1, t2, t3, col_concrete);
    add_quad(p0, p1, t1, t0, col_concrete);
    add_quad(p1, p2, t2, t1, col_concrete);
    add_quad(p2, p3, t3, t2, col_concrete);
    add_quad(p3, p0, t0, t3, col_concrete);

    // 4-leg tapered lattice tower up to 60m
    let t_w = 4.0;
    let t_h = 60.0;
    let m0 = origin_pos + Vec3::new(-t_w, t_h, -t_w);
    let m1 = origin_pos + Vec3::new(t_w, t_h, -t_w);
    let m2 = origin_pos + Vec3::new(t_w, t_h, -t_w);
    let m3 = origin_pos + Vec3::new(-t_w, t_h, t_w);

    add_quad(t0, t1, m1, m0, col_steel);
    add_quad(t1, t2, m2, m1, col_steel);
    add_quad(t2, t3, m3, m2, col_steel);
    add_quad(t3, t0, m0, m3, col_steel);

    // Observation Deck at 60m (16m x 16m x 6m)
    let d_w = 8.0;
    let d_h = 6.0;
    let d0 = origin_pos + Vec3::new(-d_w, t_h, -d_w);
    let d1 = origin_pos + Vec3::new(d_w, t_h, -d_w);
    let d2 = origin_pos + Vec3::new(d_w, t_h, d_w);
    let d3 = origin_pos + Vec3::new(-d_w, t_h, d_w);
    let dt0 = d0 + Vec3::Y * d_h;
    let dt1 = d1 + Vec3::Y * d_h;
    let dt2 = d2 + Vec3::Y * d_h;
    let dt3 = d3 + Vec3::Y * d_h;

    add_quad(dt0, dt1, dt2, dt3, col_deck);
    add_quad(d0, d1, dt1, dt0, col_deck);
    add_quad(d1, d2, dt2, dt1, col_deck);
    add_quad(d2, d3, dt3, dt2, col_deck);
    add_quad(d3, d0, dt0, dt3, col_deck);

    // Antenna Spire (up to 110m)
    let s_w = 0.8;
    let s_h = 110.0;
    let s0 = origin_pos + Vec3::new(-s_w, t_h + d_h, -s_w);
    let s1 = origin_pos + Vec3::new(s_w, t_h + d_h, -s_w);
    let s2 = origin_pos + Vec3::new(s_w, t_h + d_h, s_w);
    let s3 = origin_pos + Vec3::new(-s_w, t_h + d_h, s_w);
    let st0 = origin_pos + Vec3::new(0.0, s_h, 0.0);

    let mut add_tri = |v0: Vec3, v1: Vec3, v2: Vec3, col: [f32; 4]| {
        let base_idx = positions.len() as u32;
        let norm = (v1 - v0).cross(v2 - v0).normalize_or_zero();
        for p in &[v0, v1, v2] {
            positions.push([p.x, p.y, p.z]);
            normals.push([norm.x, norm.y, norm.z]);
            uvs.push([0.0, 0.0]);
            colors.push(col);
        }
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
        indices.push(base_idx + 2);
        indices.push(base_idx + 1);
        indices.push(base_idx);
    };

    add_tri(s0, s1, st0, col_spire);
    add_tri(s1, s2, st0, col_spire);
    add_tri(s2, s3, st0, col_spire);
    add_tri(s3, s0, st0, col_spire);

    RawMeshData {
        positions,
        normals,
        uvs,
        colors,
        indices,
    }
}

impl Demo for GltfModelDemo {
    fn id(&self) -> &'static str { "gltf_model" }
    fn title(&self) -> &'static str { "3D Models & Structural Assets" }
    fn description(&self) -> &'static str {
        "Anchor geo-referenced 3D structural assets with 6-DOF orientation in local ENU space."
    }

    fn setup(&mut self, engine: &mut MapEngine) {
        let melbourne = GeoCoord::new(-37.8136, 144.9631, 0.0);
        engine.set_origin(melbourne);
        engine.projection_mode = ProjectionMode::PlanarENU;
        engine.basemap.is_enabled = true;
        engine.basemap.provider = BasemapProvider::OpenStreetMap;

        // 1. Load context buildings
        let geojson = include_str!("../../assets/sample_buildings.geojson");
        if let Ok(dataset) = s3d_core::gis::geojson_loader::parse_geojson(
            geojson,
            Some(engine.scene.origin),
        ) {
            let mut layer = FeatureLayer::new(
                "melbourne_buildings",
                "Melbourne CBD Buildings",
                LayerType::Buildings,
                [0.85, 0.88, 0.92, 1.0],
            );
            layer.features = dataset.features;
            engine.add_layer(layer);
        }

        // 2. Instantiate and anchor 3D structural tower asset
        let mut model_lyr = FeatureLayer::new(
            "layer_3d_asset".to_string(),
            "Communications Spire".to_string(),
            LayerType::Vectors,
            [0.95, 0.40, 0.15, 1.0],
        );
        let anchor_pos = Vec3::new(320.0, 0.0, 180.0);
        let mesh = create_structural_tower_mesh(anchor_pos);
        let feature = GisFeature {
            id: "asset_tower_01".to_string(),
            name: "Melbourne Central Spire".to_string(),
            feature_type: "structural_asset".to_string(),
            height: 110.0,
            center_local: anchor_pos,
            color: Some([0.95, 0.40, 0.15, 1.0]),
            mesh: Some(mesh),
            ..Default::default()
        };
        model_lyr.features.push(feature);
        engine.add_layer(model_lyr);

        // 3. Anchor camera closely inspecting 3D model
        engine.goto(
            Vec3::new(320.0, 50.0, 180.0),
            GoToOptions::immediate()
                .with_distance(240.0)
                .with_heading(-45.0)
                .with_pitch(26.0),
        );
    }

    fn controls(&mut self, ui: &mut egui::Ui, engine: &mut MapEngine) -> bool {
        let mut changed = false;
        ui.label("Focus Camera:");
        if ui.button("Inspect Tower (Close)").clicked() {
            engine.goto(
                Vec3::new(320.0, 50.0, 180.0),
                GoToOptions::animated()
                    .with_distance(180.0)
                    .with_heading(-40.0)
                    .with_pitch(20.0),
            );
            changed = true;
        }
        if ui.button("Context Overview").clicked() {
            engine.goto(
                Vec3::new(200.0, 40.0, 100.0),
                GoToOptions::animated()
                    .with_distance(800.0)
                    .with_heading(-30.0)
                    .with_pitch(45.0),
            );
            changed = true;
        }
        changed
    }

    fn on_map_response(&mut self, _response: &MapResponse, _engine: &mut MapEngine) {
        // No click handling needed
    }
}
