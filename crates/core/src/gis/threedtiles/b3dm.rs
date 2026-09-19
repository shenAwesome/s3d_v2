use super::gltf::{GltfParsedModel, GltfParser};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ----------------------------------------------------
// B3DM Data Structures
// ----------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureTableJson {
    #[serde(rename = "BATCH_LENGTH")]
    pub batch_length: Option<u32>,
    #[serde(rename = "RTC_CENTER")]
    pub rtc_center: Option<[f64; 3]>,
}

#[derive(Debug, Clone)]
pub struct ParsedB3dmModel {
    pub model: GltfParsedModel,
    pub batch_length: u32,
    pub rtc_center: Option<[f64; 3]>,
    pub batch_table_json: HashMap<String, Vec<serde_json::Value>>,
}

pub struct B3dmParser;

impl B3dmParser {
    pub const B3DM_MAGIC: u32 = 0x6d643362; // "b3dm"
    pub const GLB_MAGIC: u32 = 0x46546C67;  // "glTF"
    pub const CMPT_MAGIC: u32 = 0x74706d63; // "cmpt"

    pub fn parse(bytes: &[u8]) -> Result<ParsedB3dmModel, String> {
        if bytes.len() < 4 {
            return Err("3D tile payload too short (< 4 bytes)".to_string());
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());

        // 1. Direct GLB
        if magic == Self::GLB_MAGIC {
            let model = GltfParser::parse_glb(bytes)?;
            return Ok(ParsedB3dmModel {
                model,
                batch_length: 0,
                rtc_center: None,
                batch_table_json: HashMap::new(),
            });
        }

        // 2. Composite Tile ("cmpt")
        if magic == Self::CMPT_MAGIC {
            if bytes.len() < 16 {
                return Err("Invalid CMPT tile header".to_string());
            }
            let _version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
            let byte_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
            let tiles_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;

            let mut inner_offset = 16;
            let mut combined_primitives = Vec::new();
            let mut total_batch_length = 0;
            let total_len = byte_length.min(bytes.len());

            for _ in 0..tiles_length {
                if inner_offset + 12 > total_len {
                    break;
                }
                let inner_byte_length = u32::from_le_bytes(
                    bytes[inner_offset + 8..inner_offset + 12].try_into().unwrap(),
                ) as usize;
                if inner_byte_length == 0 || inner_offset + inner_byte_length > total_len {
                    break;
                }

                if let Ok(parsed_inner) = Self::parse(&bytes[inner_offset..inner_offset + inner_byte_length]) {
                    combined_primitives.extend(parsed_inner.model.primitives);
                    total_batch_length += parsed_inner.batch_length;
                }

                inner_offset += inner_byte_length;
            }

            return Ok(ParsedB3dmModel {
                model: GltfParsedModel {
                    primitives: combined_primitives,
                    image_rgba: None,
                },
                batch_length: total_batch_length,
                rtc_center: None,
                batch_table_json: HashMap::new(),
            });
        }

        // 3. Batched 3D Model ("b3dm")
        if magic != Self::B3DM_MAGIC {
            // Fallback: try parsing directly as GLB / glTF
            if let Ok(model) = GltfParser::parse_glb(bytes) {
                return Ok(ParsedB3dmModel {
                    model,
                    batch_length: 0,
                    rtc_center: None,
                    batch_table_json: HashMap::new(),
                });
            }
            return Err(format!("Unsupported 3D Tile magic: 0x{:08X}", magic));
        }

        if bytes.len() < 28 {
            return Err("B3DM payload too short (< 28 bytes header)".to_string());
        }

        let _version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let byte_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let ft_json_byte_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let ft_bin_byte_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        let bt_json_byte_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let bt_bin_byte_len = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;

        let mut offset = 28;
        let total_len = byte_length.min(bytes.len());

        // Feature Table JSON
        let mut feature_table = FeatureTableJson::default();
        if ft_json_byte_len > 0 && offset + ft_json_byte_len <= total_len {
            if let Ok(json_str) = std::str::from_utf8(&bytes[offset..offset + ft_json_byte_len]) {
                if let Ok(ft) = serde_json::from_str::<FeatureTableJson>(json_str) {
                    feature_table = ft;
                }
            }
            offset += ft_json_byte_len;
        }

        // Feature Table Binary
        offset += ft_bin_byte_len;

        // Batch Table JSON
        let mut batch_table_json = HashMap::new();
        if bt_json_byte_len > 0 && offset + bt_json_byte_len <= total_len {
            if let Ok(json_str) = std::str::from_utf8(&bytes[offset..offset + bt_json_byte_len]) {
                if let Ok(bt) = serde_json::from_str::<HashMap<String, Vec<serde_json::Value>>>(json_str) {
                    batch_table_json = bt;
                }
            }
            offset += bt_json_byte_len;
        }

        // Batch Table Binary
        offset += bt_bin_byte_len;

        // Embedded glTF / GLB payload
        if offset >= total_len {
            return Err("B3DM missing embedded glTF payload".to_string());
        }

        let glb_bytes = &bytes[offset..total_len];
        let model = GltfParser::parse_glb(glb_bytes)?;

        Ok(ParsedB3dmModel {
            model,
            batch_length: feature_table.batch_length.unwrap_or(0),
            rtc_center: feature_table.rtc_center,
            batch_table_json,
        })
    }
}

