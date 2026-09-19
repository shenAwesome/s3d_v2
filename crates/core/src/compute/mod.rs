//! Compute Subsystem: Multi-threaded & Parallel Execution Engine
//!
//! Exposes universal parallel iterators (Rayon on Desktop, transparent fallback on WASM)
//! and background task dispatchers for high-performance GIS and spatial analysis.

pub mod parallel;
pub use parallel::*;

pub mod worker;
pub use worker::*;
