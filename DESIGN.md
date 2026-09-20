# S3D Core — Architecture & Design

> A modern, high-performance 3D GIS map engine written in Rust and WebGPU (`wgpu`).
> Runs seamlessly as both a native desktop application and a WebAssembly web app.

---

## 1. Mission & Design Principles

S3D is an open-source 3D Geographic Information System engine built from scratch in Rust.
It draws architectural inspiration from **Esri ArcGIS Maps SDK**, **CesiumJS**, and **OpenLayers**,
but leverages the performance, memory safety, and modern GPU capabilities of Rust and WebGPU.

### Core Principles

| # | Principle | Rationale |
|---|-----------|-----------|
| P1 | **The canvas renders no chrome** | `MapWidget` is a pure GPU viewport. All UI controls live outside the canvas in `s3d-control` / `s3d-studio`. |
| P2 | **Mutation is one-way (Command pattern)** | All state changes flow through `MapCommand` → `MapEngine::apply()`. No scattered mutation. |
| P3 | **State is data; GPU objects are derived** | The `Map` document, layers, and features are plain serializable Rust structs. GPU buffers are rebuilt from state. |
| P4 | **Streaming is budgeted** | Tile downloads, LOD traversals, and GPU uploads are bounded by `ResourceBudget` — never unbounded. |
| P5 | **No flicker on LOD change** | Parent tiles remain visible until children are fully decoded and GPU-ready. |
| P6 | **The render thread never blocks** | All network I/O and heavy decoding happen on background threads (native) or async tasks (WASM). |
| P7 | **100% code transparency** | Showcase examples display the **exact source file** that runs. No hardcoded snippets. |

---

## 2. Repository & Crate Boundaries

```
s3d_v2/
├── crates/
│   ├── core/       ← s3d-core    Map Engine (this document)
│   ├── control/    ← s3d-control Reusable map controls & widgets
│   └── studio/     ← s3d-studio  Full desktop GIS application
└── assets/
```

### `s3d-core` — Headless Map Engine

Everything in this document. Headless by design — can run without any UI framework.
Optional `egui` feature gate enables `MapWidget` for viewport embedding.

**Owns**: GIS math, CRS, map document model, layers, streaming (basemap, terrain, I3S,
3D Tiles, ArcGIS), rendering pipeline, solar calculation, spatial queries, camera navigation.

### `s3d-control` — Reusable Map Controls

Modular UI widgets that enhance the map: navigation gizmo, compass, measurement tools,
basemap selector, layer tree, sun/time slider. Depends on `s3d-core`.

### `s3d-studio` — Desktop Application

Full-featured ArcGIS Pro / QGIS-style professional workstation.
Integrates `s3d-core` + `s3d-control` with project management, attribute tables,
ribbon toolbars, and spatial analytics.

---

## 3. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         s3d-core                                    │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    MapEngine (Façade)                         │   │
│  │  apply(MapCommand) → poll_events() → MapEvent               │   │
│  └─────────┬──────────┬──────────┬──────────┬──────────────────┘   │
│            │          │          │          │                        │
│  ┌─────────▼───┐ ┌────▼────┐ ┌──▼──────┐ ┌▼────────────────────┐  │
│  │ Map Document│ │ Camera  │ │ Scene   │ │ Streaming Managers  │  │
│  │             │ │         │ │         │ │                     │  │
│  │ Basemap     │ │ Orbit   │ │ Nodes   │ │ BasemapManager      │  │
│  │ Ground      │ │ GoTo    │ │ Materials│ │ TerrainManager     │  │
│  │ Layers      │ │ Flight  │ │ Bounds  │ │ I3SManager          │  │
│  │ Tables      │ │ Frustum │ │         │ │ Tiles3DManager      │  │
│  │ ViewingMode │ │         │ │         │ │ ArcGISFeatureManager│  │
│  └─────────────┘ └─────────┘ └─────────┘ └─────────────────────┘  │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    RenderEngine (wgpu)                        │   │
│  │  Pipelines │ Shadow Map │ Edge Detection │ JFA Highlight     │   │
│  │  shader.wgsl │ edges.wgsl │ jfa.wgsl                        │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌───────────┐ ┌───────────┐ ┌───────────────┐ ┌───────────────┐  │
│  │ GIS / CRS │ │ Solar     │ │ Spatial       │ │ Compute       │  │
│  │           │ │           │ │               │ │               │  │
│  │ WGS84     │ │ SunCalc   │ │ Picking       │ │ Parallel      │  │
│  │ ECEF      │ │ Shadow    │ │ Measurement   │ │ Worker        │  │
│  │ ENU       │ │ Analysis  │ │ Sketch        │ │               │  │
│  │ Mercator  │ │ DateTime  │ │ Toolbox       │ │               │  │
│  └───────────┘ └───────────┘ └───────────────┘ └───────────────┘  │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ #[cfg(feature = "egui")]  MapWidget → MapResponse            │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Module Map

