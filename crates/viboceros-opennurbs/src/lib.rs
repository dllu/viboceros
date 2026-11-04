//! Shared OpenNURBS library, owned mesh operations, and curve queries.
//! File-format APIs remain in viboceros-io; this crate has no geometry dependency.
mod mesh;
pub use mesh::*;
mod curve;
pub use curve::{
    ArcDefinition, CurveContinuity, CurveParametricContinuity, CurveQueryError, arc_through_points,
    curve_continuity_breaks, curve_continuity_breaks_with_curvature, curve_discontinuities,
    curve_parametric_continuity_breaks, curve_plane_normal, curve_speeds, curve_tangents,
    transform_analytic_circle,
};

pub use curve::{
    AnalyticCurve, curve_analytic_form, curve_polyline_vertices, polyline_simplify_indices,
};

mod surface;
pub use surface::{
    SurfaceDefinition, SurfaceQueryError, surface_continuity_breaks, surface_control_polygon_mesh,
    surface_discontinuities, surface_frames,
};

mod subd;
pub use subd::{
    SubDControlNetPriority, SubDData, SubDError, SubDExtrudeDirection, SubDFaceGrid, SubDFacePack,
    SubDPartialPatchStencil, SubDPatchStencil, SubDRegularPatch, SubDSharpnessRefinement,
    subd_append, subd_box, subd_chain_sharpness, subd_control_mesh,
    subd_control_mesh_with_priority, subd_crease_components, subd_cylinder, subd_edge_chains,
    subd_edge_samples, subd_edit_points, subd_extrude, subd_face_grids, subd_face_packs,
    subd_from_archive, subd_from_mesh, subd_from_polygons, subd_insert_face_edge, subd_insert_path,
    subd_insert_vertex, subd_inset, subd_join_assemble, subd_offset, subd_partial_patch_stencils,
    subd_patch_stencils, subd_refined_face_grids, subd_regular_patches, subd_remap_vertices,
    subd_retain_faces, subd_set_face_packs, subd_set_tags, subd_sharpness_from_percent,
    subd_sharpness_refinement, subd_smooth_vertex_normals, subd_soft_crease, subd_spin_edge,
    subd_stored_sharpness, subd_subdivide, subd_tessellate, subd_tessellate_with_jets,
    subd_tessellate_with_normals, subd_transform,
};

mod subd_interpolation;
pub use subd_interpolation::{SubDWeights, subd_interpolation_weights};

mod extrusion;
pub use extrusion::{
    ExtrusionData, ExtrusionError, ExtrusionProfile, ExtrusionSurfaceData, extrusion_create,
    extrusion_create_command, extrusion_read, extrusion_surface, extrusion_transform,
};
