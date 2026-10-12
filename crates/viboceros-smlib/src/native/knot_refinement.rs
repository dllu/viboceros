//! Exact NLib refinement with the original unclamped/periodic definition.
use super::*;
use viboceros_geometry::SurfaceKnotDirection;

unsafe extern "C" {
    fn vb_nurbs_insert_knot(
        source: *const brep::NurbsData,
        axis: c_int,
        parameter: f64,
        target: usize,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Clamp a periodic curve at both active endpoints using the shared native
/// split path. Non-periodic definitions are returned unchanged.
pub fn make_curve_non_periodic(source: &NurbsCurve) -> Result<NurbsCurve, Error> {
    if !source.is_periodic() {
        return Ok(source.clone());
    }
    let domain = source.domain();
    let target = source.degree() + 1;
    let start = insert_curve_knot(source, *domain.start(), target)?;
    insert_curve_knot(&start, *domain.end(), target)
}

/// Clamp only periodic tensor directions selected by the caller. Determine
/// both flags before editing either axis, matching the portable policy.
pub fn make_surface_non_periodic(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
) -> Result<NurbsSurface, Error> {
    let clamp_u = matches!(
        direction,
        SurfaceKnotDirection::U | SurfaceKnotDirection::Both
    ) && source.is_periodic_u();
    let clamp_v = matches!(
        direction,
        SurfaceKnotDirection::V | SurfaceKnotDirection::Both
    ) && source.is_periodic_v();
    let mut result = source.clone();
    for (axis, clamp, domain, target) in [
        (
            SurfaceKnotDirection::U,
            clamp_u,
            source.domain_u(),
            source.degree_u() + 1,
        ),
        (
            SurfaceKnotDirection::V,
            clamp_v,
            source.domain_v(),
            source.degree_v() + 1,
        ),
    ] {
        if clamp {
            result = insert_surface_knot(&result, axis, *domain.start(), target)?;
            result = insert_surface_knot(&result, axis, *domain.end(), target)?;
        }
    }
    Ok(result)
}

pub fn insert_curve_knot(
    source: &NurbsCurve,
    parameter: f64,
    target: usize,
) -> Result<NurbsCurve, Error> {
    let points = source
        .control_points()
        .iter()
        .flat_map(|p| {
            let [x, y, z] = p.point().to_array();
            [x, y, z, p.weight()]
        })
        .collect::<Vec<_>>();
    let view = brep::NurbsData {
        degree: [source.degree(), 0],
        count: [source.control_points().len(), 1],
        knot_count: [source.knots().len(), 0],
        knots: [source.knots().as_ptr(), std::ptr::null()],
        points: points.as_ptr(),
    };
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_insert_knot(&view, 0, parameter, target, &mut pointer, error, capacity)
    })?;
    Ok(source.restore_knot_insertion_topology(
        brep::curve_from_native(pointer)?,
        parameter,
        target,
    )?)
}

pub fn insert_surface_knot(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
    target: usize,
) -> Result<NurbsSurface, Error> {
    let axis = match direction {
        SurfaceKnotDirection::U => 0,
        SurfaceKnotDirection::V => 1,
        SurfaceKnotDirection::Both => {
            return Err(Error::Kernel(
                "Knot refinement requires one parameter direction".into(),
            ));
        }
    };
    let points = source
        .control_points()
        .iter()
        .flat_map(|p| {
            let [x, y, z] = p.point().to_array();
            [x, y, z, p.weight()]
        })
        .collect::<Vec<_>>();
    let view = brep::NurbsData {
        degree: [source.degree_u(), source.degree_v()],
        count: [
            source.control_point_count_u(),
            source.control_point_count_v(),
        ],
        knot_count: [source.knots_u().len(), source.knots_v().len()],
        knots: [source.knots_u().as_ptr(), source.knots_v().as_ptr()],
        points: points.as_ptr(),
    };
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_insert_knot(
            &view,
            axis,
            parameter,
            target,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    Ok(source.restore_knot_insertion_topology(
        brep::surface_from_native(pointer)?,
        direction,
        parameter,
        target,
    )?)
}