```
src/
├── lib.rs                          Top-level re-exports
│
├── engine/                         Engine Façade & Interaction
│   ├── map_engine.rs               MapEngine — central orchestrator
│   ├── command.rs                  MapCommand enum (Camera, Layer, Basemap, Terrain, I3S, Clock, Environment, Edge)
│   ├── event.rs                    MapEvent enum (CoordinatesClicked, FeatureSelected, CameraMoved, …)
│   ├── view.rs                     MapView — immutable read-only snapshot for UI
│   ├── navigation.rs              Pan, orbit, zoom controllers & gesture handling
│   └── widget.rs                   [egui] MapWidget + MapResponse
│
├── gis/                            GIS Document Model, Formats & Streaming
│   ├── map.rs                      Map, MapBuilder, Basemap, Ground, LayerCollection, ViewingMode
│   ├── layer.rs                    Layer trait, FeatureLayer, TileLayer, ElevationLayer, SceneLayer,
│   │                               IntegratedMeshLayer, GraphicsLayer, GroupLayer, LayerRegistry
│   ├── crs.rs                      GeoCoord, ProjectOrigin, ProjectionMode, WGS84↔ECEF↔ENU↔Mercator
│   ├── geometry.rs                 Point, Polyline, Polygon, Extent, Mesh, SpatialReference
│   ├── feature.rs                  Feature, Geometry (Point/Polygon/Mesh), PointSymbol3D
│   ├── graphic.rs                  Graphic (Geometry + Symbol + Attributes)
│   ├── symbol.rs                   Symbol3D (Point/Line/Polygon/Mesh), 3D symbol layers
│   ├── renderer.rs                 Renderer (Simple, UniqueValue, ClassBreaks), VisualVariable
│   ├── source.rs                   Source trait, TileSource, SourceRegistry, XyzRasterSource
│   ├── basemap.rs                  BasemapManager — slippy tile streaming (OSM, Esri)
│   ├── terrain.rs                  TerrainManager — DEM elevation streaming (LERC, Terrarium, Mapbox RGB)
│   ├── geojson_loader.rs          GeoJSON parser, GisFeature, parallel triangulation
│   ├── extrusion.rs                RawMeshData, polygon triangulation (earcutr), 3D extrusion
│   ├── arcgis.rs                   ArcGIS REST FeatureServer/MapServer client
│   ├── cache.rs                    ResourceBudget, LRU cache
│   ├── rasterizer.rs              Vector-to-raster draping via tiny-skia
│   ├── i3s/                        ◆ Esri I3S Scene Layer format
│   │   ├── spec.rs                 I3S 1.7/1.8 JSON schema, OBB, nodepages
│   │   ├── decoder.rs             Binary geometry decoder, per-building segmentation
│   │   └── manager.rs             I3SManager — background streaming, LOD traversal
│   ├── threedtiles/                ◆ OGC 3D Tiles format
│   │   ├── spec.rs                 TilesetJson, Tile3DNode, BoundingVolume, SSE
│   │   ├── b3dm.rs                 B3DM / CMPT / GLB container parsers
│   │   ├── gltf.rs                 Embedded glTF 2.0 binary parser
│   │   └── manager.rs             Tiles3DManager — background streaming, LOD traversal
│   └── platform/
│       └── http.rs                 Cross-platform HTTP (ureq native / ehttp WASM)
│
├── renderer/                       GPU Rendering Pipeline (wgpu)
│   ├── render_engine.rs           Main render loop, offscreen textures, pass orchestration
│   ├── camera.rs                   Camera, GoTo animations, Globe flight state machine
│   ├── pipeline.rs                 wgpu pipelines, bind groups, vertex layouts
│   ├── mesh.rs                     GPU vertex/index buffers, terrain mesh generation
│   ├── shadow_map.rs              Directional shadow map pass
│   ├── edges.rs                    CAD silhouette edge detection
│   ├── jfa_highlight.rs           JFA selection outline halos
│   ├── shader.wgsl                 Core PBR + shadow + globe morph shader
│   ├── edges.wgsl                  Edge detection shader
│   └── jfa.wgsl                    Jump Flood Algorithm shader
│
├── scene/                          Scene Graph
│   ├── scene.rs                    Scene container (origin, nodes, bounds)
│   ├── node.rs                     SceneNode (transform, material, mesh ref, AABB)
│   └── material.rs                 PBR material parameters
│
├── solar/                          Astronomical Calculation
│   ├── sun_calc.rs                 NOAA solar position algorithm (azimuth, elevation)
│   ├── shadow_analysis.rs         Shadow raycasting & compliance analysis
│   └── datetime_state.rs          Simulation date/time state
│
├── spatial/                        Spatial Queries & Tools
│   ├── picking.rs                  screen_to_ray, Ray, ground/scene intersection
│   ├── measurement.rs             Distance, area, height clearance
│   ├── sketch.rs                   Interactive 3D sketching engine
│   └── toolbox.rs                  Spatial analysis (viewshed, line-of-sight)
│
└── compute/                        Parallelism
    ├── parallel.rs                 Rayon (native) / sequential fallback (WASM)
    └── worker.rs                   Background task dispatcher
```

