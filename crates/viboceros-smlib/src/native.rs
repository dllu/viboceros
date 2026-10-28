use std::{
    ffi::{CStr, c_char, c_int},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};
use viboceros_geometry::{
    GeometryError, NurbsCurve, NurbsSurface, Point3, Tolerance, TriangleMesh, Vector3,
    WeightedPoint3,
};
mod brep;
mod mesh_boolean;
pub use mesh_boolean::{mesh_boolean, mesh_boolean_interaction};
mod mesh_split;
pub use mesh_split::{MeshSplitPart, mesh_split, mesh_split_with_face_sources};
mod mesh_intersections;
pub use mesh_intersections::{MeshIntersections, mesh_intersections, mesh_self_intersections};
mod construction;
mod edge_blends;
mod shell;
pub use edge_blends::{ChamferRailType, EdgeBlendRailType};

mod boundary_extension;
mod curve_blend;
pub use boundary_extension::{
    extend_curve_to_boundaries, join_curve_to_boundaries, separate_curve_extensions_to_boundaries,
};
mod curve_connections;
mod curve_distances;
mod curve_extension;
mod curve_fillet_contacts;
mod curve_intersections;
pub use curve_distances::{
    CurveDistanceExtrema, CurveDistanceWitness, curve_distance_extrema, curve_minimum_distance,
    projected_curve_intervals,
};
mod curve_surface_intersections;
mod surface_intersections;
mod surface_pull;
pub use curve_intersections::{curve_intersection_events, curve_self_intersection_events};
pub use curve_surface_intersections::{
    curve_brep_intersection_events, curve_surface_intersection_events,
};
pub use surface_intersections::{
    brep_intersection_events, modeling_surface_intersection_events,
    surface_brep_intersection_events, surface_face_intersection_events,
    surface_intersection_events,
};
pub use surface_pull::{SurfacePullCurve, pull_curve_to_surface};
mod curve_offset_fit;
mod working_curve;
pub use curve_connections::{
    chamfer_curves_joined, chamfer_curves_parts, connect_curves_joined, connect_curves_parts,
    fillet_corners, fillet_curves_joined, fillet_curves_parts,
};
pub use curve_extension::{
    extend_curve, extend_curve_by_length, extend_curve_naturally_by_length, extend_surface,
    extend_surface_by_length, join_curve_natural_extension, join_curve_smooth_extension,
    separate_curve_natural_extensions, separate_curve_smooth_extensions,
};
mod curve_offset;
pub use curve_blend::blend_curve;
pub use curve_offset::{curve_offset_operations, offset_curve_parts, offset_curve_through_point};
mod surface_fillet;
pub use surface_fillet::{
    SurfaceFilletOptions, SurfaceFilletParentMode, SurfaceFilletResult, SurfaceFilletSide,
    fillet_surfaces, fillet_surfaces_with_parents,
};
mod surface_blend;
pub use surface_blend::{blend_surfaces, trim_blend_parent};
mod morph_preview;
pub use morph_preview::{morph_preview_operations, prepare_morph_preview_curve};
#[cfg(test)]
mod curve_edit_point_tests;
mod knot_refinement;
pub use knot_refinement::{
    average_match_curve_ends, change_curve_degree, change_curve_seam, change_surface_degree,
    change_surface_seam, curve_bezier_spans, curve_edit_point_interpolation, curve_tweens,
    fit_cubic_curve_at_stations, fit_cubic_curve_with_constraints, fit_curve,
    fit_sweep_model_surface, insert_curve_knot, insert_surface_knot, interpolate_affine_greville,
    interpolate_curve, interpolate_curve_for_command, interpolate_endpoints,
    interpolate_open_point_grid, interpolate_point_grid, interpolate_surface_blend, loft_surface,
    loft_surface_rebuilt, loft_surface_refitted, loft_surface_refitted_with_angle,
    loft_surface_with_end_points, make_curve_non_periodic, make_curve_periodic,
    make_surface_non_periodic, make_surface_periodic, match_curve_end, morph_interpolation,
    point_grid_brep, prepare_loft_sections, rebuild_curve, rebuild_surface, rebuild_sweep_sections,
    reduce_curve_knots_in_range, refit_sweep_sections, remove_curve_knot,
    remove_curve_knot_at_value, remove_curve_multiple_knots, remove_periodic_surface_knot,
    remove_surface_knot, remove_surface_multiple_knots, split_curve, split_curve_at_parameters,
    split_surface, subcurve, subcurve_at_arc_length, surface_isocurve, surface_single_span_patches,
    surface_tweens, surface_tweens_refitted, surface_tweens_sampled, sweep_model,
    sweep_model_with_point_ends, sweep_surface, trim_curve, trim_surface, trim_surface_direction,
};
mod coplanar_merge;
mod face_creases;
mod shape;
mod silhouette;
mod surface_extrusion;
mod two_rail_sweep;
pub use two_rail_sweep::two_rail_sweep_surface;
mod network_surface;
pub use network_surface::{
    FittedGordonNetwork, GordonCurveFamilyFit, GordonCurveFitClock, GordonCurveFitInput,
    GordonNetworkSourceImage, fit_gordon_curve_family, fit_gordon_curve_family_with_clock,
    fitted_gordon_network_surface, gordon_network_surface, prepared_gordon_surface,
};
mod rail_revolve;
pub use rail_revolve::{rail_revolved_brep, rail_revolved_surface};
mod tapered;
pub use shape::StitchResult;

