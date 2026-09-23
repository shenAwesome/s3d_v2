# S3D V2 — Engineering Improvement Plan & Architecture Roadmap

This document outlines the systematic code quality review, architectural decoupling, performance optimizations, and feature roadmap for the **S3D V2** 3D GIS platform.

---

## 1. Project Health Status

| Component | Status | Findings |
| :--- | :---: | :--- |
| **`crates/core` (`s3d-core`)** | 🟢 Active | High-precision geodesy (WGS84, ECEF, ENU, Web Mercator), terrain DEM streaming, 3D tiles, building extrusion, solar calculations, and WGPU 24 render pipeline. |
| **`crates/control` (`s3d-control`)** | 🔴 Stub | 7-line placeholder (`pub struct NavigationControl;`). Reusable overlay widgets and controls are unimplemented. |
| **`crates/studio` (`s3d-studio`)** | 🔴 Stub | 6-line scaffold (`println!("S3D Studio scaffold");`). Needs full application assembly. |
| **Code Quality & Lints** | 🟡 Warnings | 26 unique Clippy warnings (77 total instances including test passes). 1 pattern-match logic bug in I3S LoD metrics. |
| **Examples & Showcase** | 🟢 Aligned | OpenLayers-style minimal-UI showcase with copyable Rust snippets; needs minor code snippet synchronization for remote GeoJSON loading. |
| **Automated Testing** | 🟡 Basic | 9 unit/integration tests passing; key subsystems (tile indexing, terrain decoding, 3D Tiles B3DM, measurement math) lack test coverage. |

---

## 2. Architecture & Crate Boundaries

```
┌────────────────────────────────────────────────────────┐
│                   crates/studio                        │
│   ArcGIS Pro / QGIS-like Desktop & Web GIS App        │
└──────────────────────────┬─────────────────────────────┘
                           │ uses
┌──────────────────────────▼─────────────────────────────┐
│                   crates/control                       │
│   Reusable Egui Widgets: NavigationGizmo, LayerTree,   │
│   BasemapPicker, SolarSlider, MeasurementOverlay       │
└──────────────────────────┬─────────────────────────────┘
                           │ operates via MapCommand / MapView
┌──────────────────────────▼─────────────────────────────┐
│                    crates/core                         │
│   Headless Map Engine: CRS, Geodesy, WGPU Renderer,    │
│   Terrain DEM, Vector Extrusion, Solar Analysis        │
└────────────────────────────────────────────────────────┘
```

### 2.1 Complete `s3d-control`
In accordance with `AGENTS.md`, `crates/core` must remain headless and minimal, while `crates/control` houses modular UI controls and interactive tools.

**Planned Components for `s3d-control`:**
1. **`NavigationGizmo`**:
   - Interactive compass rose displaying current azimuth / yaw.
   - Clickable cardinal directions (N/S/E/W) with North alignment button.
   - Pitch / tilt indicator and slider (0°–90°).
   - Zoom in (`+`), zoom out (`-`), and reset view buttons.
   - Emits `MapCommand::Camera(...)`.
2. **`BasemapPicker`**:
   - Reusable gallery / dropdown for selecting active basemap raster provider (OpenStreetMap, Esri Imagery, Esri Streets, Esri Topographic).
   - Cache flush and attribution display.
   - Emits `MapCommand::Basemap(...)`.
3. **`LayerTree`**:
   - Reusable layer list widget with checkboxes for visibility toggling.
   - Opacity slider and feature count readouts.
   - Delete layer and reorder layers controls.
   - Emits `MapCommand::Layer(...)`.
4. **`SolarTimeSlider`**:
   - 24-hour diurnal scrubber with live sun elevation / azimuth readout.
   - Solstice presets (Summer Solstice, Winter Solstice, Equinox).
   - Daylight toggle and ambient/sun intensity sliders.
   - Emits `MapCommand::Clock(...)` and `MapCommand::Environment(...)`.
5. **`MeasurementToolbar`**:
   - Interactive tool selector (Distance, Height, Area).
   - HUD overlay displaying 3D distance, horizontal distance, vertical delta, and geodesic length.
   - VCAT-grade Shoelace polygon area calculation (±0.01 m²).