---

## 5. Map Document Model

The `Map` is a headless, serializable GIS document inspired by the Esri ArcGIS Maps SDK:

```
Map
├── id: String
├── title: String
├── spatial_reference: SpatialReference
├── origin: GeoCoord                    ← Project anchor (double→single precision pivot)
├── viewing_mode: ViewingMode           ← Globe | Planar | Auto { threshold_altitude }
├── basemap: Option<Basemap>
│   ├── base_layers: LayerCollection    ← Raster tile layers (OSM, Esri, etc.)
│   └── reference_layers: LayerCollection
├── ground: Ground
│   ├── layers: LayerCollection         ← ElevationLayer instances (DEM sources)
│   └── elevation_exaggeration: f32
├── layers: LayerCollection             ← Operational layers (features, scene layers, etc.)
└── tables: TableCollection             ← Non-spatial attribute tables
```

### MapBuilder (Fluent API)

```rust
let map = Map::builder()
    .title("Melbourne CBD")
    .basemap(Basemap::osm())
    .origin([144.9631, -37.8136, 0.0])
    .viewing_mode(ViewingMode::Auto { threshold_altitude: 50_000.0 })
    .ground(Ground::with_elevation_layer(Arc::new(
        ElevationLayer::mapbox_terrain_rgb("dem", "https://...")
    )))
    .build();
```

---

## 6. Coordinate Reference Systems

S3D maintains four coordinate spaces and provides exact conversions between them:

```
┌────────────────────────────────────────────────────────────────────┐
│                    Coordinate Pipeline                              │
│                                                                     │
│  WGS84 (lon°, lat°, elev_m)                                       │
│    │                                                                │
│    ├──► Web Mercator (EPSG:3857)     Slippy tile math              │
│    │      x = lon * 20037508.34/180                                │
│    │      y = ln(tan(π/4 + lat_rad/2)) * 20037508.34/π            │
│    │                                                                │
│    ├──► ECEF (Earth-Centered Fixed)  Globe rendering               │
│    │      N = a / √(1 - e²sin²φ)                                  │
│    │      X = (N+h)cosφ cosλ                                       │
│    │      Y = (N+h)cosφ sinλ                                       │
│    │      Z = (N(1-e²)+h) sinφ                                     │
│    │                                                                │
│    └──► Local ENU (East-North-Up)    Planar rendering              │
│           ProjectOrigin anchors the tangent plane                   │
│           ΔE = Δlon·cos(lat)·Rₑ,  ΔN = Δlat·Rₙ,  ΔU = Δelev     │
│           Engine axes: X=East, Y=Up, Z=−North                      │
│                                                                     │
│  Dual-projection vertex shader morphs between ENU and ECEF         │
│  based on ProjectionMode + morph_progress uniform                   │
└────────────────────────────────────────────────────────────────────┘
```

### Key Types

