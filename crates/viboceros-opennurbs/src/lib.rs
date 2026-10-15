//! Shared OpenNURBS native library and owned mesh modeling adapters.
//! File-format APIs remain in viboceros-io; this crate has no geometry dependency.
mod mesh;
pub use mesh::*;
