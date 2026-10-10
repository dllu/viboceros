//! Optional native USD BRep/SMLib adapter. Enable `native` on Linux.
//!
//! Kernel handles are thread-local; native calls are serialized process-wide.
//! Boolean operations copy their operands before mutation. Native solids can
//! export owned NURBS topology into validated Rust B-reps. Rust-to-native import
//! and application command integration remain in progress.
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::{BooleanOperation, Error, KernelCurve, Solid, SolidProperties, Tessellation};