| Type | Purpose |
|------|---------|
| `GeoCoord` | `(latitude°, longitude°, elevation_m)` — WGS84 geographic |
| `ProjectOrigin` | Tangent plane anchor with precomputed radii of curvature (`re`, `rn`) |
| `ProjectionMode` | `PlanarENU` or `GlobeECEF` — controls which coordinate space is active |

### Geodetic Functions

- `geodetic_to_ecef` / `ecef_to_geodetic` — Bowring's sub-millimeter method
- `geo_to_local` / `local_to_geo` — ENU ↔ WGS84 via `ProjectOrigin`
- `wgs84_to_web_mercator` / `web_mercator_to_wgs84` — tile coordinate math
- `haversine_distance` — great-circle distance between two points
- `geodetic_surface_normal` — surface normal vector for globe lighting

---

## 7. Layer System & Format Support

### Layer Trait Hierarchy

```rust
pub trait Layer: Debug + Send + Sync + 'static {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn layer_type(&self) -> LayerType;
    fn visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn opacity(&self) -> f32;
    fn descriptor(&self) -> LayerDescriptor;
    fn update(&mut self) -> LayerStatus;
    fn as_any(&self) -> &dyn Any;
    // ...
}
```

### Concrete Layer Types

| Layer Type | Struct | Format | Streaming |
|------------|--------|--------|-----------|
| **Raster Basemap** | `TileLayer` | XYZ/TMS PNG/JPEG tiles | `BasemapManager` |
| **Elevation DEM** | `ElevationLayer` | Mapbox Terrain-RGB, Terrarium PNG, Esri LERC | `TerrainManager` |
| **Vector Features** | `FeatureLayer` | GeoJSON, ArcGIS FeatureServer | Inline / `ArcGISFeatureManager` |
| **3D Buildings** | `FeatureLayer` | GeoJSON extrusion | Inline |
| **I3S Scene Layer** | `SceneLayer` | Esri I3S 1.7/1.8 (IntegratedMesh, 3DObject) | `I3SManager` |
| **3D Tiles** | `IntegratedMeshLayer` | OGC 3D Tiles 1.0 (B3DM, CMPT, GLB) | `Tiles3DManager` |
| **Graphics** | `GraphicsLayer` | Programmatic `Graphic` objects | — |
| **Group** | `GroupLayer` | Container for nested layers | — |

### Format Support Matrix

| Format | Status | Parser | Notes |
|--------|--------|--------|-------|
| **GeoJSON** | ✅ Complete | `geojson_loader.rs` | FeatureCollection, polygon triangulation via `earcutr`, height attribute extraction, 3D extrusion |
| **Esri I3S** | ✅ Complete | `gis/i3s/` | SceneLayer JSON, 64-node nodepages, binary vertex buffers, gzip decompression, OBB frustum culling, per-building feature segmentation |
| **OGC 3D Tiles** | ✅ Complete | `gis/threedtiles/` | `tileset.json` hierarchy, B3DM, CMPT composites, embedded glTF/GLB, screen-space error LOD, batch table attributes, Union-Find building clustering |
| **glTF / GLB** | ✅ Complete | `threedtiles/gltf.rs` | Hand-written parser: accessors, buffer views, PBR materials, embedded images, node graph traversal, normal matrix transforms |
| **Esri LERC** | ✅ Complete | `terrain.rs` + `lerc-rs` | Limited Error Raster Compression for elevation tiles |
| **Mapbox Terrain-RGB** | ✅ Complete | `terrain.rs` | `h = (R*256 + G + B/256) - 32768` |
| **Terrarium** | ✅ Complete | `terrain.rs` | `h = (R*256 + G + B/256) - 32768` (same formula) |
| **ArcGIS REST** | ✅ Complete | `arcgis.rs` | FeatureServer vector query + MapServer raster export, dynamic Douglas-Peucker generalization, scale gating |
| **XYZ / TMS Tiles** | ✅ Complete | `basemap.rs` | OSM, Esri World Imagery/Streets/Topo |
| **KML / KMZ** | ⬜ Planned | — | — |
| **Shapefile** | ⬜ Planned | — | — |
| **FlatGeobuf** | ⬜ Planned | — | — |
| **OGC WMS / WMTS** | ⬜ Planned | — | — |
| **OGC WFS** | ⬜ Planned | — | — |

---

## 8. Streaming Architecture

All data streaming follows a unified architecture:

```
                ┌──────────────────────┐
                │     MapEngine        │
                │  update_streaming()  │
                └──────────┬───────────┘
                           │
            ┌──────────────┼──────────────┐
            ▼              ▼              ▼
     ┌──────────┐   ┌──────────┐   ┌──────────┐
     │ Basemap  │   │ Terrain  │   │ I3S /    │
     │ Manager  │   │ Manager  │   │ 3DTiles  │
     └────┬─────┘   └────┬─────┘   └────┬─────┘
          │               │               │
  ┌───────▼───────────────▼───────────────▼───────┐
  │              Work Queue (Arc<Mutex>)            │
  │  tasks: Vec<Task>                              │
  │  valid_keys: HashSet                           │
  │  in_flight: usize                              │
  └───────────────────┬───────────────────────────┘
                      │
          ┌───────────┼───────────┐
          ▼           ▼           ▼
     ┌─────────┐ ┌─────────┐ ┌─────────┐
     │Worker 1 │ │Worker 2 │ │Worker N │   (N ≤ 12 native, async WASM)
     │ ureq /  │ │ ureq /  │ │ ureq /  │
     │ ehttp   │ │ ehttp   │ │ ehttp   │
     └────┬────┘ └────┬────┘ └────┬────┘
          │           │           │
          └───────────┼───────────┘
                      ▼
              Result Channel (mpsc)
                      │
                      ▼
              MapEngine::update()
              ├── decode tile
              ├── upload to GPU
              └── update scene graph
```

### Per-Manager Details

#### BasemapManager
- **Input**: Camera viewport → visible tile set at zoom level `z`
- **Output**: `DecodedTile` (RGBA8 raster)
- **Cache**: In-memory LRU `TileRamCache` + GPU texture atlas
- **Tile Math**: Standard Web Mercator slippy map quadtree

#### TerrainManager
- **Input**: Visible tile set (same as basemap)
- **Output**: `DecodedTerrainTile` — 33×33 float32 height grid per tile
- **Formats**: Esri LERC, Terrarium PNG, Mapbox Terrain-RGB
- **Features**: Bilinear elevation sampling, skirt generation, multi-resolution ancestor resampling

#### I3SManager
- **Input**: Camera frustum + LOD metric thresholds
- **Output**: `DecodedI3SNode` — GPU-ready vertex buffers + per-feature mesh splits
- **LOD**: Hierarchical nodepage traversal with OBB SAT frustum culling
- **Metrics**: `maxScreenThresholdSQ`, `maxScreenThreshold`, `screenSpaceError`
- **Throttle**: Re-evaluates LOD at most every 150ms during camera motion

#### Tiles3DManager
- **Input**: Camera frustum + `maximum_screen_space_error` threshold
- **Output**: `DecodedThreeDTileMesh` — positions (ENU + ECEF dual), normals, UVs, batch attributes
- **LOD**: Recursive `tileset.json` traversal with screen-space error metric
- **Refinement**: `Add` (parent + children) vs `Replace` (children replace parent)
- **Reprojection**: Zero-network `reproject_all()` when origin changes — recomputes ENU from cached ECEF

---

## 9. Data-Driven Symbology

The rendering model follows Esri's data-driven symbology pattern:

```
Layer
├── renderer: Option<Renderer>
│   ├── Simple { symbol, visual_variables }
│   ├── UniqueValue { field, unique_value_infos, default_symbol }
│   └── ClassBreaks { field, class_break_infos, default_symbol }
│
└── features: Vec<GisFeature>
    └── properties: HashMap<String, String>   ← attributes drive symbol selection
```

### Symbol3D Variants

```
Symbol3D
├── Point
│   ├── Icon    (2D screen billboard: Circle, Square, Triangle, Diamond)
│   └── Object  (3D volumetric: Cube, Sphere, Cylinder, Cone, Tetrahedron)
├── Line
│   ├── Line    (screen-space stroke with cap/join)
│   └── Path    (3D tube with width, height, profile)
├── Polygon
│   ├── Fill    (2D flat fill with outline)
│   └── Extrude (3D extrusion with height and shadows)
└── Mesh        (PBR material for pre-built meshes)
```

### Visual Variables

Continuous attribute-driven modifications applied on top of the base symbol:

