//! Native isocurves, trimming, splitting and Bezier extraction.
use super::*;

/// Extract an exact isocurve with the complete varying-axis basis and gauge.
pub fn surface_isocurve(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<NurbsCurve, Error> {
    source.isocurve_with_extractor(
        direction,
        parameter,
        &mut |surface, direction, parameter| {
            let input = NurbsInput::surface(surface);
            let view = input.view();
            let mut pointer = std::ptr::null_mut();
            let varying = if direction == SurfaceKnotDirection::U {
                0
            } else {
                1
            };
            call(|error, capacity| unsafe {
                vb_nurbs_surface_isocurve(&view, varying, parameter, &mut pointer, error, capacity)
            })?;
            brep::curve_from_native(pointer)
        },
    )
}

/// Trim an exact parameter interval, retaining the original chart and gauge.
pub fn trim_curve(
    source: &NurbsCurve,
    interval: std::ops::RangeInclusive<f64>,
) -> Result<NurbsCurve, Error> {
    viboceros_geometry::validate_curve_trim_interval(&interval, source.domain())?;
    // The shared exact affine path preserves zero crossings needed by
    // intersection topology; a native floating ratio can round them away.
    if interval == source.domain()
        || (source.degree() == 1
            && source.control_points().len() == 2
            && source.control_points()[0].weight() == source.control_points()[1].weight())
    {
        return Ok(source.try_trimmed(interval)?);
    }
    let input = NurbsInput::curve(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_trim_curve(
            &view,
            *interval.start(),
            *interval.end(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    brep::curve_from_native(pointer)
}

/// Extract a directed NURBS subcurve, reusing the shared reversal/seam policy.
pub fn subcurve(source: &NurbsCurve, start: f64, end: f64) -> Result<NurbsCurve, Error> {
    source.try_subcurve_with_extractor(start, end, &mut trim_curve)
}

/// Anchor-relative signed-length extraction using the shared sampler and
/// native NURBS trimming, retaining analytic and composite leaf classes.
pub fn subcurve_at_arc_length(
    source: &viboceros_geometry::Curve3,
    anchor: f64,
    length: f64,
    tolerance: Tolerance,
) -> Result<Option<(viboceros_geometry::Curve3, f64)>, Error> {
    source.try_subcurve_at_arc_length_with_operations(
        anchor,
        length,
        tolerance,
        &mut trim_curve,
        &mut change_curve_seam,
    )
}

/// Relocate a closed NURBS seam using native refinement or checked segment
/// extraction, with the shared cyclic control/knot and homogeneous join policy.
pub fn change_curve_seam(source: &NurbsCurve, parameter: f64) -> Result<NurbsCurve, Error> {
    source.try_change_closed_seam_with_operations(
        parameter,
        &mut insert_curve_knot,
        &mut |curve, parameter, _periodic| split_curve(curve, parameter),
    )
}

/// Relocate closed tensor seams in U then V using native whole-net refinement
/// or checked directional extraction and shared homogeneous seam assembly.
pub fn change_surface_seam(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter_u: f64,
    parameter_v: f64,
) -> Result<NurbsSurface, Error> {
    source.try_change_closed_seam_with_operations(
        direction,
        parameter_u,
        parameter_v,
        &mut insert_surface_knot,
        &mut |surface, axis, parameter, _periodic| {
            let domain = match axis {
                SurfaceKnotDirection::U => surface.domain_u(),
                SurfaceKnotDirection::V => surface.domain_v(),
                SurfaceKnotDirection::Both => unreachable!("one seam direction at a time"),
            };
            Ok((
                trim_surface_direction(surface, axis, *domain.start()..=parameter)?,
                trim_surface_direction(surface, axis, parameter..=*domain.end())?,
            ))
        },
    )
}

/// Restrict one surface axis, retaining the complete unselected basis and
/// exact whole-domain no-ops. Two-control lines reuse the shared exact affine
/// path; rational lines and other control curves use native extraction.
pub fn trim_surface_direction(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    interval: std::ops::RangeInclusive<f64>,
) -> Result<NurbsSurface, Error> {
    source.try_trimmed_direction_with_extractor(
        direction,
        interval,
        &mut |surface, direction, range| {
            let (degree, width) = match direction {
                SurfaceKnotDirection::U => (surface.degree_u(), surface.control_point_count_u()),
                SurfaceKnotDirection::V => (surface.degree_v(), surface.control_point_count_v()),
                SurfaceKnotDirection::Both => unreachable!("direction was validated"),
            };
            if degree == 1 && width == 2 {
                surface.try_trimmed_direction_with_nurbs(direction, range, &mut trim_curve)
            } else {
                trim_surface_axis(surface, direction, *range.start(), *range.end())
            }
        },
    )
}

/// Extract a rectangular subdomain in U then V with native directional strips.
pub fn trim_surface(
    source: &NurbsSurface,
    u: std::ops::RangeInclusive<f64>,
    v: std::ops::RangeInclusive<f64>,
) -> Result<NurbsSurface, Error> {
    source.try_trimmed_with_extractor(u, v, &mut trim_surface_direction)
}

/// Split one surface axis into two clamped intervals using checked extraction.
pub fn split_surface(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: f64,
) -> Result<(NurbsSurface, NurbsSurface), Error> {
    source.try_split_direction_with_extractor(
        direction,
        parameter,
        &mut |surface, direction, parameter| {
            let domain = match direction {
                SurfaceKnotDirection::U => surface.domain_u(),
                SurfaceKnotDirection::V => surface.domain_v(),
                SurfaceKnotDirection::Both => unreachable!("direction was validated"),
            };
            Ok((
                trim_surface_direction(surface, direction, *domain.start()..=parameter)?,
                trim_surface_direction(surface, direction, parameter..=*domain.end())?,
            ))
        },
    )
}

fn trim_surface_axis(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    start: f64,
    end: f64,
) -> Result<NurbsSurface, Error> {
    let axis = match direction {
        SurfaceKnotDirection::U => 0,
        SurfaceKnotDirection::V => 1,
        SurfaceKnotDirection::Both => unreachable!("one seam direction at a time"),
    };
    let input = NurbsInput::surface(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_trim_surface_axis(&view, axis, start, end, &mut pointer, error, capacity)
    })?;
    brep::surface_from_native(pointer)
}

/// Split once into the two clamped parameter intervals with native extraction.
pub fn split_curve(source: &NurbsCurve, parameter: f64) -> Result<(NurbsCurve, NurbsCurve), Error> {
    if !parameter.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "NURBS curve split parameter",
        }
        .into());
    }
    let domain = source.domain();
    if parameter <= *domain.start() || parameter >= *domain.end() {
        return Err(GeometryError::InvalidCurveSplitParameter.into());
    }
    Ok((
        trim_curve(source, *domain.start()..=parameter)?,
        trim_curve(source, parameter..=*domain.end())?,
    ))
}

