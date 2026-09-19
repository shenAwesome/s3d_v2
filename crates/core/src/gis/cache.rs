#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use crate::gis::basemap::{BasemapProvider, TileCoord};
use crate::gis::terrain::TerrainProvider;

// ----------------------------------------------------
// Persistent Tile Disk Cache Manager
// ----------------------------------------------------

pub struct DiskCacheManager;

/// Memory and concurrency resource budgets per ARCHITECTURE_v2.md §5
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourceBudget {
    /// Maximum GPU VRAM budget in bytes (e.g. 24 MiB desktop, 12 MiB wasm)
    pub gpu_bytes: u64,
    /// Maximum CPU RAM cache budget in bytes
    pub cpu_bytes: u64,
    /// Maximum concurrent in-flight requests
    pub max_inflight: usize,
    /// Number of parallel decode workers
    pub decode_workers: usize,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                // ~12 MB VRAM for 48 standard 256x256 RGBA8 tiles (~256 KB each)
                gpu_bytes: 12 * 1024 * 1024,
                cpu_bytes: 32 * 1024 * 1024,
                max_inflight: 6, // Browser per-host HTTP limit
                decode_workers: 0,
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                // ~24 MB VRAM for 96 standard 256x256 RGBA8 tiles (~256 KB each)
                gpu_bytes: 24 * 1024 * 1024,
                cpu_bytes: 128 * 1024 * 1024,
                max_inflight: 16,
                decode_workers: 4,
            }
        }
    }
}

impl ResourceBudget {
    /// Constructs a custom budget
    pub fn new(gpu_bytes: u64, cpu_bytes: u64, max_inflight: usize, decode_workers: usize) -> Self {
        Self {
            gpu_bytes,
            cpu_bytes,
            max_inflight,
            decode_workers,
        }
    }

    /// Calculates tile capacity for 256x256 RGBA8 textures (~256 KB each)
    pub fn max_gpu_tiles_count(&self) -> usize {
        const BYTES_PER_TILE: u64 = 256 * 256 * 4;
        (self.gpu_bytes / BYTES_PER_TILE) as usize
    }
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub basemap_bytes: u64,
    pub basemap_count: usize,
    pub terrain_bytes: u64,
    pub terrain_count: usize,
    pub i3s_bytes: u64,
    pub i3s_count: usize,
    pub total_bytes: u64,
}

impl CacheStats {
    pub fn format_bytes(bytes: u64) -> String {
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else if bytes >= 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} B", bytes)
        }
    }

    pub fn total_formatted(&self) -> String {
        Self::format_bytes(self.total_bytes)
    }

    pub fn basemap_formatted(&self) -> String {
        Self::format_bytes(self.basemap_bytes)
    }

    pub fn terrain_formatted(&self) -> String {
        Self::format_bytes(self.terrain_bytes)
    }

    pub fn i3s_formatted(&self) -> String {
        Self::format_bytes(self.i3s_bytes)
    }
}

impl DiskCacheManager {
    pub const CACHE_ROOT_DIR: &'static str = ".cache";

    /// Gets the root cache directory
    pub fn get_cache_root() -> PathBuf {
        PathBuf::from(Self::CACHE_ROOT_DIR)
    }

    /// Gets the basemap cache root directory
    pub fn get_basemap_cache_root() -> PathBuf {
        Self::get_cache_root().join("basemap")
    }

    /// Gets the terrain cache root directory
    pub fn get_terrain_cache_root() -> PathBuf {
        Self::get_cache_root().join("terrain")
    }

    /// Cache file path for a basemap tile: .cache/basemap/{provider_id}/{z}/{x}_{y}.bin
    pub fn basemap_tile_path(provider: BasemapProvider, coord: TileCoord) -> PathBuf {
        let prov_str = match provider {
            BasemapProvider::OpenStreetMap => "osm",
            BasemapProvider::EsriStreet => "esri_street",
            BasemapProvider::EsriTopo => "esri_topo",
            BasemapProvider::EsriImagery => "esri_imagery",
            BasemapProvider::None => "none",
        };
        Self::get_basemap_cache_root()
            .join(prov_str)
            .join(coord.z.to_string())
            .join(format!("{}_{}.bin", coord.x, coord.y))
    }

    /// Cache file path for a terrain DEM tile: .cache/terrain/{provider_id}/{z}/{x}_{y}.bin
    pub fn terrain_tile_path(provider: TerrainProvider, coord: TileCoord) -> PathBuf {
        let prov_str = match provider {
            TerrainProvider::EsriTerrain3D => "esri_lerc",
            TerrainProvider::AwsTerrarium => "aws_terrarium",
        };
        Self::get_terrain_cache_root()
            .join(prov_str)
            .join(coord.z.to_string())
            .join(format!("{}_{}.bin", coord.x, coord.y))
    }