| Type | Effect |
|------|--------|
| `ColorRamp` | Interpolate color between `min_color` → `max_color` based on numeric field |
| `SizeRange` | Scale geometry size between `min_size` → `max_size` |
| `OpacityRange` | Fade opacity between `min_opacity` → `max_opacity` |

---

## 10. Engine Façade — `MapEngine`

`MapEngine` is the central orchestrator that owns all state and managers:

```rust
pub struct MapEngine {
    // GIS Document
    pub map: Map,

    // Rendering
    pub renderer: Option<RenderEngine>,
    pub scene: Scene,
    pub camera: Camera,
    pub projection_mode: ProjectionMode,

    // Streaming Managers
    pub basemap: BasemapManager,
    pub terrain: TerrainManager,
    pub i3s: I3SManager,
    pub threedtiles: Tiles3DManager,

    // Feature Layers & Sources
    pub layers: Vec<FeatureLayer>,
    pub sources: SourceRegistry,
    pub layer_registry: LayerRegistry,

    // Solar & Environment
    pub solar_dt: SolarDateTimeState,
    pub solar_pos: SolarPosition,
    pub sunlight_enabled: bool,

    // Selection & Events
    pub selected_feature: Option<GisFeature>,
    pub events: Vec<MapEvent>,
    // ...
}
```

### Command → Event Flow

```
User Interaction / API Call
    │
    ▼
MapCommand (enum)
    │  Camera(Pan { dx, dy })
    │  Layer(SetVisibility { id, visible })
    │  Basemap(SetProvider(EsriImagery))
    │  Terrain(SetHeightExaggeration(2.0))
    │  I3S(SetServiceUrl("https://..."))
    │  Clock(SetTime { hour: 14, minute: 0 })
    │  Environment(SetSunIntensity(1.5))
    │  Edge(SetEnabled(true))
    │
    ▼
MapEngine::apply(cmd)
    │  Mutates internal state
    │  Triggers streaming reconfiguration
    │
    ▼
MapEngine::update(dt)
    │  Advance camera animation
    │  Process streaming results
    │  Rebuild GPU meshes if dirty
    │
    ▼
MapEvent (enum)
    │  CoordinatesClicked { geo, local }
    │  FeatureSelected(Option<GisFeature>)
    │  CameraMoved
    │  BasemapProviderChanged(provider)
    │  StatusChanged(message)
    │
    ▼
UI / Application consumes events
```

---

## 11. Rendering Pipeline

The `RenderEngine` orchestrates a multi-pass wgpu rendering pipeline:

```
Frame
│
├─ Pass 1: Shadow Map
│  └─ Directional light depth-only render of all shadow-casting geometry
│
├─ Pass 2: Main Forward Render (shader.wgsl)
│  ├─ Basemap raster tiles (textured quads at z=0 or terrain-draped)
│  ├─ Terrain DEM mesh (33×33 grid per tile, with skirts)
│  ├─ Feature layers (extruded buildings, vector geometry)
│  ├─ I3S scene layer nodes
│  ├─ 3D Tiles content meshes
│  └─ Graphics layer objects
│
├─ Pass 3: Edge Detection (edges.wgsl)
│  └─ Sobel-filter depth + normal discontinuities → CAD silhouette edges
│
├─ Pass 4: JFA Selection Highlight (jfa.wgsl)
│  └─ Jump Flood Algorithm → glowing outline halo on selected features
│
└─ Composite → egui texture
```

### Dual Projection in Shader

The vertex shader computes **both** planar ENU and globe ECEF positions,
then blends between them using a `morph_progress` uniform (0.0 = planar, 1.0 = globe):

```wgsl
let pos_enu = model_matrix * vec4<f32>(position, 1.0);
let pos_ecef = enu_to_ecef(pos_enu.xyz);
let final_pos = mix(pos_enu, pos_ecef, morph_progress);
```

This enables smooth animated transitions between city-level planar view
and planetary globe view without rebuilding any geometry.

---

## 12. Camera & Navigation

```
Camera
├── Orbit Model: target + distance + pitch + yaw
├── Smooth Interpolation: target_* fields with exponential decay
├── GoTo API: GoToTarget (Geo or Local) + GoToOptions (distance, heading, tilt, animate)
└── Globe Flight: 3-phase state machine
    ├── Phase 1: Spinning  (rotate globe to target longitude/latitude)
    ├── Phase 2: Descent   (zoom from orbital to city altitude)
    └── Phase 3: Landing   (switch to planar ENU + final camera pose)
```

