//! Data-driven rendering system for 3D GIS features, inspired by Esri ArcGIS Renderers.
//!
//! This module defines how features in a layer are styled and symbolized based on their
//! attribute values. It supports:
//! - [`Renderer::Simple`]: All features share a base symbol, optionally scaled by continuous visual variables.
//! - [`Renderer::UniqueValue`]: Categorical mapping from string attribute values to distinct symbols.
//! - [`Renderer::ClassBreaks`]: Quantitative mapping from numeric value ranges (classes) to symbols.
//! - Continuous scaling via [`VisualVariable`] (color ramps, size ranges, opacity ranges).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::gis::symbol::Symbol3D;

/// Type of continuous visual variable used to scale symbol properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualVariableType {
    /// Linearly interpolates color between `min_color` and `max_color` based on values in `[min_value, max_value]`.
    ColorRamp {
        min_value: f64,
        max_value: f64,
        min_color: [f32; 4],
        max_color: [f32; 4],
    },
    /// Linearly interpolates size between `min_size` and `max_size` based on values in `[min_value, max_value]`.
    SizeRange {
        min_value: f64,
        max_value: f64,
        min_size: f32,
        max_size: f32,
    },
    /// Linearly interpolates opacity between `min_opacity` and `max_opacity` based on values in `[min_value, max_value]`.
    OpacityRange {
        min_value: f64,
        max_value: f64,
        min_opacity: f32,
        max_opacity: f32,
    },
}

impl VisualVariableType {
    /// Linearly interpolates between `min_color` and `max_color` using factor `t` in `[0.0, 1.0]`.
    pub fn resolve_color(t: f32, min_color: [f32; 4], max_color: [f32; 4]) -> [f32; 4] {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        [
            min_color[0] + (max_color[0] - min_color[0]) * t,
            min_color[1] + (max_color[1] - min_color[1]) * t,
            min_color[2] + (max_color[2] - min_color[2]) * t,
            min_color[3] + (max_color[3] - min_color[3]) * t,
        ]
    }

    /// Linearly interpolates between `min_size` and `max_size` using factor `t` in `[0.0, 1.0]`.
    pub fn resolve_size(t: f32, min_size: f32, max_size: f32) -> f32 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        min_size + (max_size - min_size) * t
    }

    /// Linearly interpolates between `min_opacity` and `max_opacity` using factor `t` in `[0.0, 1.0]`.
    pub fn resolve_opacity(t: f32, min_opacity: f32, max_opacity: f32) -> f32 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        min_opacity + (max_opacity - min_opacity) * t
    }
}

/// Continuously scales a symbol property (color, size, opacity) by numeric attribute values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualVariable {
    /// The name of the feature attribute field used to calculate the visual variable.
    pub field: String,
    /// The scaling definition and target range.
    pub variable_type: VisualVariableType,
}

impl VisualVariable {
    /// Creates a new visual variable for the specified attribute field.
    pub fn new(field: impl Into<String>, variable_type: VisualVariableType) -> Self {
        Self {
            field: field.into(),
            variable_type,
        }
    }

    /// Computes a normalized interpolation factor `t` in `[0.0, 1.0]` for `value` between `min_val` and `max_val`.
    ///
    /// Values less than or equal to `min_val` return `0.0`.
    /// Values greater than or equal to `max_val` return `1.0`.
    /// If `min_val == max_val`, returns `0.0` to avoid division by zero.
    pub fn interpolate_t(value: f64, min_val: f64, max_val: f64) -> f32 {
        let range = max_val - min_val;
        if range.abs() < 1e-12 {
            return 0.0;
        }
        let t = (value - min_val) / range;
        if t.is_nan() {
            return 0.0;
        }
        t.clamp(0.0, 1.0) as f32
    }

    /// Computes the normalized factor `t` for this visual variable given a feature's attributes.
    ///
    /// Returns `None` if the attribute field is missing or cannot be parsed as a float (`f64`).
    pub fn compute_t(&self, attributes: &HashMap<String, String>) -> Option<f32> {
        let val_str = attributes.get(&self.field)?;
        let val = val_str.trim().parse::<f64>().ok()?;
        match &self.variable_type {
            VisualVariableType::ColorRamp { min_value, max_value, .. }
            | VisualVariableType::SizeRange { min_value, max_value, .. }
            | VisualVariableType::OpacityRange { min_value, max_value, .. } => {
                Some(Self::interpolate_t(val, *min_value, *max_value))
            }
        }
    }

