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
    BooleanOperation, ChamferRailType, EdgeBlendRailType, Error, KernelCurve, Shape, Solid,
    SolidProperties, StitchResult, SurfacePullCurve, Tessellation, average_match_curve_ends,
    blend_curve, blend_surfaces, brep_intersection_events, chamfer_curves_joined,
    chamfer_curves_parts, change_curve_degree, change_curve_seam, change_surface_degree,
    change_surface_seam, connect_curves_joined, connect_curves_parts, curve_bezier_spans,
    curve_brep_intersection_events, curve_edit_point_interpolation, curve_intersection_events,
    curve_offset_operations, curve_self_intersection_events, curve_surface_intersection_events,
    curve_tweens, extend_curve, extend_curve_by_length, extend_curve_naturally_by_length,
    extend_curve_to_boundaries, extend_surface, extend_surface_by_length, fillet_corners,
    fillet_curves_joined, fillet_curves_parts, fit_cubic_curve_at_stations,
    fit_cubic_curve_with_constraints, fit_curve, fit_sweep_model_surface, gordon_network_surface,
    insert_curve_knot, insert_surface_knot, interpolate_affine_greville, interpolate_curve,
    interpolate_curve_for_command, interpolate_endpoints, interpolate_open_point_grid,
    interpolate_point_grid, interpolate_surface_blend, join_curve_natural_extension,
    join_curve_smooth_extension, join_curve_to_boundaries, loft_surface, make_curve_non_periodic,
    make_curve_periodic, make_surface_non_periodic, make_surface_periodic, match_curve_end,
    modeling_surface_intersection_events, morph_interpolation, morph_preview_operations,
    offset_curve_parts, offset_curve_through_point, point_grid_brep, prepare_loft_sections,
    prepare_morph_preview_curve, prepared_gordon_surface, pull_curve_to_surface,
    rail_revolved_brep, rail_revolved_surface, rebuild_curve, rebuild_surface, remove_curve_knot,
    remove_curve_multiple_knots, remove_surface_knot, remove_surface_multiple_knots,
    separate_curve_extensions_to_boundaries, separate_curve_natural_extensions,
    separate_curve_smooth_extensions, split_curve, split_curve_at_parameters, split_surface,
    subcurve, subcurve_at_arc_length, surface_brep_intersection_events,
    surface_face_intersection_events, surface_intersection_events, surface_isocurve,
    surface_single_span_patches, surface_tweens, surface_tweens_refitted, surface_tweens_sampled,
    sweep_model, sweep_surface, trim_blend_parent, trim_curve, trim_surface,
    trim_surface_direction, two_rail_sweep_surface,
};