### Navigation Controllers

| Mode | Input | Behavior |
|------|-------|----------|
| **Pan** | Middle-drag / Shift+drag | Translates `camera.target` on the ground plane |
| **Orbit** | Left-drag | Rotates `yaw` and `pitch` around target |
| **Zoom** | Scroll wheel | Scales `camera.distance` toward cursor world point |
| **GoTo** | API call | Animated flight to geographic coordinate |
| **Globe Flight** | `fly_to_geo()` | Multi-phase animated transition with projection switching |

---

## 13. Solar & Shadow System

```
SolarDateTimeState (hour, minute, month, day, year, timezone)
    │
    ▼
calculate_solar_position(lat, lon, datetime) → SolarPosition { azimuth, elevation }
    │
    ▼
Directional Light → Shadow Map Pass → Projected Shadows on 3D Geometry
```

### Shadow Analysis

The `shadow_analysis` module provides:
- **Raycasting**: Scene collider tests ray against all building meshes
- **Duration Maps**: Accumulated shadow hours for statutory compliance
- **False-Color Heatmaps**: Visual representation of shadow impact

---

## 14. Spatial Query & Analysis

| Capability | Module | Description |
|-----------|--------|-------------|
| **Raycasting** | `picking.rs` | `screen_to_ray()` + ground/scene/terrain intersection |
| **Feature Picking** | `picking.rs` | Ray-AABB then ray-triangle tests against feature meshes |
| **Measurement** | `measurement.rs` | 3D distance, height clearance, area calculation |
| **Sketching** | `sketch.rs` | Interactive drawing on terrain/building surfaces |
| **Viewshed** | `toolbox.rs` | Observer visibility analysis from a point |
| **Line of Sight** | `toolbox.rs` | Visibility between two points |

---

## 15. Technology Stack

| Domain | Technology | Version |
|--------|------------|---------|
| **Language** | Rust | 2021 Edition |
| **GPU** | wgpu (Vulkan, Metal, DX12, WebGPU) | 24.0 |
| **Shading** | WGSL | — |
| **GUI** | egui / eframe (optional) | 0.31 |
| **Linear Algebra** | glam (SIMD) | 0.29 |
| **GIS Geometry** | geojson + earcutr | 0.24 / 0.4 |
| **Raster** | image (PNG/JPEG) | 0.25 |
| **Elevation** | lerc-rs (Esri LERC) | 0.5 |
| **Compression** | flate2 (gzip/deflate) | 1.0 |
| **2D Rasterizer** | tiny-skia | 0.11 |
| **Time** | chrono + web-time | 0.4 / 1.1 |
| **Serialization** | serde + serde_json | 1.0 |
| **HTTP (Native)** | ureq (TLS) | 2.10 |
| **HTTP (WASM)** | ehttp | 0.5 |
| **Parallelism** | rayon (native) | 1.10 |
| **WASM** | wasm-bindgen + web-sys | 0.2 / 0.3 |

---

## 16. Cross-Platform Strategy

```
                    s3d-core (Rust)
                    ┌─────────────────────┐
                    │   Shared Logic       │
                    │   GIS, CRS, Render   │
                    │   Camera, Solar      │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                                 ▼
     Native Desktop                      WebAssembly (WASM)
   ┌──────────────────┐            ┌──────────────────┐
   │ eframe (wgpu)    │            │ eframe (wgpu)    │
   │ ureq (HTTP/TLS)  │            │ ehttp (fetch)    │
   │ rayon (threads)   │            │ sequential iter  │
   │ rfd (file dialog)│            │ web-sys (DOM)    │
   │ std::fs (cache)  │            │ LocalStorage     │
   └──────────────────┘            └──────────────────┘
```

Platform-specific code is isolated behind `#[cfg(target_arch = "wasm32")]` /
`#[cfg(not(target_arch = "wasm32"))]` gates in:
- `gis/platform/http.rs` — HTTP fetching
- `compute/parallel.rs` — parallel iteration
- `gis/basemap.rs`, `gis/terrain.rs`, `gis/i3s/manager.rs`, `gis/threedtiles/manager.rs` — worker spawning

---

## 17. Showcase Examples

Demos live in `crates/core/examples/demos/` — one file per demo:

| Demo | File | Features Demonstrated |
|------|------|----------------------|
| Simple Map | `simple_map.rs` | Map + OSM basemap + auto viewing mode |
| Basemap Switcher | `basemap_switcher.rs` | Runtime basemap provider switching |
| GeoJSON Buildings | `geojson_buildings.rs` | GeoJSON parsing, triangulation, 3D extrusion |
| Sun & Shadows | `sun_and_shadows.rs` | Solar calculation, directional shadow casting |
| Terrain Elevation | `terrain_elevation.rs` | DEM terrain with height exaggeration |
| Camera Navigation | `camera_fly_to.rs` | GoTo preset camera angles |
| Coordinate Picking | `coordinate_picking.rs` | Screen-to-ray WGS84 coordinate extraction |

The showcase runner (`showcase.rs`) uses `include_str!` to display each demo's
**actual source file** in the code panel — what you see IS what runs.

---

## Appendix A: Migration from Old S3D

The old project (`D:\code\github\s3d`) was a single monolithic crate that grew to ~60+ source files.
The v2 project preserves all core algorithms while cleanly separating concerns:

| Old s3d | New s3d_v2 | Status |
|---------|------------|--------|
| `src/gis/i3s/` | `crates/core/src/gis/i3s/` | ✅ Migrated |
| `src/gis/threedtiles/` | `crates/core/src/gis/threedtiles/` | ✅ Migrated |
| `src/gis/terrain.rs` | `crates/core/src/gis/terrain.rs` | ✅ Migrated |
| `src/gis/arcgis.rs` | `crates/core/src/gis/arcgis.rs` | ✅ Migrated |
| `src/gis/basemap.rs` | `crates/core/src/gis/basemap.rs` | ✅ Migrated |
| `src/gis/crs.rs` | `crates/core/src/gis/crs.rs` | ✅ Migrated |
| `src/smap/engine.rs` | `crates/core/src/engine/map_engine.rs` | ✅ Migrated (renamed) |
| `src/smap/command.rs` | `crates/core/src/engine/command.rs` | ✅ Migrated |
| `src/smap/controls/` | `crates/control/src/` | 🔲 Pending |
| `src/ui/` | `crates/studio/src/` | 🔲 Pending |
| `src/plugin/` | `crates/studio/src/` | 🔲 Pending |
| `src/showcase/demos/` (31 demos) | `crates/core/examples/demos/` (8 demos) | 🔲 Partial — 23 more to port |

### Old Showcase Demos Not Yet Ported

These 23 demos from the old project should be ported as new demo files:

| Demo | Priority | Dependencies |
|------|----------|-------------|
| `i3s_layer.rs` | 🔴 High | I3SManager (already in core) |
| `threed_tiles.rs` | 🔴 High | Tiles3DManager (already in core) |
| `gltf_model.rs` | 🔴 High | glTF parser (already in core) |
| `terrain_dem.rs` | 🟡 Medium | TerrainManager (already in core) |
| `flyto_landmarks.rs` | 🟡 Medium | GoTo API (already in core) |
| `cad_edges.rs` | 🟡 Medium | Edge detection pipeline |
| `unique_value_renderer.rs` | 🟡 Medium | Renderer system |
| `class_breaks_renderer.rs` | 🟡 Medium | Renderer system |
| `simple_feature_layer.rs` | 🟡 Medium | FeatureLayer |
| `composite_3d_symbols.rs` | 🟡 Medium | Symbol3D |
| `vector_primitives.rs` | 🟡 Medium | Geometry types |
| `raster_tiles.rs` | 🟡 Medium | BasemapManager |
| `solar_shadows.rs` | 🟡 Medium | Solar + shadow system |
| `shadow_colors.rs` | 🟡 Medium | Shadow analysis |
| `shadow_probe.rs` | 🟡 Medium | Shadow raycasting |
| `elevation_profile.rs` | 🟡 Medium | Terrain sampling |
| `height_clamping.rs` | 🟡 Medium | ElevationMode::OnGround |
| `coordinate_transform.rs` | 🟢 Low | CRS functions |
| `measurement.rs` | 🟢 Low | Spatial measurement |
| `sketch_drawing.rs` | 🟢 Low | Sketch engine |
| `spatial_analysis.rs` | 🟢 Low | Toolbox |
| `spatial_heatmap.rs` | 🟢 Low | Toolbox |
| `flight_tracker.rs` | 🟢 Low | Animated graphics |
