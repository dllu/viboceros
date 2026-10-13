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
    Tessellation, change_curve_degree, change_curve_seam, change_surface_degree,
    change_surface_seam, curve_bezier_spans, curve_tweens, fit_curve, fit_sweep_model_surface,
    insert_curve_knot, insert_surface_knot, interpolate_curve, interpolate_curve_for_command,
    interpolate_open_point_grid, loft_surface, make_curve_non_periodic, make_surface_non_periodic,
    prepare_loft_sections, rebuild_curve, rebuild_surface, remove_curve_knot,
    remove_curve_multiple_knots, remove_surface_knot, remove_surface_multiple_knots, split_curve,
    split_curve_at_parameters, split_surface, subcurve, subcurve_at_arc_length, surface_isocurve,
    surface_single_span_patches, surface_tweens, surface_tweens_refitted, surface_tweens_sampled,
    sweep_model, sweep_surface, trim_curve, trim_surface, trim_surface_direction,
};
