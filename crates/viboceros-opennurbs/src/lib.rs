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
pub use surface::{SurfaceDefinition, SurfaceQueryError, surface_frames};

mod subd;
pub use subd::{
    SubDData, SubDError, subd_edit_points, subd_from_archive, subd_from_polygons, subd_tessellate,
    subd_transform,
};
