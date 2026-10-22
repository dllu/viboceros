//! Shared OpenNURBS library, owned mesh operations, and curve queries.
//! File-format APIs remain in viboceros-io; this crate has no geometry dependency.
mod mesh;
pub use mesh::*;
mod curve;
pub use curve::{
    ArcDefinition, CurveQueryError, arc_through_points, curve_plane_normal,
    transform_analytic_circle,
};

pub use curve::{
    AnalyticCurve, curve_analytic_form, curve_polyline_vertices, polyline_simplify_indices,
};

mod surface;
pub use surface::{SurfaceDefinition, SurfaceQueryError, surface_discontinuities, surface_frames};

mod subd;
pub use subd::{
    SubDControlNetPriority, SubDData, SubDError, subd_append, subd_box, subd_chain_sharpness,
    subd_control_mesh, subd_control_mesh_with_priority, subd_crease_components, subd_cylinder,
    subd_edge_chains, subd_edge_samples, subd_edit_points, subd_from_archive, subd_from_mesh,
    subd_from_polygons, subd_set_tags, subd_sharpness_from_percent, subd_soft_crease,
    subd_stored_sharpness, subd_subdivide, subd_tessellate, subd_tessellate_with_normals,
    subd_transform,
};

mod subd_interpolation;
pub use subd_interpolation::{SubDWeights, subd_interpolation_weights};

mod extrusion;
pub use extrusion::{
    ExtrusionData, ExtrusionError, ExtrusionProfile, ExtrusionSurfaceData, extrusion_create,
    extrusion_create_command, extrusion_read, extrusion_surface, extrusion_transform,
};
