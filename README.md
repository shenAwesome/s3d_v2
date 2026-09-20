# S3D (Spatial 3D Engine) V2

A modern, high-performance 3D GIS map engine and spatial analytics workstation written in Rust and WebGPU (`wgpu`). Designed to run seamlessly as both a native desktop application and a high-framerate WebAssembly (WASM) web application.

---

## 1. What This Project Is About

S3D V2 is an open-source, next-generation 3D Geographic Information System (GIS) engine built from the ground up in Rust. It draws architectural inspiration from industry standards like the **Esri ArcGIS Maps SDK**, **Cesium**, and **OpenLayers**, but leverages the performance, memory safety, and modern graphics capabilities of Rust and WebGPU.

### Core Mission
- **Unified 2D/3D & Dual-Projection**: Provide a seamless mathematical bridge between flat planar Cartesian projection (Local ENU) and whole-Earth ellipsoidal geocentric curvature (WGS84 ECEF) with smooth geodetic morphing and automatic elevation-based switching.
- **Modern GPU-Accelerated Pipelines**: Implement tile raster streaming, 3D digital elevation models (DEM), volumetric building extrusion, and atmospheric Rayleigh/Mie scattering via WGSL shaders in `wgpu`.
- **Astronomical Precision**: Real-time solar ephemeris computation (azimuth and altitude angles based on latitude, longitude, and UTC timestamp) casting physically-accurate directional shadows.
- **Zero Hidden Logic (100% Code Transparency)**: Showcase examples follow the OpenLayers philosophy where the runnable code snippet displayed side-by-side matches the running engine state 100%.

---

## 2. Architecture & Crate Boundaries

The repository is strictly partitioned into three modular crates:

```
s3d_v2/
├── crates/
│   ├── core/       # [s3d-core] Headless GIS & 3D Map Engine (Math, GPU, CRS, Shaders)
│   ├── control/    # [s3d-control] Reusable UI Controls, Widgets & Navigation Gizmos
│   └── studio/     # [s3d-studio] Full-featured Desktop GIS Workstation (ArcGIS Pro / QGIS style)
└── assets/         # Sample GeoJSON building footprints, terrain rasters, fonts
```

### `crates/core` (`s3d-core`) — Map Engine ONLY
- **Headless Document Model**:
  - `Map`: Headless GIS document containing spatial references, basemaps, ground surfaces, and operational layer collections.
  - `Basemap`: Base backdrop layers (OpenStreetMap, Esri World Imagery, Esri World Streets, Esri World Topo).
  - `Ground`: 3D planetary elevation surface with support for Mapbox Terrain-RGB, Terrarium, and Esri LERC DEMs.
  - `LayerCollection`: Observable, thread-safe collections of vector, raster, and 3D layers.
  - `ViewingMode`: Viewing policy (`Globe`, `Planar`, or `Auto { threshold_altitude }`).
- **Coordinate Reference Systems (CRS)**:
  - WGS84 (`[lon, lat, elev]` / `GeoCoord`), Web Mercator (EPSG:3857), ECEF (Earth-Centered Earth-Fixed), and ENU (East-North-Up local tangent plane).
  - `ProjectOrigin`: Geographic anchor for double-precision-to-single-precision local Cartesian transformations.
- **Rendering & Shaders**:
  - Custom WGSL shaders for extruded meshes, atmospheric glow/scattering, vector lines, and procedural grids.
  - Dual projection morphing between planar and spherical coordinates directly in the vertex shader.
- **Minimal Viewport**:
  - Headless by design, with an optional minimal `MapWidget` for embedding the GPU canvas into `egui` viewports.

### `crates/control` (`s3d-control`) — Reusable Map Controls
- Modular map widgets: 3D navigation gizmo, compass/north arrow, measurement tools, basemap selector popups, layer tree controllers, and timeline/solar sliders.

### `crates/studio` (`s3d-studio`) — Spatial Studio Application
- Comprehensive professional desktop & web application modeled after ArcGIS Pro and QGIS, integrating `s3d-core` and `s3d-control` with advanced spatial analytics, attribute tables, and project management.

---

## 3. Current Demos & Examples

The interactive showcase (`crates/core/examples/showcase.rs`) demonstrates core engine capabilities with an OpenLayers-style split view: a live 3D map viewport on the left, and the exact, runnable Rust code on the right.

### Demo Catalog

