# S3D V2 Architecture & Project Rules

## 1. Scope & Crate Boundaries
This repository is strictly partitioned into three distinct crates:
- **`crates/core` (`s3d-core`)**: **Map Engine ONLY**. GIS mathematical algorithms, coordinate reference systems (ENU, ECEF, WGS84, Web Mercator), raster tile streaming, 3D DEM terrain, GeoJSON/polygon extrusion, solar calculations, camera navigation, and rendering. Headless by design, with an optional minimal `MapWidget` for viewport embedding.
- **`crates/control` (`s3d-control`)**: Reusable map controls, navigation widgets, layer trees, and UI tools to enhance map functionality.
- **`crates/studio` (`s3d-studio`)**: Full-fledged, ArcGIS Pro / QGIS-like professional desktop application.

## 2. Core Demos & Examples Rule (CRITICAL)
- **Minimal UI Only**:
  - Demos in `crates/core` must **NEVER** include bloated debug dashboards, fake GIS toolbars, camera telemetry sidebars, or mock UI controls that come out of nowhere.
  - Complex widgets and toolbars belong in `crates/control` or `crates/studio`, NOT in `core`.
  - In `core`, the demo UI must be stripped to the bare minimum: a simple example selector at the top, a clean map viewport (`MapWidget`), and a dedicated code section.
- **100% Matching Code Section**:
  - Every example in `core` must provide a prominent **Code Section** (modeled directly after OpenLayers Examples).
  - The code displayed must match the running demo **100%**: it must be the exact, runnable Rust code required to initialize and use that specific `s3d-core` feature.
  - Never add UI features to a demo that are not derived directly from the displayed code snippet.
