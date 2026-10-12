//! NLib refinement and basis editing with complete source definitions.
use super::*;
use viboceros_geometry::SurfaceKnotDirection;

unsafe extern "C" {
    fn vb_nurbs_fit_basis(
        source: *const brep::NurbsData,
        axis: c_int,
        knots: *const f64,
        knot_count: usize,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_nurbs_change_degree(
        source: *const brep::NurbsData,
        axis: c_int,
        degree: usize,
        deformable: c_int,
        knots: *const f64,
        knot_count: usize,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
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

fn clamp_curve(source: &NurbsCurve) -> Result<NurbsCurve, Error> {
    let domain = source.domain();
    let target = source.degree() + 1;
    let result = insert_curve_knot(source, *domain.start(), target)?;
    insert_curve_knot(&result, *domain.end(), target)
}

fn clamp_surface_direction(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
) -> Result<NurbsSurface, Error> {
    let (domain, degree) = match direction {
        SurfaceKnotDirection::U => (source.domain_u(), source.degree_u()),
        SurfaceKnotDirection::V => (source.domain_v(), source.degree_v()),
        SurfaceKnotDirection::Both => {
            return Err(Error::Kernel(
                "Basis editing requires one parameter direction".into(),
            ));
        }
    };
    let result = insert_surface_knot(source, direction, *domain.start(), degree + 1)?;
    insert_surface_knot(&result, direction, *domain.end(), degree + 1)
}

fn fit_basis(
    input: NurbsInput<'_>,
    axis: c_int,
    knots: &[f64],
) -> Result<*mut brep::NativeSurfaceData, Error> {
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_fit_basis(
            &view,
            axis,
            knots.as_ptr(),
            knots.len(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    Ok(pointer)
}

/// Remove the knot chosen by the existing model-space command policy, fitting
/// the replacement homogeneous controls with the shared native basis editor.
pub fn remove_curve_knot(source: &NurbsCurve, parameter: f64) -> Result<NurbsCurve, Error> {
    let knot = source.knot_removal_candidate(parameter)?;
    let clamped = clamp_curve(source)?;
    let knots = viboceros_geometry::knot_removal_knots(clamped.degree(), clamped.knots(), knot)?;
    brep::curve_from_native(fit_basis(NurbsInput::curve(&clamped), 0, &knots)?)
}

/// Remove one knot in the requested tensor direction. Selection uses parameter
/// distance and the complete net's periodic state, rather than an individual row.
pub fn remove_surface_knot(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<NurbsSurface, Error> {
    let knot = source.knot_removal_candidate(direction, parameter)?;
    let clamped = clamp_surface_direction(source, direction)?;
    let (axis, degree, before) = match direction {
        SurfaceKnotDirection::U => (0, clamped.degree_u(), clamped.knots_u()),
        SurfaceKnotDirection::V => (1, clamped.degree_v(), clamped.knots_v()),
        SurfaceKnotDirection::Both => unreachable!("candidate requires one parameter direction"),
    };
    let knots = viboceros_geometry::knot_removal_knots(degree, before, knot)?;
    brep::surface_from_native(fit_basis(NurbsInput::surface(&clamped), axis, &knots)?)
}

struct NurbsInput<'a> {
    degree: [usize; 2],
    count: [usize; 2],
    knots: [&'a [f64]; 2],
    points: Vec<f64>,
}
impl<'a> NurbsInput<'a> {
    fn new(
        degree: [usize; 2],
        count: [usize; 2],
        knots: [&'a [f64]; 2],
        controls: &[WeightedPoint3],
    ) -> Self {
        Self {
            degree,
            count,
            knots,
            points: controls
                .iter()
                .flat_map(|p| {
                    let [x, y, z] = p.point().to_array();
                    [x, y, z, p.weight()]
                })
                .collect(),
        }
    }
    fn curve(source: &'a NurbsCurve) -> Self {
        Self::new(
            [source.degree(), 0],
            [source.control_points().len(), 1],
            [source.knots(), &[]],
            source.control_points(),
        )
    }
    fn surface(source: &'a NurbsSurface) -> Self {
        Self::new(
            [source.degree_u(), source.degree_v()],
            [
                source.control_point_count_u(),
                source.control_point_count_v(),
            ],
            [source.knots_u(), source.knots_v()],
            source.control_points(),
        )
    }
    fn view(&self) -> brep::NurbsData {
        brep::NurbsData {
            degree: self.degree,
            count: self.count,
            knot_count: self.knots.map(|k| k.len()),
            knots: self.knots.map(|k| k.as_ptr()),
            points: self.points.as_ptr(),
        }
    }
}

/// Native exact elevation or homogeneous interpolation in the existing
/// application target basis. Degree changes clamp the active source interval.
pub fn change_curve_degree(
    source: &NurbsCurve,
    degree: usize,
    deformable: bool,
) -> Result<NurbsCurve, Error> {
    if degree == 0 {
        return Err(GeometryError::InvalidDegree.into());
    }
    if degree == source.degree() {
        return Ok(source.clone());
    }
    let clamped = clamp_curve(source)?;
    let knots = viboceros_geometry::degree_change_knots(
        clamped.degree(),
        degree,
        deformable,
        clamped.knots(),
    )?;
    let input = NurbsInput::curve(&clamped);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_change_degree(
            &view,
            0,
            degree,
            c_int::from(deformable),
            knots.as_ptr(),
            knots.len(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    brep::curve_from_native(pointer)
}

pub fn change_surface_degree(
    source: &NurbsSurface,
    u: usize,
    v: usize,
    deformable: bool,
) -> Result<NurbsSurface, Error> {
    if u == 0 || v == 0 {
        return Err(GeometryError::InvalidDegree.into());
    }
    let mut result = source.clone();
    for (axis, degree) in [(0, u), (1, v)] {
        let old = if axis == 0 {
            result.degree_u()
        } else {
            result.degree_v()
        };
        if old == degree {
            continue;
        }
        let direction = if axis == 0 {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        };
        result = clamp_surface_direction(&result, direction)?;
        let before = if axis == 0 {
            result.knots_u()
        } else {
            result.knots_v()
        };
        let knots = viboceros_geometry::degree_change_knots(old, degree, deformable, before)?;
        let input = NurbsInput::surface(&result);
        let view = input.view();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_nurbs_change_degree(
                &view,
                axis,
                degree,
                c_int::from(deformable),
                knots.as_ptr(),
                knots.len(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        result = brep::surface_from_native(pointer)?;
    }
    Ok(result)
}

/// Clamp a periodic curve at both active endpoints using the shared native
/// split path. Non-periodic definitions are returned unchanged.
pub fn make_curve_non_periodic(source: &NurbsCurve) -> Result<NurbsCurve, Error> {
    if !source.is_periodic() {
        return Ok(source.clone());
    }
    clamp_curve(source)
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
    for (axis, clamp) in [
        (SurfaceKnotDirection::U, clamp_u),
        (SurfaceKnotDirection::V, clamp_v),
    ] {
        if clamp {
            result = clamp_surface_direction(&result, axis)?;
        }
    }
    Ok(result)
}

pub fn insert_curve_knot(
    source: &NurbsCurve,
    parameter: f64,
    target: usize,
) -> Result<NurbsCurve, Error> {
    let input = NurbsInput::curve(source);
    let view = input.view();
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
    let input = NurbsInput::surface(source);
    let view = input.view();
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
