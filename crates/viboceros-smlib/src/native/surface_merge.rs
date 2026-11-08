//! Certified boundary admission and public SMLib support concatenation.
use super::*;
use viboceros_geometry::SurfaceKnotDirection;

unsafe extern "C" {
    fn vb_nurbs_join_surfaces(
        first: *const brep::NurbsData,
        second: *const brep::NurbsData,
        axis: c_int,
        output: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceMergeOptions {
    pub tolerance: f64,
    pub smooth: bool,
    pub roundness: f64,
}

/// Merge the first maximum U/V boundary with the second minimum U/V boundary.
/// Boundary traversal must agree. The complete boundary curves must satisfy an
/// affine parameter correspondence within `tolerance`; unsupported certificates
/// decline. The other parameter direction follows the first support's domain.
/// Inputs remain unchanged, including on native failure.
///
/// Keep the seam's full degree multiplicity for a sharp merge. Smoothing moves
/// its two outer knots toward the neighboring distinct knots by half the
/// roundness. Controls stay fixed, so smoothing deliberately changes the shape;
/// tolerance admits the input boundaries, not that shape change. This primitive
/// does not choose edges, trim surfaces, or create B-rep/document topology.
/// Homogeneous seam controls are averaged without normalizing operand weight
/// scales, matching the public SDK: changing one scale can change the result.
pub fn merge_surface_supports(
    first: &NurbsSurface,
    second: &NurbsSurface,
    direction: SurfaceKnotDirection,
    options: SurfaceMergeOptions,
) -> Result<NurbsSurface, Error> {
    if !options.tolerance.is_finite()
        || options.tolerance < 0.
        || !options.roundness.is_finite()
        || !(0. ..=1.).contains(&options.roundness)
    {
        return Err(Error::Kernel("Invalid surface merge options".into()));
    }
    let (axis, seam, a, b, second) = match direction {
        SurfaceKnotDirection::U => (
            0,
            *first.domain_u().end(),
            first.isocurve_v(*first.domain_u().end())?,
            second.isocurve_v(*second.domain_u().start())?,
            second.try_reparameterized(second.domain_u(), first.domain_v())?,
        ),
        SurfaceKnotDirection::V => (
            1,
            *first.domain_v().end(),
            first.isocurve_u(*first.domain_v().end())?,
            second.isocurve_u(*second.domain_v().start())?,
            second.try_reparameterized(first.domain_u(), second.domain_v())?,
        ),
        SurfaceKnotDirection::Both => {
            return Err(Error::Kernel(
                "Surface merging requires one parameter direction".into(),
            ));
        }
    };
    if a.parameter_curve_deviation_bound(&b, options.tolerance)?
        .is_none()
    {
        return Err(Error::Kernel(
            "Surface boundaries lack a complete join certificate".into(),
        ));
    }
    let a = knot_refinement::NurbsInput::surface(first);
    let b = knot_refinement::NurbsInput::surface(&second);
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_join_surfaces(&a.view(), &b.view(), axis, &mut pointer, error, capacity)
    })?;
    let mut joined = brep::surface_from_native(pointer)?;
    let (degree, knots) = if axis == 0 {
        (joined.degree_u(), joined.knots_u())
    } else {
        (joined.degree_v(), joined.knots_v())
    };
    // SMLib may remove an exactly redundant seam. Restore its original basis
    // before the separately measured sharp/smooth policy is applied.
    if knots.iter().filter(|&&t| t == seam).count() < degree {
        joined = insert_surface_knot(&joined, direction, seam, degree)?;
    }
    if !options.smooth || options.roundness == 0. || degree < 2 {
        return Ok(joined);
    }
    let mut knots = if axis == 0 {
        joined.knots_u().to_vec()
    } else {
        joined.knots_v().to_vec()
    };
    let start = knots.partition_point(|&t| t < seam);
    let end = knots.partition_point(|&t| t <= seam);
    if start == 0 || end == knots.len() || end - start != degree {
        return Err(Error::InvalidOutput);
    }
    knots[start] += (knots[start - 1] - seam) * (options.roundness * 0.5);
    knots[end - 1] += (knots[end] - seam) * (options.roundness * 0.5);
    let (knots_u, knots_v) = if axis == 0 {
        (knots, joined.knots_v().to_vec())
    } else {
        (joined.knots_u().to_vec(), knots)
    };
    Ok(NurbsSurface::try_new_rational(
        joined.degree_u(),
        joined.degree_v(),
        joined.control_point_count_u(),
        joined.control_point_count_v(),
        joined.control_points().to_vec(),
        knots_u,
        knots_v,
    )?)
}