    /// Evaluates this visual variable against feature attributes, returning the interpolated color if this is a `ColorRamp`.
    pub fn evaluate_color(&self, attributes: &HashMap<String, String>) -> Option<[f32; 4]> {
        if let VisualVariableType::ColorRamp { min_color, max_color, .. } = self.variable_type {
            let t = self.compute_t(attributes)?;
            Some(VisualVariableType::resolve_color(t, min_color, max_color))
        } else {
            None
        }
    }

    /// Evaluates this visual variable against feature attributes, returning the interpolated size if this is a `SizeRange`.
    pub fn evaluate_size(&self, attributes: &HashMap<String, String>) -> Option<f32> {
        if let VisualVariableType::SizeRange { min_size, max_size, .. } = self.variable_type {
            let t = self.compute_t(attributes)?;
            Some(VisualVariableType::resolve_size(t, min_size, max_size))
        } else {
            None
        }
    }

    /// Evaluates this visual variable against feature attributes, returning the interpolated opacity if this is an `OpacityRange`.
    pub fn evaluate_opacity(&self, attributes: &HashMap<String, String>) -> Option<f32> {
        if let VisualVariableType::OpacityRange { min_opacity, max_opacity, .. } = self.variable_type {
            let t = self.compute_t(attributes)?;
            Some(VisualVariableType::resolve_opacity(t, min_opacity, max_opacity))
        } else {
            None
        }
    }
}

/// Maps a discrete attribute value to a specific 3D symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniqueValueInfo {
    /// The string attribute value to match against.
    pub value: String,
    /// The symbol to render when the feature's attribute matches `value`.
    pub symbol: Symbol3D,
    /// Human-readable label for legend or UI display.
    #[serde(default)]
    pub label: String,
}

impl UniqueValueInfo {
    /// Creates a new `UniqueValueInfo` entry.
    pub fn new(value: impl Into<String>, symbol: Symbol3D, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            symbol,
            label: label.into(),
        }
    }
}

/// Maps a numeric attribute range `[min_value, max_value]` to a specific 3D symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassBreakInfo {
    /// Minimum numeric value of the class break (inclusive).
    pub min_value: f64,
    /// Maximum numeric value of the class break (inclusive).
    pub max_value: f64,
    /// The symbol to render when the feature's numeric attribute falls in `[min_value, max_value]`.
    pub symbol: Symbol3D,
    /// Human-readable label for legend or UI display.
    #[serde(default)]
    pub label: String,
}

impl ClassBreakInfo {
    /// Creates a new `ClassBreakInfo` entry.
    pub fn new(min_value: f64, max_value: f64, symbol: Symbol3D, label: impl Into<String>) -> Self {
        Self {
            min_value,
            max_value,
            symbol,
            label: label.into(),
        }
    }

    /// Returns `true` if `value` falls within `[min_value, max_value]` inclusive.
    pub fn contains(&self, value: f64) -> bool {
        value >= self.min_value && value <= self.max_value
    }
}

/// Data-driven renderer determining feature symbols and visual properties.
///
/// Corresponds to ArcGIS Web Scene / FeatureLayer renderers:
/// - `Simple`: Single symbol for all features, with optional continuous visual variables.
/// - `UniqueValue`: Categorical symbology based on distinct attribute values.
/// - `ClassBreaks`: Graduated symbology based on numeric value ranges.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Renderer {
    /// Applies a uniform symbol to every feature in the layer, optionally scaled by visual variables.
    Simple {
        symbol: Symbol3D,
        #[serde(default)]
        visual_variables: Vec<VisualVariable>,
    },
    /// Categorizes features by matching a string attribute value to configured unique values.
    UniqueValue {
        field: String,
        default_symbol: Symbol3D,
        unique_value_infos: Vec<UniqueValueInfo>,
        #[serde(default)]
        visual_variables: Vec<VisualVariable>,
    },
    /// Categorizes features by matching a numeric attribute value into ranges (class breaks).
    ClassBreaks {
        field: String,
        default_symbol: Symbol3D,
        class_break_infos: Vec<ClassBreakInfo>,
        #[serde(default)]
        visual_variables: Vec<VisualVariable>,
    },
}

