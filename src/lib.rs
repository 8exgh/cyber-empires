//! Cyber Empires re-implementation (strategy + real-time cyborg battles) for WebAssembly.

pub mod app;
pub mod assets;
pub mod battle;
pub mod data;
pub mod gfx;
pub mod gif;
pub mod input;
pub mod rng;
pub mod strategy;
pub mod text;
pub mod ui;
pub mod world_data;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
