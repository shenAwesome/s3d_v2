use glam::Vec3;
use crate::gis::basemap::BasemapProvider;
use crate::gis::crs::GeoCoord;
use crate::gis::layer::{FeatureLayer, LayerDescriptor};
use crate::engine::map_engine::{GoToOptions, GoToTarget};
use crate::solar::datetime_state::SolarDateTimeState;

/// Sub-commands for camera manipulation
#[derive(Debug, Clone, PartialEq)]
pub enum CameraCommand {
    Pan { delta_x: f32, delta_y: f32 },
    Orbit { delta_yaw: f32, delta_pitch: f32 },
    Zoom(f32),
    ZoomScale(f32),
    ViewTop,
    ViewPerspective,
    AlignNorth,
    RotateYaw(f32),
    ResetView,
    FlyTo { target_geo: GeoCoord, distance: f32 },
    LookAt { target: Vec3, distance: f32, pitch: f32, yaw: f32 },
    GoTo { target: GoToTarget, options: Option<GoToOptions> },
}

/// Sub-commands for layer property mutations
#[derive(Debug, Clone)]
pub enum LayerCommand {
    SetVisibility { id: String, visible: bool },
    SetOpacity { id: String, opacity: f32 },
    SetColorTint { id: String, tint: [f32; 4] },
    SetShadow { id: String, cast_shadows: bool },
    Remove(String),
    Add(Box<FeatureLayer>),
    AddDescriptor(LayerDescriptor),
}

/// Sub-commands for basemap raster tile streaming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasemapCommand {
    SetProvider(BasemapProvider),
    SetEnabled(bool),
    ResetCache,
}

/// Sub-commands for 3D terrain elevation streaming
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerrainCommand {
    SetEnabled(bool),
    SetHeightExaggeration(f32),
}

/// Sub-commands for scene clock & solar time
#[derive(Debug, Clone)]
pub enum ClockCommand {
    SetTime { hour: u32, minute: u32 },
    SetDate { month: u32, day: u32 },
    SetDateTime(SolarDateTimeState),
}

/// Sub-commands for lighting and environment
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentCommand {
    SetSunlightEnabled(bool),
    SetSunIntensity(f32),
    SetAmbientIntensity(f32),
}

/// Sub-commands for CAD silhouette edge rendering
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeCommand {
    SetEnabled(bool),
    SetWidth(f32),
    SetColor([f32; 4]),
    SetDepthThreshold(f32),
    SetNormalThreshold(f32),
}

/// Top-level unified command enum for deterministic one-way state mutations.
#[derive(Debug)]
pub enum MapCommand {
    Camera(CameraCommand),
    Layer(LayerCommand),
    Basemap(BasemapCommand),
    Terrain(TerrainCommand),
    Clock(ClockCommand),
    Environment(EnvironmentCommand),
    Edge(EdgeCommand),
    SetStatusMessage(String),
}

/// Error returned when a command fails to apply
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    LayerNotFound(String),
    SourceNotFound(String),
    InvalidParameter(String),
    NotSupported(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LayerNotFound(id) => write!(f, "Layer with ID '{}' not found", id),
            Self::SourceNotFound(id) => write!(f, "Source with ID '{}' not found", id),
            Self::InvalidParameter(msg) => write!(f, "Invalid command parameter: {}", msg),
            Self::NotSupported(msg) => write!(f, "Operation not supported: {}", msg),
        }
    }
}

impl std::error::Error for CommandError {}