impl Renderer {
    /// Creates a simple renderer using a single symbol for all features.
    pub fn simple(symbol: Symbol3D) -> Self {
        Self::Simple {
            symbol,
            visual_variables: Vec::new(),
        }
    }

    /// Creates a simple renderer with a continuous color ramp visual variable.
    pub fn simple_with_color_ramp(
        symbol: Symbol3D,
        field: &str,
        min_val: f64,
        max_val: f64,
        min_color: [f32; 4],
        max_color: [f32; 4],
    ) -> Self {
        Self::Simple {
            symbol,
            visual_variables: vec![VisualVariable {
                field: field.to_string(),
                variable_type: VisualVariableType::ColorRamp {
                    min_value: min_val,
                    max_value: max_val,
                    min_color,
                    max_color,
                },
            }],
        }
    }

    /// Creates a unique value renderer with a field, default symbol, and list of value mappings.
    pub fn unique_value(
        field: impl Into<String>,
        default_symbol: Symbol3D,
        unique_value_infos: Vec<UniqueValueInfo>,
    ) -> Self {
        Self::UniqueValue {
            field: field.into(),
            default_symbol,
            unique_value_infos,
            visual_variables: Vec::new(),
        }
    }

    /// Creates a class breaks renderer with a field, default symbol, and list of break ranges.
    pub fn class_breaks(
        field: impl Into<String>,
        default_symbol: Symbol3D,
        class_break_infos: Vec<ClassBreakInfo>,
    ) -> Self {
        Self::ClassBreaks {
            field: field.into(),
            default_symbol,
            class_break_infos,
            visual_variables: Vec::new(),
        }
    }

    /// Resolves which symbol should be used for a feature with the given attributes.
    ///
    /// If an attribute is missing or does not match any category or range, this method
    /// safely falls back to the default symbol (or the base symbol for `Renderer::Simple`).
    pub fn get_symbol(&self, attributes: &HashMap<String, String>) -> &Symbol3D {
        match self {
            Renderer::Simple { symbol, .. } => symbol,
            Renderer::UniqueValue {
                field,
                default_symbol,
                unique_value_infos,
                ..
            } => {
                if let Some(val) = attributes.get(field) {
                    for info in unique_value_infos {
                        if &info.value == val {
                            return &info.symbol;
                        }
                    }
                }
                default_symbol
            }
            Renderer::ClassBreaks {
                field,
                default_symbol,
                class_break_infos,
                ..
            } => {
                if let Some(val_str) = attributes.get(field) {
                    if let Ok(val) = val_str.trim().parse::<f64>() {
                        for info in class_break_infos {
                            if info.contains(val) {
                                return &info.symbol;
                            }
                        }
                    }
                }
                default_symbol
            }
        }
    }

    /// Returns a reference to the default (fallback) symbol for this renderer.
    pub fn default_symbol(&self) -> &Symbol3D {
        match self {
            Self::Simple { symbol, .. } => symbol,
            Self::UniqueValue { default_symbol, .. } => default_symbol,
            Self::ClassBreaks { default_symbol, .. } => default_symbol,
        }
    }

    /// Returns the target attribute field name, if applicable.
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Simple { .. } => None,
            Self::UniqueValue { field, .. } => Some(field.as_str()),
            Self::ClassBreaks { field, .. } => Some(field.as_str()),
        }
    }

    /// Returns the list of visual variables associated with this renderer.
    pub fn visual_variables(&self) -> &[VisualVariable] {
        match self {
            Self::Simple { visual_variables, .. }
            | Self::UniqueValue { visual_variables, .. }
            | Self::ClassBreaks { visual_variables, .. } => visual_variables,
        }
    }

    /// Returns a mutable reference to the visual variables list.
    pub fn visual_variables_mut(&mut self) -> &mut Vec<VisualVariable> {
        match self {
            Self::Simple { visual_variables, .. }
            | Self::UniqueValue { visual_variables, .. }
            | Self::ClassBreaks { visual_variables, .. } => visual_variables,
        }
    }
}

