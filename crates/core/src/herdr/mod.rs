//! Client for the Herdr socket API (newline-delimited JSON).

pub mod client;
pub mod model;
pub mod transport;

pub use client::{EventStream, HerdrClient, HerdrError};
pub use model::*;
pub use transport::Endpoint;