### 2.2 Implement `s3d-studio` Application
Transform `crates/studio/src/main.rs` into a full desktop GIS environment:
- Docking panel layout powered by `egui_dock` or native `egui::SidePanel` / `CentralPanel`.
- Left panel: Layer Manager (`LayerTree`) and Spatial Catalog.
- Right panel: Inspector / Attribute table for selected features and Solar Analysis panel.
- Top bar: Ribbon toolbar (File, View, Analysis, Basemaps, Bookmarks).
- Viewport: Fullscreen `MapWidget` with floating `NavigationGizmo` and `MeasurementToolbar`.
- Bottom bar: Status bar displaying cursor coordinates (Lat/Lon/Elev), active camera distance, and streaming network activity.

---

## 3. Decoupling & Renderer Refactoring

### 3.1 Unused & Leaky Parameters in `RenderEngine::render`
In `crates/core/src/renderer/render_engine.rs`:
```rust
// Current bloated signature (11 arguments):
pub fn render(
    &mut self,
    scene: &Scene,
    camera: &Camera,
    solar_pos: &SolarPosition,
    measurement: &MeasurementEngine,                // UNUSED: Completely ignored
    toolbox: &crate::spatial::toolbox::ToolboxEngine, // LEAK: High-level analytical tool leak
    basemap_visible: bool,
    basemap_zoom: u32,
    sun_intensity: f32,
    ambient_intensity: f32,
    sunlight_enabled: bool,
)
```

**Refactoring Plan:**
1. **Remove `measurement: &MeasurementEngine`**: It is never accessed in `render_with_encoder`.
2. **Introduce `RenderParams` / `FrameParams`**:
   ```rust
   pub struct RenderParams<'a> {
       pub scene: &'a Scene,
       pub camera: &'a Camera,
       pub solar_pos: &'a SolarPosition,
       pub basemap_visible: bool,
       pub basemap_zoom: u32,
       pub sun_intensity: f32,
       pub ambient_intensity: f32,
       pub sunlight_enabled: bool,
       pub overlay_lines: &'a [OverlayLine],
   }
   ```
3. **Decouple Line-of-Sight (LOS) Sightlines**:
   - Instead of passing `&ToolboxEngine` directly to the renderer, pass a slice of generic `OverlayLine` items (`obs_pt`, `target_pt`, `color`, `width_px`, `xray`).
   - Removes dummy allocations in `MapWidget::show_ui` where `dummy_measurement` and `dummy_toolbox` are created on every frame.

---

## 4. Performance & GPU Resource Management

### 4.1 Eliminate Per-Frame Dynamic GPU Allocations
**Problem:** In `render_engine.rs` (lines 1756 & 1770), `GpuMesh::create_screen_line(&self.device, ...)` allocates new GPU vertex and index buffers (`device.create_buffer_init`) on **every frame** at 60+ FPS when sightlines are visible.
**Solution:**
- Retain a dedicated `sightline_mesh: Option<GpuMesh>` on `RenderEngine`.
- Only recreate or update GPU buffers when sightline points actually mutate (dirty flag), avoiding per-frame driver allocation pressure.

### 4.2 Fix Channel Message Enum Sizing (3D Tiles)
**Problem:** In `crates/core/src/gis/threedtiles/manager.rs`:
- `TileRequest::FetchContent` is ~180 bytes due to `DMat4` and multiple strings, while `FetchTileset` is only 24 bytes.
- `TileResponse::ContentLoaded` carries `DecodedThreeDTileMesh` with large vectors.
- Every channel message between the async worker thread and main thread copies the maximum variant size.
**Solution:**
- Box large variant payloads (`Box<DecodedThreeDTileMesh>`) to reduce stack copying overhead and memory footprint.

### 4.3 Optimize 3D Tiles Active Pruning
**Problem:** `prune_unneeded_threedtiles` currently clones all active tile strings before checking if they reside in GPU memory:
```rust
self.active_threedtiles_to_draw = active_tiles
    .iter()
    .cloned() // Clones all IDs eagerly
    .filter(|id| self.threedtiles_gpu_tiles.contains_key(id))
    .collect();
```
**Solution:**
```rust
self.active_threedtiles_to_draw = active_tiles
    .iter()
    .filter(|id| self.threedtiles_gpu_tiles.contains_key(*id))
    .cloned() // Only clone matching IDs
    .collect();
```

---

## 5. Correctness & Showcase Rule Compliance