    pub const CACHE_READ_ENABLED: bool = true;

    #[cfg(not(target_arch = "wasm32"))]
    /// Read basemap tile from disk cache if present
    pub fn read_basemap_tile(provider: BasemapProvider, coord: TileCoord) -> Option<Vec<u8>> {
        if !Self::CACHE_READ_ENABLED {
            return None;
        }
        let path = Self::basemap_tile_path(provider, coord);
        fs::read(path).ok()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn read_basemap_tile(_provider: BasemapProvider, _coord: TileCoord) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Write basemap tile raw bytes to disk cache
    pub fn write_basemap_tile(provider: BasemapProvider, coord: TileCoord, data: &[u8]) {
        let path = Self::basemap_tile_path(provider, coord);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, data);
    }
    #[cfg(target_arch = "wasm32")]
    pub fn write_basemap_tile(_provider: BasemapProvider, _coord: TileCoord, _data: &[u8]) {}

    #[cfg(not(target_arch = "wasm32"))]
    /// Read terrain DEM tile from disk cache if present
    pub fn read_terrain_tile(provider: TerrainProvider, coord: TileCoord) -> Option<Vec<u8>> {
        if !Self::CACHE_READ_ENABLED {
            return None;
        }
        let path = Self::terrain_tile_path(provider, coord);
        fs::read(path).ok()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn read_terrain_tile(_provider: TerrainProvider, _coord: TileCoord) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Write terrain DEM tile raw bytes to disk cache
    pub fn write_terrain_tile(provider: TerrainProvider, coord: TileCoord, data: &[u8]) {
        let path = Self::terrain_tile_path(provider, coord);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, data);
    }
    #[cfg(target_arch = "wasm32")]
    pub fn write_terrain_tile(_provider: TerrainProvider, _coord: TileCoord, _data: &[u8]) {}

    /// Gets the ArcGIS cache root directory
    pub fn get_arcgis_cache_root() -> PathBuf {
        Self::get_cache_root().join("arcgis")
    }

    /// Cache file path for an ArcGIS query cell/tile: .cache/arcgis/{service_name}/{layer_id}/{cell_key}.json
    pub fn arcgis_cache_path(service_slug: &str, layer_id: u32, cell_key: &str) -> PathBuf {
        Self::get_arcgis_cache_root()
            .join(service_slug)
            .join(layer_id.to_string())
            .join(format!("{}.json", cell_key))
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Read ArcGIS query response from disk cache if present
    pub fn read_arcgis_data(service_slug: &str, layer_id: u32, cell_key: &str) -> Option<Vec<u8>> {
        if !Self::CACHE_READ_ENABLED {
            return None;
        }
        let path = Self::arcgis_cache_path(service_slug, layer_id, cell_key);
        fs::read(path).ok()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn read_arcgis_data(_service_slug: &str, _layer_id: u32, _cell_key: &str) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Write ArcGIS query response to disk cache
    pub fn write_arcgis_data(service_slug: &str, layer_id: u32, cell_key: &str, data: &[u8]) {
        let path = Self::arcgis_cache_path(service_slug, layer_id, cell_key);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, data);
    }
    #[cfg(target_arch = "wasm32")]
    pub fn write_arcgis_data(_service_slug: &str, _layer_id: u32, _cell_key: &str, _data: &[u8]) {}

    /// Gets the I3S 3D scene layer cache root directory
    pub fn get_i3s_cache_root() -> PathBuf {
        Self::get_cache_root().join("i3s")
    }

    /// Converts a service URL or name into a filesystem-safe directory slug
    pub fn service_to_slug(url: &str) -> String {
        let clean = url.trim_end_matches('/');
        let name = clean.split('/').last().unwrap_or("layer");
        let slug: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        if slug.is_empty() {
            "default".to_string()
        } else {
            slug
        }
    }

    /// Cache file path for an I3S geometry buffer: .cache/i3s/{service_slug}/geometries/{resource_id}.bin
    pub fn i3s_geom_path(service_slug: &str, resource_id: u32) -> PathBuf {
        Self::get_i3s_cache_root()
            .join(service_slug)
            .join("geometries")
            .join(format!("{}.bin", resource_id))
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Read I3S geometry raw bytes from disk cache if present
    pub fn read_i3s_geometry(service_slug: &str, resource_id: u32) -> Option<Vec<u8>> {
        if !Self::CACHE_READ_ENABLED {
            return None;
        }
        let path = Self::i3s_geom_path(service_slug, resource_id);
        fs::read(path).ok()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn read_i3s_geometry(_service_slug: &str, _resource_id: u32) -> Option<Vec<u8>> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Write I3S geometry raw bytes to disk cache
    pub fn write_i3s_geometry(service_slug: &str, resource_id: u32, data: &[u8]) {
        let path = Self::i3s_geom_path(service_slug, resource_id);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, data);
    }
    #[cfg(target_arch = "wasm32")]
    pub fn write_i3s_geometry(_service_slug: &str, _resource_id: u32, _data: &[u8]) {}

    /// Cache file path for an I3S nodepage JSON: .cache/i3s/{service_slug}/pages/{page_id}.json
    pub fn i3s_page_path(service_slug: &str, page_id: u32) -> PathBuf {
        Self::get_i3s_cache_root()
            .join(service_slug)
            .join("pages")
            .join(format!("{}.json", page_id))
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Read I3S nodepage JSON string from disk cache if present
    pub fn read_i3s_nodepage(service_slug: &str, page_id: u32) -> Option<String> {
        if !Self::CACHE_READ_ENABLED {
            return None;
        }
        let path = Self::i3s_page_path(service_slug, page_id);
        fs::read_to_string(path).ok()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn read_i3s_nodepage(_service_slug: &str, _page_id: u32) -> Option<String> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Write I3S nodepage JSON string to disk cache
    pub fn write_i3s_nodepage(service_slug: &str, page_id: u32, data: &str) {
        let path = Self::i3s_page_path(service_slug, page_id);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, data);
    }
    #[cfg(target_arch = "wasm32")]
    pub fn write_i3s_nodepage(_service_slug: &str, _page_id: u32, _data: &str) {}

    #[cfg(not(target_arch = "wasm32"))]
    /// Recursively counts files and total bytes in a directory
    fn scan_dir(dir: &Path) -> (u64, usize) {
        let mut bytes = 0u64;
        let mut count = 0usize;

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let (sub_bytes, sub_count) = Self::scan_dir(&entry.path());
                        bytes += sub_bytes;
                        count += sub_count;
                    } else if file_type.is_file() {
                        if let Ok(meta) = entry.metadata() {
                            bytes += meta.len();
                            count += 1;
                        }
                    }
                }
            }
        }

