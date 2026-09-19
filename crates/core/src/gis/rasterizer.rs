//! Tile-Space 2D Vector Rasterizer for MapLibre-style 3D Terrain Draping
//!
//! Projects 2D GIS vector geometries (polygons, fills, strokes) into Web Mercator tile pixel space
//! and renders them using tiny-skia onto 512x512 RGBA textures draped directly over 3D DEM terrain meshes.

use crate::gis::basemap::TileCoord;
use crate::gis::layer::GisFeature;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub const TILE_RASTER_SIZE: u32 = 512;

/// Converts (latitude, longitude) in degrees into (u, v) pixel coordinates within a Web Mercator tile.
pub fn geo_to_tile_pixel(lat: f64, lon: f64, coord: &TileCoord, tile_width: f32, tile_height: f32) -> (f32, f32) {
    let n = 2.0_f64.powi(coord.z as i32);
    let lat_rad = lat.to_radians().clamp(-1.4844, 1.4844); // Clamp to Mercator limits ~85 deg

    let x_world = (lon + 180.0) / 360.0 * n;
    let y_world = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)) / 2.0 * n;

    let u = ((x_world - coord.x as f64) * tile_width as f64) as f32;
    let v = ((y_world - coord.y as f64) * tile_height as f64) as f32;

    (u, v)
}

/// Converts tile-relative (u, v) pixel coordinates back to (latitude, longitude) in degrees.
pub fn tile_pixel_to_geo(u: f32, v: f32, coord: &TileCoord, tile_width: f32, tile_height: f32) -> (f64, f64) {
    let n = 2.0_f64.powi(coord.z as i32);
    let x_world = coord.x as f64 + (u as f64 / tile_width as f64);
    let y_world = coord.y as f64 + (v as f64 / tile_height as f64);

    let lon_deg = x_world / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * y_world / n)).sinh().atan();
    let lat_deg = lat_rad.to_degrees();

    (lat_deg, lon_deg)
}

/// Tests if a geographic point (lat, lon) lies within a 2D polygon ring.
pub fn point_in_polygon_geo(lat: f64, lon: f64, ring: &[[f64; 2]]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;

    for i in 0..n {
        let lat_i = ring[i][0];
        let lon_i = ring[i][1];
        let lat_j = ring[j][0];
        let lon_j = ring[j][1];

        let intersect = ((lon_i > lon) != (lon_j > lon))
            && (lat < (lat_j - lat_i) * (lon - lon_i) / (lon_j - lon_i + 1e-12) + lat_i);
        if intersect {
            inside = !inside;
        }
        j = i;
    }

    inside
}

pub struct TileVectorRasterizer;