### 5.1 Fix I3S LoD Metric Pattern Match Bug
In `crates/core/src/gis/i3s/manager.rs` (line 538):
```rust
// INCORRECT (wildcard with guard shadows subsequent branches):
match metric_type.as_str() {
    "maxScreenThresholdSQ" | _ if metric_type.ends_with("SQ") => { ... }
    "maxScreenThreshold" => { ... }
    "screenSpaceError" => { ... }
    _ => { ... }
}

// CORRECT:
match metric_type.as_str() {
    s if s.ends_with("SQ") => { ... }
    "maxScreenThreshold" => { ... }
    "screenSpaceError" => { ... }
    _ => { ... }
}
```
Also simplify the duplicate condition in lines 533-535 from `if dist <= radius { true } else if lod_threshold <= 0.0 { true }` to `if dist <= radius || lod_threshold <= 0.0 { true }`.

### 5.2 Showcase Remote GeoJSON Snippet Synchronization
In `crates/core/examples/showcase.rs`:
- An asynchronous "⬇ Load Remote Data" button was added for the `GeoJsonBuildings` example.
- To strictly adhere to the `AGENTS.md` rule (*"The code displayed must match the running demo 100%: it must be the exact, runnable Rust code required to initialize and use that specific s3d-core feature"*):
  - Update `CoreExample::code_snippet()` for `GeoJsonBuildings` so that it includes the exact snippet showing how remote GeoJSON is fetched via `http::fetch_text` and loaded into a `Layer`.

---

## 6. Code Cleanliness & Clippy Resolution

Systematic plan to resolve all 26 unique Clippy warnings:

| Warning Type | Locations | Action |
| :--- | :--- | :--- |
| `derivable_impls` | `layer.rs` (`ElevationMode`, `LayerSource`), `symbol.rs` (`IconResource`), `terrain.rs` (`TerrainProvider`), `sketch.rs` (`PointMarkerStyle`, `SketchToolMode`), `toolbox.rs` (`ToolboxToolMode`) | Replace manual `impl Default` with `#[derive(Default)] #[default]`. |
| `missing_default` | `terrain.rs` (`TerrainManager`), `manager.rs` (`Tiles3DManager`) | Add `impl Default` calling `Self::new(...)`. |
| `should_implement_trait` | `sketch.rs` (`PointMarkerStyle::from_str`) | Implement `std::str::FromStr` instead of inherent `from_str`. |
| `chunks_exact_to_as_chunks` | `shadow_analysis.rs` (lines 117, 155), `threedtiles/gltf.rs` (lines 400, 417, 437, 502) | Use `as_chunks::<3>().0` for zero-cost compile-time triangular indexing. |
| `manual_map` | `shadow_analysis.rs` (line 306) | Use `top_feature.map(...)` instead of manual `if let Some ...`. |
| `module_inception` | `scene/mod.rs` | Suppress `#[allow(clippy::module_inception)]` or re-export cleanly. |
| `collapsible_if` | `render_engine.rs` (lines 743, 774), `showcase.rs` (line 709) | Combine nested `if` statements with `&&`. |
| `while_let_on_iterator` | `showcase.rs` (line 266) | Replace `while let Some(arg) = args.next()` with `for arg in args`. |
| `unnecessary_map_or` | `showcase.rs` (line 658) | Replace `.map_or(false, |t| current_time - t < 2.0)` with `.is_some_and(|t| current_time - t < 2.0)`. |
| `needless_range_loop` | `toolbox.rs` (line 1125), `terrain.rs` (lines 134, 141, 195, 202, 577, 580) | Iterate directly using `.iter_mut().enumerate()`. |
| `clone_on_copy` | `i3s/manager.rs` (line 156) | Dereference `*o` instead of calling `.clone()` on `ProjectOrigin`. |
| `iterate_on_values` | `i3s/manager.rs` (line 771) | Replace `for (_, cached_parent) in &self.node_cache` with `for cached_parent in self.node_cache.values()`. |
| `redundant_closure` | `threedtiles/gltf.rs` (lines 346, 348) | Pass `glam::Vec3::from_array` directly into `.map()`. |

---

## 7. Test Suite Expansion Plan

Currently, 9 tests exist across 5 test files. Expand tests across high-value GIS algorithms:

### 7.1 New Unit Test Suites
1. **`spatial_test.rs`** (`crates/core/tests/spatial_test.rs`):
   - Shoelace 2D polygon area calculation across convex, concave, and clockwise/counter-clockwise rings.
   - Perimeter length calculation.
   - 3D distance decomposition (3D Euclidean, horizontal XZ, vertical delta).
   - Geodesic distance verification against Australian survey benchmarks.
2. **`basemap_tile_test.rs`** (`crates/core/tests/basemap_tile_test.rs`):
   - WGS84 coordinates to slippy map tile `(z, x, y)` conversions.
   - Tile bounding box calculation in Web Mercator and WGS84.
   - QuadKey string encoding and decoding.
   - Dynamic zoom LOD calculation and hysteresis stability.
3. **`terrain_mesh_test.rs`** (`crates/core/tests/terrain_mesh_test.rs`):
   - Elevation grid normalization and height exaggeration scaling.
   - Vertex normal calculation over steep terrain slopes.
   - Terrarium RGB decoding formula: `elev = (R * 256 + G + B / 256) - 32768`.
4. **`command_event_test.rs`** (`crates/core/tests/command_event_test.rs`):
   - Dispatching `MapCommand::Basemap`, `MapCommand::Layer`, `MapCommand::Environment`.
   - Verifying state mutations and deterministic `MapEvent` generation.
5. **`threedtiles_b3dm_test.rs`** (`crates/core/tests/threedtiles_b3dm_test.rs`):
   - B3DM binary header parsing (magic check, version, feature table, batch table).
   - Embedded GLB payload offset and extraction.

---

## 8. Implementation Phases & Prioritized Checklist

### Phase 1: Code Polish, Clippy Resolution & Bug Fixes (Immediate)
- [ ] Fix I3S LoD pattern match and duplicate blocks in `crates/core/src/gis/i3s/manager.rs`.
- [ ] Resolve all 26 unique Clippy warnings across `s3d-core` and `showcase.rs`.
- [ ] Implement `FromStr` for `PointMarkerStyle` and `Default` for `TerrainManager` and `Tiles3DManager`.
- [ ] Synchronize `GeoJsonBuildings` code snippet in `showcase.rs` with remote fetching UI.
- [ ] Verify `cargo clippy --all-targets` passes with **0 warnings**.
- [ ] Verify all existing unit tests pass with `cargo test`.

### Phase 2: Engine & Renderer Decoupling (High Impact)
- [ ] Create `RenderParams` struct in `crates/core/src/renderer/render_engine.rs`.
- [ ] Remove unused `measurement: &MeasurementEngine` parameter from `render` and `render_with_encoder`.
- [ ] Decouple `ToolboxEngine` from `render_engine.rs` by introducing generic `OverlayLine` rendering.
- [ ] Retain GPU buffer for sightlines to eliminate per-frame dynamic GPU allocations.
- [ ] Box large channel message enum variants in `crates/core/src/gis/threedtiles/manager.rs`.
- [ ] Optimize tile string pruning in `render_engine.rs` to avoid eager allocations.

### Phase 3: Core Automated Test Suite (Reliability)
- [ ] Create `crates/core/tests/spatial_test.rs` (Shoelace area, 3D measurements).
- [ ] Create `crates/core/tests/basemap_tile_test.rs` (tile indexing, quadkeys, zoom calculation).
- [ ] Create `crates/core/tests/command_event_test.rs` (command processing, event dispatching).
- [ ] Run full test suite: `cargo test --workspace`.

### Phase 4: Populate `s3d-control` (Crate Realization)
- [ ] Implement `NavigationGizmo` in `crates/control/src/navigation_gizmo.rs`.
- [ ] Implement `BasemapPicker` in `crates/control/src/basemap_picker.rs`.
- [ ] Implement `LayerTree` in `crates/control/src/layer_tree.rs`.
- [ ] Implement `SolarTimeSlider` in `crates/control/src/solar_slider.rs`.
- [ ] Implement `MeasurementToolbar` in `crates/control/src/measurement_toolbar.rs`.
- [ ] Export public control widgets in `crates/control/src/lib.rs`.

### Phase 5: Scaffold to Desktop Studio `s3d-studio` (Application Assembly)
- [ ] Build multi-panel layout integrating `MapWidget` with `s3d-control` widgets.
- [ ] Add menu bar, feature inspector, and status telemetry bar.
- [ ] Test native desktop execution (`cargo run -p s3d-studio`).
