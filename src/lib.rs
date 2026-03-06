//! Hindsight HTTP client for Rust.
//!
//! This crate provides a client for the Hindsight agent memory system API.

mod client;
mod error;
mod models;

pub use client::HindsightClient;
pub use error::{Error, Result};
pub use models::MemoryHit;
