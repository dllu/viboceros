//! Optional native USD BRep/SMLib adapter. Enable `native` on Linux.
//!
//! Kernel handles are thread-local; native calls are serialized process-wide.
//! Boolean operations copy their operands before mutation. Native solids can
//! import and export owned NURBS topology with validated Rust B-reps. The optional
//! application feature uses this adapter for curved BooleanDifference.
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::{BooleanOperation, Error, KernelCurve, Solid, SolidProperties, Tessellation};
