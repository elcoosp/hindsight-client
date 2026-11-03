//! Wire models for the Hindsight HTTP API (spec 1.1.0).
//!
//! All types serialize with `snake_case` field names matching the API, omit
//! `None` fields from payloads (`skip_serializing_if`), and tolerate unknown
//! fields in responses (forward compatibility with newer servers).

pub mod audit;
pub mod bank;
pub mod common;
pub mod directive;
pub mod document;
pub mod entity;
pub mod knowledge;
pub mod memory;
pub mod mental_model;
pub mod operation;
pub mod reflect;
pub mod transfer;
pub mod webhook;

pub use audit::*;
pub use bank::*;
pub use common::*;
pub use directive::*;
pub use document::*;
pub use entity::*;
pub use knowledge::*;
pub use memory::*;
pub use mental_model::*;
pub use operation::*;
pub use reflect::*;
pub use transfer::*;
pub use webhook::*;