/// A native manifold sheet or solid, owned independently of a document.
/// Sheet handles cannot be passed to the closed-solid Boolean interface.
pub struct Shape {
    model: Solid,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SMLib: {0}")]
    Kernel(String),
    #[error("SMLib returned inconsistent data")]
    InvalidOutput,
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

#[repr(C)]
struct NativeSolid {
    _private: [u8; 0],
}
#[repr(C)]
struct NativeCurve {
    _private: [u8; 0],
}
#[repr(C)]
struct NativeMesh {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vb_solid_primitive(
        kind: c_int,
        origin: *const f64,
        size: *const f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_boolean(
        a: *const NativeSolid,
        b: *const NativeSolid,
        operation: c_int,
        options: c_int,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_properties(
        solid: *const NativeSolid,
        accuracy: f64,
        volume: *mut f64,
        bounds: *mut f64,
        manifold: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_volume(
        solid: *const NativeSolid,
        accuracy: f64,
        volume: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_free(solid: *mut NativeSolid);
    fn vb_solid_mesh(
        solid: *const NativeSolid,
        quality: *const f64,
        out: *mut *mut NativeMesh,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_mesh_sizes(mesh: *const NativeMesh, vertices: *mut usize, triangles: *mut usize)
    -> c_int;
    fn vb_mesh_copy(
        mesh: *const NativeMesh,
        vertices: *mut f64,
        vertex_count: usize,
        triangles: *mut u32,
        triangle_count: usize,
    ) -> c_int;
    fn vb_mesh_free(mesh: *mut NativeMesh);
    fn vb_curve_create(
        degree: usize,
        points: *const f64,
        point_count: usize,
        knots: *const f64,
        knot_count: usize,
        out: *mut *mut NativeCurve,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_evaluate(
        curve: *const NativeCurve,
        parameter: f64,
        point: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_sizes(
        curve: *const NativeCurve,
        degree: *mut usize,
        points: *mut usize,
        knots: *mut usize,
    ) -> c_int;
    fn vb_curve_copy(
        curve: *const NativeCurve,
        points: *mut f64,
        point_count: usize,
        knots: *mut f64,
        knot_count: usize,
    ) -> c_int;
    fn vb_curve_free(curve: *mut NativeCurve);
}

fn call(function: impl FnOnce(*mut c_char, usize) -> c_int) -> Result<(), Error> {
    let mut error = [0 as c_char; 512];
    if function(error.as_mut_ptr(), error.len()) == 0 {
        return Ok(());
    }
    // Every error buffer starts zeroed; the C boundary always writes a terminated message.
    Err(Error::Kernel(
        unsafe { CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    ))
}

/// An exact native solid, owned independently of any document.
///
/// ```compile_fail
/// # use viboceros_smlib::Solid;
/// fn must_send<T: Send>() {}
/// must_send::<Solid>();
/// ```
pub struct Solid {
    handle: NonNull<NativeSolid>,
    _thread_local: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug)]
pub enum BooleanOperation {
    Union,
    Intersection,
    Difference,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolidProperties {
    pub volume: f64,
    pub bounds: [Point3; 2],
    pub manifold: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Tessellation {
    pub chord_height: f64,
    pub curve_angle_degrees: f64,
    pub surface_angle_degrees: f64,
}
impl Default for Tessellation {
    fn default() -> Self {
        Self {
            chord_height: 0.005,
            curve_angle_degrees: 15.,
            surface_angle_degrees: 15.,
        }
    }
}

impl Solid {
    fn primitive(kind: c_int, origin: Point3, size: [f64; 3]) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        let origin = origin.to_array();
        // The input arrays remain live; native code returns a unique owned handle.
        call(|error, capacity| unsafe {
            vb_solid_primitive(
                kind,
                origin.as_ptr(),
                size.as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }
    pub fn box_solid(origin: Point3, dimensions: [f64; 3]) -> Result<Self, Error> {
        Self::primitive(0, origin, dimensions)
    }
    pub fn sphere(center: Point3, radius: f64) -> Result<Self, Error> {
        Self::primitive(1, center, [radius, 0., 0.])
    }
    pub fn cylinder(base: Point3, radius: f64, height: f64) -> Result<Self, Error> {
        Self::primitive(2, base, [radius, height, 0.])
    }

    /// Returns a new result, preserving both inputs on success and failure.
    pub fn boolean(&self, other: &Self, operation: BooleanOperation) -> Result<Self, Error> {
        self.boolean_with_postprocess(other, operation, true)
    }

    /// Perform a copied-operand Boolean with optional kernel topology cleanup.
    /// Disabling cleanup retains imprinted seams; both inputs stay unchanged.
    pub fn boolean_with_postprocess(
        &self,
        other: &Self,
        operation: BooleanOperation,
        post_process: bool,
    ) -> Result<Self, Error> {
        self.boolean_options(other, operation, post_process, false)
    }

    /// Check the original boundary against another solid's material using the
    /// kernel's cookie-cutter operation. This is distinct from volume coverage:
    /// the boundary can surround an enclosed void while lying in material.
    pub fn boundary_is_covered_by(&self, other: &Self) -> Result<bool, Error> {
        self.boolean_options(other, BooleanOperation::Difference, true, true)?
            .is_empty()
    }

    fn boolean_options(
        &self,
        other: &Self,
        operation: BooleanOperation,
        post_process: bool,
        cookie_cutter: bool,
    ) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        let operation = match operation {
            BooleanOperation::Union => 0,
            BooleanOperation::Intersection => 1,
            BooleanOperation::Difference => 2,
        };
        // Both handles stay borrowed; the native wrapper copies before consuming operands.
        call(|error, capacity| unsafe {
            vb_solid_boolean(
                self.handle.as_ptr(),
                other.handle.as_ptr(),
                operation,
                c_int::from(post_process) | (c_int::from(cookie_cutter) << 1),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }

    /// Compute material volume in the kernel without exporting geometry or integrating moments.
    /// Empty Boolean results have zero volume. The requested relative accuracy
    /// must be between 1e-12 and 0.1; it controls numerical integration rather
    /// than providing a certified error bound.
    pub fn volume(&self, relative_accuracy: f64) -> Result<f64, Error> {
        let mut volume = 0.;
        call(|error, capacity| unsafe {
            vb_solid_volume(
                self.handle.as_ptr(),
                relative_accuracy,
                &mut volume,
                error,
                capacity,
            )
        })?;
        Ok(volume)
    }

    pub fn properties(&self, relative_accuracy: f64) -> Result<SolidProperties, Error> {
        let mut volume = 0.;
        let mut bounds = [0.; 6];
        let mut manifold = 0;
        call(|error, capacity| unsafe {
            vb_solid_properties(
                self.handle.as_ptr(),
                relative_accuracy,
                &mut volume,
                bounds.as_mut_ptr(),
                &mut manifold,
                error,
                capacity,
            )
        })?;
        Ok(SolidProperties {
            volume,
            bounds: [
                Point3::try_new(bounds[0], bounds[1], bounds[2])?,
                Point3::try_new(bounds[3], bounds[4], bounds[5])?,
            ],
            manifold: manifold != 0,
        })
    }

    /// Converts a complete triangular tessellation into validated Rust mesh data.
    pub fn tessellate(
        &self,
        quality: Tessellation,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, Error> {
        let quality = [
            quality.chord_height,
            quality.curve_angle_degrees,
            quality.surface_angle_degrees,
        ];
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_solid_mesh(
                self.handle.as_ptr(),
                quality.as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        OwnedMesh(NonNull::new(pointer).ok_or(Error::InvalidOutput)?).decode(tolerance)
    }
}

impl OwnedMesh {
    fn decode(&self, tolerance: Tolerance) -> Result<TriangleMesh, Error> {
        let mut vertex_count = 0;
        let mut triangle_count = 0;
        if unsafe { vb_mesh_sizes(self.0.as_ptr(), &mut vertex_count, &mut triangle_count) } != 0
            || vertex_count > 1_000_000
            || triangle_count > 1_000_000
        {
            return Err(Error::InvalidOutput);
        }
        let mut points = vec![0.; vertex_count * 3];
        let mut triangles = vec![0; triangle_count * 3];
        if unsafe {
            vb_mesh_copy(
                self.0.as_ptr(),
                points.as_mut_ptr(),
                vertex_count,
                triangles.as_mut_ptr(),
                triangle_count,
            )
        } != 0
        {
            return Err(Error::InvalidOutput);
        }
        let points = points
            .chunks_exact(3)
            .map(|p| Point3::try_new(p[0], p[1], p[2]))
            .collect::<Result<Vec<_>, _>>()?;
        let triangles = triangles
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        Ok(TriangleMesh::try_new(points, triangles, tolerance)?)
    }
}
impl Drop for Solid {
    fn drop(&mut self) {
        unsafe { vb_solid_free(self.handle.as_ptr()) };
    }
}
struct OwnedMesh(NonNull<NativeMesh>);
impl Drop for OwnedMesh {
    fn drop(&mut self) {
        unsafe { vb_mesh_free(self.0.as_ptr()) };
    }
}

/// Native clamped NURBS with positive weights, transferred without fitting.
/// Canonical import accepts full unclamped data and clamps its active interval.
/// The pinned kernel supports degree 17 and unequal canonical weights in
/// [0.001, 100]. Polynomial imports omit irrelevant equal-weight gauges.
pub struct KernelCurve {
    handle: NonNull<NativeCurve>,
    _thread_local: PhantomData<Rc<()>>,
}
impl KernelCurve {
    pub fn from_nurbs(curve: &NurbsCurve) -> Result<Self, Error> {
        let points = curve
            .control_points()
            .iter()
            .flat_map(|p| {
                let xyz = p.point().to_array();
                [xyz[0], xyz[1], xyz[2], p.weight()]
            })
            .collect::<Vec<_>>();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_curve_create(
                curve.degree(),
                points.as_ptr(),
                curve.control_points().len(),
                curve.knots().as_ptr(),
                curve.knots().len(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }
    pub fn evaluate(&self, parameter: f64) -> Result<Point3, Error> {
        let mut point = [0.; 3];
        call(|error, capacity| unsafe {
            vb_curve_evaluate(
                self.handle.as_ptr(),
                parameter,
                point.as_mut_ptr(),
                error,
                capacity,
            )
        })?;
        Ok(Point3::try_new(point[0], point[1], point[2])?)
    }
    pub fn to_nurbs(&self) -> Result<NurbsCurve, Error> {
        let mut degree = 0;
        let mut point_count = 0;
        let mut knot_count = 0;
        if unsafe {
            vb_curve_sizes(
                self.handle.as_ptr(),
                &mut degree,
                &mut point_count,
                &mut knot_count,
            )
        } != 0
            || point_count > 1_000_000
            || degree > 64
            || knot_count > 1_000_065
        {
            return Err(Error::InvalidOutput);
        }
        let mut points = vec![0.; point_count * 4];
        let mut knots = vec![0.; knot_count];
        if unsafe {
            vb_curve_copy(
                self.handle.as_ptr(),
                points.as_mut_ptr(),
                point_count,
                knots.as_mut_ptr(),
                knot_count,
            )
        } != 0
        {
            return Err(Error::InvalidOutput);
        }
        let points = points
            .chunks_exact(4)
            .map(|p| WeightedPoint3::try_new(Point3::try_new(p[0], p[1], p[2])?, p[3]))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(NurbsCurve::try_new_rational(degree, points, knots)?)
    }
}
impl Drop for KernelCurve {
    fn drop(&mut self) {
        unsafe { vb_curve_free(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_boundary_clears_failed_outputs_and_terminates_small_error_buffers() {
        let origin = [0.; 3];
        let size = [1.; 3];
        let mut output = NonNull::<NativeSolid>::dangling().as_ptr();
        let mut error = [42 as c_char; 1];
        let status = unsafe {
            vb_solid_primitive(
                99,
                origin.as_ptr(),
                size.as_ptr(),
                &mut output,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert_ne!(status, 0);
        assert!(output.is_null());
        assert_eq!(error, [0]);
        let status = unsafe {
            vb_solid_primitive(
                0,
                std::ptr::null(),
                size.as_ptr(),
                &mut output,
                std::ptr::null_mut(),
                0,
            )
        };
        assert_ne!(status, 0);
        assert!(output.is_null());
    }
}
