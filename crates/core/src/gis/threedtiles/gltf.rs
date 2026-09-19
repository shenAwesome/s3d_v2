use glam::{Vec2, Vec3};
use serde::Deserialize;
use std::collections::HashMap;

// ----------------------------------------------------
// glTF 2.0 JSON Deserialization Structs
// ----------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfAccessor {
    pub buffer_view: Option<usize>,
    pub byte_offset: Option<usize>,
    pub component_type: u32,
    pub count: usize,
    #[serde(rename = "type")]
    pub accessor_type: String,
    pub min: Option<Vec<f64>>,
    pub max: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfBufferView {
    pub buffer: usize,
    pub byte_offset: Option<usize>,
    pub byte_length: usize,
    pub byte_stride: Option<usize>,
    pub target: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfPrimitive {
    pub attributes: HashMap<String, usize>,
    pub indices: Option<usize>,
    pub material: Option<usize>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfMesh {
    pub primitives: Vec<GltfPrimitive>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfNode {
    pub mesh: Option<usize>,
    pub children: Option<Vec<usize>>,
    pub matrix: Option<[f32; 16]>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfScene {
    pub nodes: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfTextureInfo {
    pub index: usize,
    pub tex_coord: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfMaterialPbr {
    pub base_color_factor: Option<[f32; 4]>,
    pub base_color_texture: Option<GltfTextureInfo>,
    pub metallic_factor: Option<f32>,
    pub roughness_factor: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfMaterial {
    pub pbr_metallic_roughness: Option<GltfMaterialPbr>,
    pub emissive_texture: Option<GltfTextureInfo>,
    pub emissive_factor: Option<[f32; 3]>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfImage {
    pub buffer_view: Option<usize>,
    pub mime_type: Option<String>,
    pub uri: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfTexture {
    pub sampler: Option<usize>,
    pub source: Option<usize>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfRoot {
    pub accessors: Option<Vec<GltfAccessor>>,
    pub buffer_views: Option<Vec<GltfBufferView>>,
    pub meshes: Option<Vec<GltfMesh>>,
    pub nodes: Option<Vec<GltfNode>>,
    pub scenes: Option<Vec<GltfScene>>,
    pub scene: Option<usize>,
    pub materials: Option<Vec<GltfMaterial>>,
    pub images: Option<Vec<GltfImage>>,
    pub textures: Option<Vec<GltfTexture>>,
}

// ----------------------------------------------------
// Parsed glTF Primitive Model Data
// ----------------------------------------------------

#[derive(Debug, Clone)]
pub struct GltfParsedPrimitive {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub batch_ids: Vec<u32>,
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    pub image_rgba: Option<std::sync::Arc<(u32, u32, Vec<u8>)>>,
}

#[derive(Debug, Clone)]
pub struct GltfParsedModel {
    pub primitives: Vec<GltfParsedPrimitive>,
    pub image_rgba: Option<std::sync::Arc<(u32, u32, Vec<u8>)>>,
}

// ----------------------------------------------------
// glTF / GLB Binary Parser
// ----------------------------------------------------

pub struct GltfParser;

impl GltfParser {
    pub const GLB_MAGIC: u32 = 0x46546C67; // "glTF"
    pub const CHUNK_TYPE_JSON: u32 = 0x4E4F534A; // "JSON"
    pub const CHUNK_TYPE_BIN: u32 = 0x004E4942; // "BIN\0"

    pub fn parse_glb(bytes: &[u8]) -> Result<GltfParsedModel, String> {
        if bytes.len() < 12 {
            return Err("GLB payload is too short (< 12 bytes)".to_string());
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != Self::GLB_MAGIC {
            // Check if it's direct UTF-8 JSON text
            if let Ok(json_str) = std::str::from_utf8(bytes) {
                let gltf: GltfRoot = serde_json::from_str(json_str)
                    .map_err(|e| format!("Failed to parse glTF JSON: {}", e))?;
                return Self::extract_primitives(&gltf, &[]);
            }
            return Err(format!("Invalid GLB magic: 0x{:08X}", magic));
        }

        let _version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let total_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let effective_length = total_length.min(bytes.len());

        let mut offset = 12;
        let mut json_bytes: Option<&[u8]> = None;
        let mut bin_bytes: Option<&[u8]> = None;

        while offset + 8 <= effective_length {
            let chunk_length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            let chunk_type = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
            let chunk_start = offset + 8;
            let chunk_end = (chunk_start + chunk_length).min(effective_length);

            if chunk_type == Self::CHUNK_TYPE_JSON {
                json_bytes = Some(&bytes[chunk_start..chunk_end]);
            } else if chunk_type == Self::CHUNK_TYPE_BIN {
                bin_bytes = Some(&bytes[chunk_start..chunk_end]);
            }

            offset = chunk_start + chunk_length;
        }

        let json_data = json_bytes.ok_or_else(|| "Missing JSON chunk in GLB".to_string())?;
        let json_str = std::str::from_utf8(json_data)
            .map_err(|e| format!("Invalid UTF-8 in glTF JSON: {}", e))?;

        let gltf: GltfRoot = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse glTF JSON: {}", e))?;

        let bin_data = bin_bytes.unwrap_or(&[]);
        Self::extract_primitives(&gltf, bin_data)
    }

    fn extract_primitives(gltf: &GltfRoot, bin: &[u8]) -> Result<GltfParsedModel, String> {
        // 1. Decode all embedded images
        let mut decoded_images: Vec<Option<std::sync::Arc<(u32, u32, Vec<u8>)>>> = Vec::new();
        if let Some(images) = &gltf.images {
            for img in images {
                let mut img_rgba = None;
                if let Some(bv_idx) = img.buffer_view {
                    if let Some(bvs) = &gltf.buffer_views {
                        if let Some(bv) = bvs.get(bv_idx) {
                            let start = bv.byte_offset.unwrap_or(0);
                            let end = (start + bv.byte_length).min(bin.len());
                            if start < end {
                                let img_slice = &bin[start..end];
                                if let Ok(dyn_img) = image::load_from_memory(img_slice) {
                                    let rgba = dyn_img.to_rgba8();
                                    img_rgba = Some(std::sync::Arc::new((rgba.width(), rgba.height(), rgba.into_raw())));
                                }
                            }
                        }
                    }
                }
                decoded_images.push(img_rgba);
            }
        }

        // 2. Build Material cache: mat_idx -> (base_color: [f32; 4], image: Option<Arc<(u32, u32, Vec<u8>)>>)
        let mut material_cache: Vec<([f32; 4], Option<std::sync::Arc<(u32, u32, Vec<u8>)>>)> = Vec::new();
        if let Some(materials) = &gltf.materials {
            for mat in materials {
                let mut base_color = [1.0, 1.0, 1.0, 1.0];
                let mut texture_idx = None;

                if let Some(pbr) = &mat.pbr_metallic_roughness {
                    if let Some(color) = pbr.base_color_factor {
                        base_color = color;
                    }
                    if let Some(tex) = &pbr.base_color_texture {
                        texture_idx = Some(tex.index);
                    }
                }

                if texture_idx.is_none() {
                    if let Some(tex) = &mat.emissive_texture {
                        texture_idx = Some(tex.index);
                    }
                }

                let mut img_rgba = None;
                if let Some(t_idx) = texture_idx {
                    if let Some(textures) = &gltf.textures {
                        if let Some(tex) = textures.get(t_idx) {
                            if let Some(src_idx) = tex.source {
                                if let Some(Some(arc_img)) = decoded_images.get(src_idx) {
                                    img_rgba = Some(std::sync::Arc::clone(arc_img));
                                }
                            }
                        }
                    }
                }

                material_cache.push((base_color, img_rgba));
            }
        }

        let mut primitives = Vec::new();

        if let Some(nodes) = &gltf.nodes {
            if !nodes.is_empty() {
                let mut root_indices = Vec::new();
                if let Some(scenes) = &gltf.scenes {
                    let scene_idx = gltf.scene.unwrap_or(0);
                    if let Some(scene) = scenes.get(scene_idx).or_else(|| scenes.first()) {
                        if let Some(scene_nodes) = &scene.nodes {
                            root_indices.extend(scene_nodes.iter().copied());
                        }
                    }
                }

                if root_indices.is_empty() {
                    let mut child_set = std::collections::HashSet::new();
                    for n in nodes {
                        if let Some(children) = &n.children {
                            for &c in children {
                                child_set.insert(c);
                            }
                        }
                    }
                    for i in 0..nodes.len() {
                        if !child_set.contains(&i) {
                            root_indices.push(i);
                        }
                    }
                }

                let mut visited = std::collections::HashSet::new();
                for root_idx in root_indices {
                    Self::traverse_node(root_idx, &glam::Mat4::IDENTITY, gltf, bin, &material_cache, &mut primitives, &mut visited);
                }
            }
        }

        // Fallback: If no nodes were found or processed, iterate over meshes directly with Identity transform
        if primitives.is_empty() {
            if let Some(meshes) = &gltf.meshes {
                for mesh in meshes {
                    for prim in &mesh.primitives {
                        Self::process_primitive(prim, &glam::Mat4::IDENTITY, gltf, bin, &material_cache, &mut primitives);
                    }
                }
            }
        }

        let first_image = primitives.iter().find_map(|p| p.image_rgba.clone());
        Ok(GltfParsedModel { primitives, image_rgba: first_image })
    }

    fn traverse_node(
        node_idx: usize,
        parent_mat: &glam::Mat4,
        gltf: &GltfRoot,
        bin: &[u8],
        material_cache: &[([f32; 4], Option<std::sync::Arc<(u32, u32, Vec<u8>)>>)],
        primitives: &mut Vec<GltfParsedPrimitive>,
        visited: &mut std::collections::HashSet<usize>,
    ) {
        if visited.contains(&node_idx) {
            return;
        }
        visited.insert(node_idx);

        let nodes = match &gltf.nodes {
            Some(n) => n,
            None => return,
        };
        let node = match nodes.get(node_idx) {
            Some(n) => n,
            None => return,
        };

        let local_mat = if let Some(m) = node.matrix {
            glam::Mat4::from_cols_array(&m)
        } else {
            let t = node.translation.map(|v| glam::Vec3::from_array(v)).unwrap_or(glam::Vec3::ZERO);
            let r = node.rotation.map(|v| glam::Quat::from_xyzw(v[0], v[1], v[2], v[3])).unwrap_or(glam::Quat::IDENTITY);
            let s = node.scale.map(|v| glam::Vec3::from_array(v)).unwrap_or(glam::Vec3::ONE);
            glam::Mat4::from_scale_rotation_translation(s, r, t)
        };

        let world_mat = *parent_mat * local_mat;

        if let Some(mesh_idx) = node.mesh {
            if let Some(meshes) = &gltf.meshes {
                if let Some(mesh) = meshes.get(mesh_idx) {
                    for prim in &mesh.primitives {
                        Self::process_primitive(prim, &world_mat, gltf, bin, material_cache, primitives);
                    }
                }
            }
        }

        if let Some(children) = &node.children {
            for &child_idx in children {
                Self::traverse_node(child_idx, &world_mat, gltf, bin, material_cache, primitives, visited);
            }
        }
    }

    fn process_primitive(
        prim: &GltfPrimitive,
        world_mat: &glam::Mat4,
        gltf: &GltfRoot,
        bin: &[u8],
        material_cache: &[([f32; 4], Option<std::sync::Arc<(u32, u32, Vec<u8>)>>)],
        out_prims: &mut Vec<GltfParsedPrimitive>,
    ) {
        let mut parsed_prim = GltfParsedPrimitive {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            batch_ids: Vec::new(),
            indices: Vec::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            image_rgba: None,
        };

        // Material base color factor & per-primitive texture image
        if let Some(mat_idx) = prim.material {
            if let Some((color, img)) = material_cache.get(mat_idx) {
                parsed_prim.base_color = *color;
                parsed_prim.image_rgba = img.clone();
            }
        }

        // Positions
        if let Some(&pos_accessor_idx) = prim.attributes.get("POSITION") {
            if let Some(pos_floats) = Self::read_accessor_floats(gltf, bin, pos_accessor_idx) {
                for chunk in pos_floats.chunks_exact(3) {
                    let local_p = glam::Vec3::new(chunk[0], chunk[1], chunk[2]);
                    let transformed_p = world_mat.transform_point3(local_p);
                    parsed_prim.positions.push(transformed_p);
                }
            }
        }

        let vertex_count = parsed_prim.positions.len();
        if vertex_count == 0 {
            return;
        }

        // Normals
        let normal_mat = glam::Mat3::from_mat4(*world_mat).inverse().transpose();
        if let Some(&norm_accessor_idx) = prim.attributes.get("NORMAL") {
            if let Some(norm_floats) = Self::read_accessor_floats(gltf, bin, norm_accessor_idx) {
                for chunk in norm_floats.chunks_exact(3) {
                    let local_n = glam::Vec3::new(chunk[0], chunk[1], chunk[2]);
                    let transformed_n = (normal_mat * local_n).normalize_or_zero();
                    parsed_prim.normals.push(if transformed_n.length_squared() > 0.1 {
                        transformed_n
                    } else {
                        glam::Vec3::Y
                    });
                }
            }
        }

        // Pad normals if missing
        while parsed_prim.normals.len() < vertex_count {
            parsed_prim.normals.push(glam::Vec3::Y);
        }

        // UVs
        if let Some(&uv_accessor_idx) = prim.attributes.get("TEXCOORD_0") {
            if let Some(uv_floats) = Self::read_accessor_floats(gltf, bin, uv_accessor_idx) {
                for chunk in uv_floats.chunks_exact(2) {
                    parsed_prim.uvs.push(glam::Vec2::new(chunk[0], chunk[1]));
                }
            }
        }

        // Pad UVs if missing
        while parsed_prim.uvs.len() < vertex_count {
            parsed_prim.uvs.push(glam::Vec2::ZERO);
        }

        // Batch IDs (_BATCHID)
        if let Some(&batch_accessor_idx) = prim.attributes.get("_BATCHID").or_else(|| prim.attributes.get("BATCHID")) {
            if let Some(batch_uints) = Self::read_accessor_uints(gltf, bin, batch_accessor_idx) {
                parsed_prim.batch_ids = batch_uints;
            }
        }

        // Pad batch IDs if missing
        while parsed_prim.batch_ids.len() < vertex_count {
            parsed_prim.batch_ids.push(0);
        }

        // Indices
        if let Some(idx_accessor_idx) = prim.indices {
            if let Some(indices) = Self::read_accessor_uints(gltf, bin, idx_accessor_idx) {
                parsed_prim.indices = indices;
            }
        } else {
            // Unindexed geometry
            parsed_prim.indices = (0..vertex_count as u32).collect();
        }

        out_prims.push(parsed_prim);
    }

    fn read_accessor_floats(gltf: &GltfRoot, bin: &[u8], accessor_idx: usize) -> Option<Vec<f32>> {
        let accessor = gltf.accessors.as_ref()?.get(accessor_idx)?;
        let buffer_view = gltf.buffer_views.as_ref()?.get(accessor.buffer_view?)?;

        let num_components = match accessor.accessor_type.as_str() {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            _ => 1,
        };

        let total_floats = accessor.count * num_components;
        let byte_offset = buffer_view.byte_offset.unwrap_or(0) + accessor.byte_offset.unwrap_or(0);
        let byte_stride = buffer_view.byte_stride.unwrap_or(num_components * 4);

        if byte_offset >= bin.len() {
            return None;
        }

        let mut result = Vec::with_capacity(total_floats);

        // Component type: 5126 (FLOAT)
        if accessor.component_type == 5126 {
            if byte_stride == num_components * 4 {
                let byte_len = total_floats * 4;
                if byte_offset + byte_len <= bin.len() {
                    let slice = &bin[byte_offset..byte_offset + byte_len];
                    for chunk in slice.chunks_exact(4) {
                        result.push(f32::from_le_bytes(chunk.try_into().unwrap()));
                    }
                    return Some(result);
                }
            }

            for i in 0..accessor.count {
                let el_offset = byte_offset + i * byte_stride;
                for c in 0..num_components {
                    let comp_offset = el_offset + c * 4;
                    if comp_offset + 4 <= bin.len() {
                        result.push(f32::from_le_bytes(bin[comp_offset..comp_offset + 4].try_into().unwrap()));
                    } else {
                        result.push(0.0);
                    }
                }
            }
            return Some(result);
        }

        None
    }

    fn read_accessor_uints(gltf: &GltfRoot, bin: &[u8], accessor_idx: usize) -> Option<Vec<u32>> {
        let accessor = gltf.accessors.as_ref()?.get(accessor_idx)?;
        let buffer_view = gltf.buffer_views.as_ref()?.get(accessor.buffer_view?)?;

        let byte_offset = buffer_view.byte_offset.unwrap_or(0) + accessor.byte_offset.unwrap_or(0);
        if byte_offset >= bin.len() {
            return None;
        }

        let mut result = Vec::with_capacity(accessor.count);

        match accessor.component_type {
            5121 => { // UNSIGNED_BYTE
                for i in 0..accessor.count {
                    if byte_offset + i < bin.len() {
                        result.push(bin[byte_offset + i] as u32);
                    }
                }
            }
            5123 => { // UNSIGNED_SHORT
                for i in 0..accessor.count {
                    let off = byte_offset + i * 2;
                    if off + 2 <= bin.len() {
                        result.push(u16::from_le_bytes(bin[off..off + 2].try_into().unwrap()) as u32);
                    }
                }
            }
            5125 => { // UNSIGNED_INT
                for i in 0..accessor.count {
                    let off = byte_offset + i * 4;
                    if off + 4 <= bin.len() {
                        result.push(u32::from_le_bytes(bin[off..off + 4].try_into().unwrap()));
                    }
                }
            }
            5126 => { // FLOAT (used for _BATCHID in some exporters)
                for i in 0..accessor.count {
                    let off = byte_offset + i * 4;
                    if off + 4 <= bin.len() {
                        let f = f32::from_le_bytes(bin[off..off + 4].try_into().unwrap());
                        result.push(f as u32);
                    }
                }
            }
            _ => return None,
        }

        Some(result)
    }
}
