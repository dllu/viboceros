//! Shared OpenNURBS library, owned mesh operations, and curve queries.
//! File-format APIs remain in viboceros-io; this crate has no geometry dependency.
mod mesh;
pub use mesh::*;
mod curve;
pub use curve::{CurveQueryError, curve_plane_normal};
