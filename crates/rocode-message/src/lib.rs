#![forbid(unsafe_code)]

//! Message models shared across rocode crates.
//!
//! - `message`: unified (former v2) protocol + provider mapping model.

mod finish;
mod id;
pub mod message;
pub mod part;
pub mod status;
pub mod usage;

pub use rocode_types::Role;
