//! Starfleet core: everything that is not UI.
//!
//! The Tauri shell is a thin layer over [`app::Starfleet`]; keeping the logic
//! here lets it be tested without a window.

pub mod app;
pub mod env;
pub mod herdr;
pub mod paths;
pub mod preflight;
pub mod stations;
pub mod store;
pub mod view;

pub use app::{DispatchRequest, DispatchResult, Options, Starfleet};
