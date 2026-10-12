//! Optional native USD BRep/SMLib adapter. Enable `native` on Linux.
//!
//! Kernel handles are thread-local; native calls are serialized process-wide.
//! Boolean operations copy their operands before mutation. Native solids can
//! import and export owned NURBS topology with validated Rust B-reps. The optional
//! application feature uses this adapter for closed curved BooleanDifference,
//! BooleanUnion, BooleanIntersection and BooleanSplit, surface joining/offsets,
//! and native curve extrusion.
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::{
    BooleanOperation, Error, KernelCurve, Shape, Solid, SolidProperties, StitchResult,
    Tessellation, change_curve_degree, change_surface_degree, insert_curve_knot,
    insert_surface_knot, make_curve_non_periodic, make_surface_non_periodic,
};