impl TileVectorRasterizer {
    /// Rasterizes vector features that intersect a given tile coordinate onto an RGBA pixmap.
    /// Optionally composites on top of an existing basemap satellite/street tile image.
    pub fn rasterize_tile_overlay(
        coord: &TileCoord,
        features: &[&GisFeature],
        selected_feature_id: Option<&str>,
        base_image: Option<&image::RgbaImage>,
        width: u32,
        height: u32,
    ) -> Option<image::RgbaImage> {
        let mut pixmap = if let Some(base) = base_image {
            let mut p = Pixmap::new(width, height)?;
            let data = p.data_mut();
            if base.width() == width && base.height() == height {
                data.copy_from_slice(base.as_raw());
            } else {
                for y in 0..height {
                    for x in 0..width {
                        let src_x = (x as f32 / width as f32 * base.width() as f32) as u32;
                        let src_y = (y as f32 / height as f32 * base.height() as f32) as u32;
                        let pixel = base.get_pixel(src_x.min(base.width() - 1), src_y.min(base.height() - 1));
                        let idx = ((y * width + x) * 4) as usize;
                        data[idx] = pixel[0];
                        data[idx + 1] = pixel[1];
                        data[idx + 2] = pixel[2];
                        data[idx + 3] = pixel[3];
                    }
                }
            }
            p
        } else {
            Pixmap::new(width, height)?
        };

        let (min_lat, max_lat, min_lon, max_lon) = coord.geo_bounds();
        let pad_lat = (max_lat - min_lat) * 0.15;
        let pad_lon = (max_lon - min_lon) * 0.15;
        let query_min_lat = min_lat - pad_lat;
        let query_max_lat = max_lat + pad_lat;
        let query_min_lon = min_lon - pad_lon;
        let query_max_lon = max_lon + pad_lon;

        let w_f32 = width as f32;
        let h_f32 = height as f32;

        for feat in features {
            let is_selected = selected_feature_id.map(|id| id == feat.id).unwrap_or(false);

            // Determine fill and stroke colors
            let fill_rgba = if is_selected {
                [0.0, 0.85, 1.0, 0.65]
            } else if feat.shadow_color[3] > 0.01 {
                feat.shadow_color
            } else {
                [0.30, 0.55, 0.85, 0.50]
            };

            let stroke_rgba = if is_selected {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [
                    (fill_rgba[0] * 0.25).clamp(0.05, 0.85),
                    (fill_rgba[1] * 0.25).clamp(0.05, 0.85),
                    (fill_rgba[2] * 0.25).clamp(0.05, 0.85),
                    0.95,
                ]
            };

            let fill_color = Color::from_rgba(
                fill_rgba[0].clamp(0.0, 1.0),
                fill_rgba[1].clamp(0.0, 1.0),
                fill_rgba[2].clamp(0.0, 1.0),
                fill_rgba[3].clamp(0.0, 1.0),
            )?;

            let stroke_color = Color::from_rgba(
                stroke_rgba[0].clamp(0.0, 1.0),
                stroke_rgba[1].clamp(0.0, 1.0),
                stroke_rgba[2].clamp(0.0, 1.0),
                stroke_rgba[3].clamp(0.0, 1.0),
            )?;

            let mut fill_paint = Paint::default();
            fill_paint.set_color(fill_color);
            fill_paint.anti_alias = true;

            let mut stroke_paint = Paint::default();
            stroke_paint.set_color(stroke_color);
            stroke_paint.anti_alias = true;

            let mut stroke = Stroke::default();
            stroke.width = if is_selected { 3.0 } else { 1.5 };

            for geo_poly in &feat.geo_polygons {
                // Check AABB intersection with tile
                let mut p_min_lat = f64::INFINITY;
                let mut p_max_lat = f64::NEG_INFINITY;
                let mut p_min_lon = f64::INFINITY;
                let mut p_max_lon = f64::NEG_INFINITY;

                for ring in geo_poly {
                    for &[lat, lon] in ring {
                        p_min_lat = p_min_lat.min(lat);
                        p_max_lat = p_max_lat.max(lat);
                        p_min_lon = p_min_lon.min(lon);
                        p_max_lon = p_max_lon.max(lon);
                    }
                }

                if p_max_lat < query_min_lat
                    || p_min_lat > query_max_lat
                    || p_max_lon < query_min_lon
                    || p_min_lon > query_max_lon
                {
                    continue;
                }

                // Build tiny-skia 2D path
                let mut pb = PathBuilder::new();
                for ring in geo_poly {
                    if ring.len() < 3 {
                        continue;
                    }
                    for (i, &[lat, lon]) in ring.iter().enumerate() {
                        let (u, v) = geo_to_tile_pixel(lat, lon, coord, w_f32, h_f32);
                        if i == 0 {
                            pb.move_to(u, v);
                        } else {
                            pb.line_to(u, v);
                        }
                    }
                    pb.close();
                }

                if let Some(path) = pb.finish() {
                    // 1. Draw Antialiased Polygon Fill
                    pixmap.fill_path(&path, &fill_paint, FillRule::Winding, Transform::identity(), None);
                    // 2. Draw Crisp Boundary Stroke
                    pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
                }
            }
        }

        let rgba_raw = pixmap.take();
        image::RgbaImage::from_raw(width, height, rgba_raw)
    }
}

