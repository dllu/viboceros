//! Optional native USD BRep/SMLib adapter. Enable `native` on Linux.
//!
//! Kernel handles are thread-local; native calls are serialized process-wide.
//! Boolean operations copy their operands before mutation. This adapter does
//! not yet transfer complete exact B-rep topology into Viboceros documents.
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::{BooleanOperation, Error, KernelCurve, Solid, SolidProperties, Tessellation};
