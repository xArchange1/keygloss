//! Keygloss engine core. Pure Rust, no OS calls.
//!
//! - [`platform`]: traits and data types the engine needs from the OS.
//! - [`settings`]: the persisted settings schema.

pub mod platform;
pub mod settings;