| # | Demo Name | URL Anchor | Key Capabilities Demonstrated |
|---|-----------|------------|--------------------------------|
| 1 | **Simple Map (Quickstart)** | [`#simple_map`](http://localhost:8080/#simple_map) | Building a `Map` document with `Basemap::osm()`, setting project origin `[x, y, z]`, enabling `ViewingMode::Auto` (switching between Globe & Planar at 50 km MSL), and mounting into `MapWidget`. |
| 2 | **Basemap Switcher** | [`#basemap_switcher`](http://localhost:8080/#basemap_switcher) | Dynamic runtime switching between raster basemaps: OpenStreetMap, Esri World Streets, Esri World Topo, and Esri World Imagery (Satellite). |
| 3 | **3D Buildings (GeoJSON Extrusion)** | [`#geojson_buildings`](http://localhost:8080/#geojson_buildings) | Parsing GeoJSON polygon footprints, 2.5D triangulation via `earcutr`, height attribute parsing, and real-time 3D volumetric extrusion into a `FeatureLayer`. |
| 4 | **Solar Calculation & Shadows** | [`#sun_and_shadows`](http://localhost:8080/#sun_and_shadows) | Astronomical solar position calculation (solar azimuth & elevation) for any geographic coordinates and time of day, with real-time shadow projection. |
| 5 | **3D Terrain Elevation (DEM)** | [`#terrain_elevation`](http://localhost:8080/#terrain_elevation) | Adding Mapbox Terrain-RGB digital elevation models to the `Ground` surface, rendering 3D mountain relief (e.g. Mount Fuji), and interactive height exaggeration. |
| 6 | **Camera Navigation & Angles** | [`#camera_navigation`](http://localhost:8080/#camera_navigation) | Esri SceneView-style camera targeting: setting distance, heading (yaw), tilt (pitch), look-at points, and smooth flight transitions. |
| 7 | **Coordinate Picking & Raycast** | [`#coordinate_picking`](http://localhost:8080/#coordinate_picking) | Screen-space cursor raycasting into 3D world space, intersecting building geometry and terrain surface, and extracting WGS84 geographic coordinates. |

---

## 4. Code Transparency Rule

In accordance with our core design philosophy (`AGENTS.md`):
- **Minimal UI Only**: Demos in `crates/core` contain zero bloated mock widgets, fake sidebars, or synthetic controls.
- **100% Matching Code Section**: The code displayed in the right panel is the **exact, runnable Rust code** required to reproduce the running viewport. No hidden camera angles, implicit overrides, or magic settings.

---

## 5. Getting Started & Running

### Prerequisites
- **Rust Toolchain**: 1.80+ (Stable)
  ```bash
  rustup default stable
  ```
- **WebAssembly Target** (for web builds):
  ```bash
  rustup target add wasm32-unknown-unknown
  cargo install trunk
  ```

### Running the Web Showcase (Recommended)
Launch Trunk dev-server with hot-reloading:
```bash
trunk serve
```
Open `http://localhost:8080/` in any modern browser supporting WebGPU or WebGL2.

You can navigate directly to specific demos using URL hash anchors:
- `http://localhost:8080/#simple_map`
- `http://localhost:8080/#basemap_switcher`
- `http://localhost:8080/#geojson_buildings`
- `http://localhost:8080/#sun_and_shadows`
- `http://localhost:8080/#terrain_elevation`
- `http://localhost:8080/#camera_navigation`
- `http://localhost:8080/#coordinate_picking`

### Running the Native Desktop Showcase
Run the interactive showcase as a native desktop application:
```bash
cargo run -p s3d-core --example showcase
```

To launch directly into a specific demo via CLI argument:
```bash
cargo run -p s3d-core --example showcase -- --example=geojson_buildings
```

### Running Automated Tests
Execute the GIS algorithms, CRS transformations, and map document unit tests:
```bash
cargo test -p s3d-core
```

---

## 6. Technology Stack

| Domain | Technology |
|--------|------------|
| **Language** | Rust 2021 Edition |
| **GPU & Shading** | `wgpu` 24.0 (Vulkan, Metal, DirectX 12, WebGPU), WGSL |
| **GUI Framework** | `egui` 0.31 / `eframe` 0.31 |
| **Linear Algebra** | `glam` 0.29 (SIMD-accelerated 3D vectors & matrices) |
| **GIS & Geometry** | `geojson` 0.24, `earcutr` 0.4 (polygon triangulation) |
| **Raster & Elevation** | `image` 0.25, `lerc-rs` 0.5 (Esri LERC compression) |
| **Time & Astronomy** | `chrono` 0.4, `web-time` 1.1 |
| **WebAssembly** | `wasm-bindgen`, `web-sys`, `ehttp` |
