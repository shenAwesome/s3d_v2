use crate::gis::crs::{GeoCoord, ProjectOrigin};
use crate::gis::extrusion::RawMeshData;
use glam::Vec3;
use super::spec::*;

pub struct I3SGeometryDecoder;

impl I3SGeometryDecoder {
    /// Decodes uncompressed I3S binary geometry buffer into RawMeshData with ENU and ECEF positions,
    /// and extracts individual segmented 3D building features based on featureId and faceRange.
    pub fn decode(
        buffer: &[u8],
        geom_info: &I3SGeometryInfo,
        geom_buffer_def: Option<&I3SGeometryBufferDef>,
        obb_center: [f64; 3],
        is_wgs84_deg: bool,
        origin: &ProjectOrigin,
        base_color: [f32; 4],
    ) -> Result<DecodedI3SNode, String> {
        let vertex_count = geom_info.vertex_count as usize;
        if vertex_count == 0 {
            return Err("Empty vertex count".to_string());
        }

        // 0. Transparent gzip decompression: ArcGIS SceneServers often return raw gzip
        let decompressed_buf: Vec<u8>;
        let buffer: &[u8] = if buffer.len() >= 2 && buffer[0] == 0x1f && buffer[1] == 0x8b {
            use std::io::Read;
            let mut decoder = flate2::read::GzDecoder::new(buffer);
            let mut decompressed = Vec::new();
            if let Err(e) = decoder.read_to_end(&mut decompressed) {
                return Err(format!("Failed to decompress I3S gzip buffer: {}", e));
            }
            decompressed_buf = decompressed;
            &decompressed_buf
        } else {
            buffer
        };

        let header_feature_count = if buffer.len() >= 8 {
            buffer.get(4..8)
                .and_then(|s| s.try_into().ok())
                .map(u32::from_le_bytes)
                .unwrap_or(0) as usize
        } else {
            0
        };

        let feature_count = geom_info.feature_count.map(|c| c as usize).unwrap_or(header_feature_count);

        let offset = geom_buffer_def.and_then(|d| d.offset).unwrap_or(8);
        if buffer.len() < offset {
            return Err(format!("Buffer too small: {} bytes vs offset {}", buffer.len(), offset));
        }

        let mut byte_cursor = offset;

        // 1. Positions: vertex_count * 3 * 4 bytes (f32)
        let pos_bytes = vertex_count * 3 * 4;
        if byte_cursor + pos_bytes > buffer.len() {
            return Err("Buffer overrun reading positions".to_string());
        }
        let pos_slice = &buffer[byte_cursor..byte_cursor + pos_bytes];
        byte_cursor += pos_bytes;

        // 2. Normals: vertex_count * 3 * 4 bytes (f32)
        let norm_bytes = vertex_count * 3 * 4;
        let norm_slice = if byte_cursor + norm_bytes <= buffer.len() {
            let s = &buffer[byte_cursor..byte_cursor + norm_bytes];
            byte_cursor += norm_bytes;
            Some(s)
        } else {
            None
        };

        // 3. UV0: vertex_count * 2 * 4 bytes (f32)
        let uv_bytes = vertex_count * 2 * 4;
        let uv_slice = if byte_cursor + uv_bytes <= buffer.len() {
            let s = &buffer[byte_cursor..byte_cursor + uv_bytes];
            byte_cursor += uv_bytes;
            Some(s)
        } else {
            None
        };

        // 4. Color: vertex_count * 4 * 1 byte (u8)
        let col_bytes = vertex_count * 4;
        let col_slice = if byte_cursor + col_bytes <= buffer.len() {
            let s = &buffer[byte_cursor..byte_cursor + col_bytes];
            byte_cursor += col_bytes;
            Some(s)
        } else {
            None
        };

        // 4b. Optional UV Region: vertex_count * 4 * 2 bytes (UInt16 normalized) = vertex_count * 8 bytes
        // Present in textured layers with texture atlases (e.g. Glen Eira).
        let feat_bytes_64 = feature_count * 16;
        let feat_bytes_32 = feature_count * 12;
        let feat_start = if feature_count > 0 && buffer.len() >= feat_bytes_64 && buffer.len() - feat_bytes_64 >= byte_cursor {
            buffer.len() - feat_bytes_64
        } else if feature_count > 0 && buffer.len() >= feat_bytes_32 && buffer.len() - feat_bytes_32 >= byte_cursor {
            buffer.len() - feat_bytes_32
        } else {
            buffer.len()
        };

        let region_bytes = vertex_count * 8;
        let region_slice = if byte_cursor + region_bytes <= feat_start {
            let s = &buffer[byte_cursor..byte_cursor + region_bytes];
            byte_cursor += region_bytes;
            Some(s)
        } else {
            None
        };

        // 5. Feature IDs & Face Ranges for per-building identification
        let mut feature_ids = Vec::with_capacity(feature_count);
        let mut face_ranges = Vec::with_capacity(feature_count);

        if feature_count > 0 && byte_cursor < buffer.len() {
            if byte_cursor < feat_start {
                byte_cursor = feat_start;
            }

            let remaining_bytes = buffer.len() - byte_cursor;
            // Case A: UInt64 featureId (8 bytes each) + UInt32 faceRange (8 bytes pair each) = 16 bytes per feature
            if remaining_bytes >= feature_count * 16 {
                for _ in 0..feature_count {
                    if let Some(slice) = buffer.get(byte_cursor..byte_cursor + 8).and_then(|s| s.try_into().ok()) {
                        feature_ids.push(u64::from_le_bytes(slice));
                    }
                    byte_cursor += 8;
                }
                for _ in 0..feature_count {
                    let sf = buffer.get(byte_cursor..byte_cursor + 4)
                        .and_then(|s| s.try_into().ok())
                        .map(u32::from_le_bytes)
                        .unwrap_or(0);
                    let ef = buffer.get(byte_cursor + 4..byte_cursor + 8)
                        .and_then(|s| s.try_into().ok())
                        .map(u32::from_le_bytes)
                        .unwrap_or(0);
                    byte_cursor += 8;
                    face_ranges.push((sf, ef));
                }
            } else if remaining_bytes >= feature_count * 12 {
                // Case B: UInt32 featureId (4 bytes each) + UInt32 faceRange (8 bytes pair each) = 12 bytes per feature
                for _ in 0..feature_count {
                    if let Some(slice) = buffer.get(byte_cursor..byte_cursor + 4).and_then(|s| s.try_into().ok()) {
                        feature_ids.push(u32::from_le_bytes(slice) as u64);
                    }
                    byte_cursor += 4;
                }
                for _ in 0..feature_count {
                    let sf = buffer.get(byte_cursor..byte_cursor + 4)
                        .and_then(|s| s.try_into().ok())
                        .map(u32::from_le_bytes)
                        .unwrap_or(0);
                    let ef = buffer.get(byte_cursor + 4..byte_cursor + 8)
                        .and_then(|s| s.try_into().ok())
                        .map(u32::from_le_bytes)
                        .unwrap_or(0);
                    byte_cursor += 8;
                    face_ranges.push((sf, ef));
                }
            }
        }

        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(vertex_count);
        let mut uvs = Vec::with_capacity(vertex_count);
        let mut colors = Vec::with_capacity(vertex_count);
        let mut indices = Vec::with_capacity(vertex_count);

        let mut min_enu = Vec3::splat(f32::INFINITY);
        let mut max_enu = Vec3::splat(f32::NEG_INFINITY);
        let mut min_ecef = Vec3::splat(f32::INFINITY);
        let mut max_ecef = Vec3::splat(f32::NEG_INFINITY);

        let c_lon = obb_center[0];
        let c_lat = obb_center[1];
        let c_elev = obb_center[2];

        for i in 0..vertex_count {
            let p_offset = i * 12;
            let px = pos_slice.get(p_offset..p_offset + 4).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0) as f64;
            let py = pos_slice.get(p_offset + 4..p_offset + 8).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0) as f64;
            let pz = pos_slice.get(p_offset + 8..p_offset + 12).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0) as f64;

            // Absolute geographic coordinates
            let (lat, lon, elev) = if is_wgs84_deg {
                (c_lat + py, c_lon + px, c_elev + pz)
            } else {
                // If Web Mercator (EPSG:3857) meters
                let mx = c_lon + px;
                let my = c_lat + py;
                let lon = (mx / 6378137.0).to_degrees();
                let lat = (2.0 * (my / 6378137.0).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
                (lat, lon, c_elev + pz)
            };

            // 1. Planar Local ENU coordinates
            let enu = origin.geo_to_local(&GeoCoord::new(lat, lon, elev));
            min_enu = min_enu.min(enu);
            max_enu = max_enu.max(enu);
            positions.push([enu.x, enu.y, enu.z]);

            // 2. Normal vector (default to Up if not provided)
            if let Some(ns) = norm_slice {
                let n_offset = i * 12;
                let nx = ns.get(n_offset..n_offset + 4).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0);
                let ny = ns.get(n_offset + 4..n_offset + 8).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(1.0);
                let nz = ns.get(n_offset + 8..n_offset + 12).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0);
                normals.push([nx, nz, -ny]); // Orient to engine Y-up
            } else {
                normals.push([0.0, 1.0, 0.0]);
            }

            // 3. UV texture coordinate
            if let Some(us) = uv_slice {
                let u_offset = i * 8;
                let u = us.get(u_offset..u_offset + 4).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0);
                let v = us.get(u_offset + 4..u_offset + 8).and_then(|s| s.try_into().ok()).map(f32::from_le_bytes).unwrap_or(0.0);

                if let Some(rs) = region_slice {
                    let r_offset = i * 8;
                    let u_min = rs.get(r_offset..r_offset + 2).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes).unwrap_or(0) as f32 / 65535.0;
                    let v_min = rs.get(r_offset + 2..r_offset + 4).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes).unwrap_or(0) as f32 / 65535.0;
                    let u_max = rs.get(r_offset + 4..r_offset + 6).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes).unwrap_or(65535) as f32 / 65535.0;
                    let v_max = rs.get(r_offset + 6..r_offset + 8).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes).unwrap_or(65535) as f32 / 65535.0;

                    let u_frac = u.rem_euclid(1.0);
                    // I3S uses OpenGL UV convention (V=0 at bottom); WebGPU uses V=0 at top.
                    // Flip V within [0,1] before remapping into the atlas sub-region.
                    let v_frac = 1.0 - v.rem_euclid(1.0);
                    let final_u = u_frac * (u_max - u_min) + u_min;
                    // Atlas region v_min/v_max are also in OpenGL space (v_min < v_max, bottom-up).
                    // After flipping v_frac, map into the flipped region so the sub-image is right-side up.
                    let final_v = 1.0 - (v_frac * (v_max - v_min) + v_min);
                    uvs.push([final_u, final_v]);
                } else {
                    // No region: simply flip V for OpenGL → WebGPU convention
                    uvs.push([u, 1.0 - v]);
                }
            } else {
                uvs.push([0.0, 0.0]);
            }

            // 4. Color attribute
            if let Some(cs) = col_slice {
                let c_offset = i * 4;
                let r = cs.get(c_offset).copied().unwrap_or(255) as f32 / 255.0;
                let g = cs.get(c_offset + 1).copied().unwrap_or(255) as f32 / 255.0;
                let b = cs.get(c_offset + 2).copied().unwrap_or(255) as f32 / 255.0;
                let a = cs.get(c_offset + 3).copied().unwrap_or(255) as f32 / 255.0;
                colors.push([r, g, b, a]);
            }

            // 5. Globe ECEF coordinates
            let ecef = crate::gis::crs::geodetic_to_ecef(&GeoCoord::new(lat, lon, elev));
            min_ecef = min_ecef.min(ecef);
            max_ecef = max_ecef.max(ecef);

            indices.push(i as u32);
        }

        // Build individual building feature meshes for precise per-building selection & highlight
        let mut features = Vec::new();
        if !feature_ids.is_empty() && feature_ids.len() == face_ranges.len() {
            for i in 0..feature_ids.len() {
                let fid = feature_ids[i];
                let (start_face, end_face) = face_ranges[i];
                let max_v = positions.len() as u64;
                let start_v = ((start_face as u64) * 3).min(max_v) as usize;
                let end_v = ((end_face as u64) * 3 + 3).min(max_v) as usize;

                if start_v < end_v {
                    let feat_positions = positions[start_v..end_v].to_vec();
                    let feat_normals = normals[start_v..end_v].to_vec();
                    let feat_uvs = uvs[start_v..end_v].to_vec();
                    let feat_colors = if colors.len() == positions.len() {
                        colors[start_v..end_v].to_vec()
                    } else {
                        Vec::new()
                    };
                    let feat_indices = (0..(end_v - start_v) as u32).collect();

                    let mut f_min = Vec3::splat(f32::INFINITY);
                    let mut f_max = Vec3::splat(f32::NEG_INFINITY);
                    for p in &feat_positions {
                        let v = Vec3::from_array(*p);
                        f_min = f_min.min(v);
                        f_max = f_max.max(v);
                    }

                    let center_local = (f_min + f_max) * 0.5;
                    let center_geo = origin.local_to_geo(center_local);
                    let height = (f_max.y - f_min.y).max(0.0);

                    features.push(I3SFeatureMesh {
                        feature_id: fid,
                        node_id: geom_info.resource,
                        start_face,
                        end_face,
                        raw_mesh: RawMeshData {
                            positions: feat_positions,
                            normals: feat_normals,
                            uvs: feat_uvs,
                            colors: feat_colors,
                            indices: feat_indices,
                        },
                        aabb_min: f_min,
                        aabb_max: f_max,
                        center_geo,
                        height,
                    });
                }
            }
        }

        // If no sub-features exist in this LOD node, fall back to entire tile mesh as single feature
        if features.is_empty() {
            let center_local = (min_enu + max_enu) * 0.5;
            let center_geo = origin.local_to_geo(center_local);
            let height = (max_enu.y - min_enu.y).max(0.0);
            features.push(I3SFeatureMesh {
                feature_id: geom_info.resource as u64,
                node_id: geom_info.resource,
                start_face: 0,
                end_face: (vertex_count / 3).saturating_sub(1) as u32,
                raw_mesh: RawMeshData {
                    positions: positions.clone(),
                    normals: normals.clone(),
                    uvs: uvs.clone(),
                    colors: colors.clone(),
                    indices: indices.clone(),
                },
                aabb_min: min_enu,
                aabb_max: max_enu,
                center_geo,
                height,
            });
        }

        let obb_center_enu = origin.geo_to_local(&GeoCoord::new(c_lat, c_lon, c_elev));

        Ok(DecodedI3SNode {
            node_id: geom_info.resource,
            raw_mesh: RawMeshData {
                positions,
                normals,
                uvs,
                colors,
                indices,
            },
            features,
            obb_center_enu,
            aabb_enu: (min_enu, max_enu),
            aabb_ecef: (min_ecef, max_ecef),
            base_color,
            image_rgba: None,
        })
    }
}

// ----------------------------------------------------
// I3S Background Streaming & Cache Manager
// ----------------------------------------------------

pub enum I3SDownloadResult {
    Metadata(Box<I3SSceneLayer>),
    NodePage(u32, Box<I3SNodePage>),
    PageFailure(u32, String),
    Geometry(Box<DecodedI3SNode>),
    Failure(Option<u32>, String),
}