/// Split at sorted distinct interior parameters, retaining closed cyclic order.
pub fn split_curve_at_parameters(
    source: &NurbsCurve,
    parameters: &[f64],
) -> Result<Vec<NurbsCurve>, Error> {
    source.try_split_at_parameters_with_extractor(parameters, &mut trim_curve)
}

fn decompose(
    input: NurbsInput<'_>,
    direction: c_int,
    count: usize,
    output_controls: usize,
) -> Result<Vec<*mut brep::NativeSurfaceData>, Error> {
    if output_controls > viboceros_geometry::MAX_BEZIER_CONTROL_POINTS {
        return Err(GeometryError::BezierDecompositionLimit.into());
    }
    let view = input.view();
    let mut pointers = vec![std::ptr::null_mut(); count];
    let mut control_error = 0;
    let result = call(|error, capacity| unsafe {
        vb_nurbs_bezier(
            &view,
            direction,
            pointers.as_mut_ptr(),
            count,
            &mut control_error,
            error,
            capacity,
        )
    });
    if control_error != 0 {
        return Err(GeometryError::UnrepresentableBezierControl.into());
    }
    result?;
    Ok(pointers)
}

/// Extract exact clamped Bezier curves with the original active span domains.
pub fn curve_bezier_spans(source: &NurbsCurve) -> Result<Vec<NurbsCurve>, Error> {
    let count = source.spans().count();
    let controls = count.saturating_mul(source.degree().saturating_add(1));
    brep::curves_from_native_batch(decompose(NurbsInput::curve(source), 0, count, controls)?)
}

/// Extract single-axis strips or U-outer/V-inner tensor Bezier patches.
/// The unselected direction retains its complete source basis.
pub fn surface_single_span_patches(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
) -> Result<Vec<NurbsSurface>, Error> {
    let u = direction != SurfaceKnotDirection::V;
    let v = direction != SurfaceKnotDirection::U;
    let count = (if u { source.spans_u().count() } else { 1 }).saturating_mul(if v {
        source.spans_v().count()
    } else {
        1
    });
    let controls = count
        .saturating_mul(if u {
            source.degree_u().saturating_add(1)
        } else {
            source.control_point_count_u()
        })
        .saturating_mul(if v {
            source.degree_v().saturating_add(1)
        } else {
            source.control_point_count_v()
        });
    brep::surfaces_from_native_batch(decompose(
        NurbsInput::surface(source),
        match direction {
            SurfaceKnotDirection::U => 0,
            SurfaceKnotDirection::V => 1,
            SurfaceKnotDirection::Both => 2,
        },
        count,
        controls,
    )?)
}