        (bytes, count)
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Computes full disk cache metrics
    pub fn compute_stats() -> CacheStats {
        let (bm_bytes, bm_count) = Self::scan_dir(&Self::get_basemap_cache_root());
        let (ter_bytes, ter_count) = Self::scan_dir(&Self::get_terrain_cache_root());
        let (i3s_bytes, i3s_count) = Self::scan_dir(&Self::get_i3s_cache_root());

        CacheStats {
            basemap_bytes: bm_bytes,
            basemap_count: bm_count,
            terrain_bytes: ter_bytes,
            terrain_count: ter_count,
            i3s_bytes,
            i3s_count,
            total_bytes: bm_bytes + ter_bytes + i3s_bytes,
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn compute_stats() -> CacheStats {
        CacheStats::default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Clears basemap imagery disk cache
    pub fn clear_basemap_cache() -> Result<usize, String> {
        let root = Self::get_basemap_cache_root();
        let (_, count) = Self::scan_dir(&root);
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        }
        Ok(count)
    }
    #[cfg(target_arch = "wasm32")]
    pub fn clear_basemap_cache() -> Result<usize, String> {
        Ok(0)
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Clears terrain elevation DEM disk cache
    pub fn clear_terrain_cache() -> Result<usize, String> {
        let root = Self::get_terrain_cache_root();
        let (_, count) = Self::scan_dir(&root);
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        }
        Ok(count)
    }
    #[cfg(target_arch = "wasm32")]
    pub fn clear_terrain_cache() -> Result<usize, String> {
        Ok(0)
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Clears I3S 3D buildings disk cache
    pub fn clear_i3s_cache() -> Result<usize, String> {
        let root = Self::get_i3s_cache_root();
        let (_, count) = Self::scan_dir(&root);
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        }
        Ok(count)
    }
    #[cfg(target_arch = "wasm32")]
    pub fn clear_i3s_cache() -> Result<usize, String> {
        Ok(0)
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Clears all tile caches on disk
    pub fn clear_all() -> Result<usize, String> {
        let root = Self::get_cache_root();
        let (_, count) = Self::scan_dir(&root);
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        }
        Ok(count)
    }
    #[cfg(target_arch = "wasm32")]
    pub fn clear_all() -> Result<usize, String> {
        Ok(0)
    }
}

