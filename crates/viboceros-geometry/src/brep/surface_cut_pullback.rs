//! Certified cutting p-curves with shared UV endpoints.
use super::*;

pub(super) fn surface_split_parameter_curve(
    surface: &NurbsSurface,
    curve: &NurbsCurve,
    start: Point2,
    end: Point2,
    tolerance: Tolerance,
) -> Result<NurbsCurve2, GeometryError> {
    let curve_domain = curve.domain();
    let line = NurbsCurve2::try_new(
        1,
        vec![start, end],
        vec![
            *curve_domain.start(),
            *curve_domain.start(),
            *curve_domain.end(),
            *curve_domain.end(),
        ],
    )?;
    let line_matches = parameter_curve_matches_spatial_curve(surface, &line, curve, tolerance)?;
    let parameter_curve = match surface.try_pullback_bilinear_curve(curve, tolerance) {
        Ok(_) if curve.degree() == 1 && curve.control_points().len() == 2 && line_matches => {
            return Ok(line);
        }
        Ok(parameter_curve) => parameter_curve,
        Err(_) if line_matches => {
            // Rhino's general-surface pullback stores an exact straight trim as
            // one cubic Bezier, while its bilinear path retains the source form.
            let delta_x = end.x() - start.x();
            let delta_y = end.y() - start.y();
            let cubic = NurbsCurve2::try_new(
                3,
                vec![
                    start,
                    Point2::try_new(
                        delta_x.mul_add(1.0 / 3.0, start.x()),
                        delta_y.mul_add(1.0 / 3.0, start.y()),
                    )?,
                    Point2::try_new(
                        delta_x.mul_add(2.0 / 3.0, start.x()),
                        delta_y.mul_add(2.0 / 3.0, start.y()),
                    )?,
                    end,
                ],
                vec![
                    *curve_domain.start(),
                    *curve_domain.start(),
                    *curve_domain.start(),
                    *curve_domain.start(),
                    *curve_domain.end(),
                    *curve_domain.end(),
                    *curve_domain.end(),
                    *curve_domain.end(),
                ],
            )?;
            if !parameter_curve_matches_spatial_curve(surface, &cubic, curve, tolerance)? {
                return surface.try_pullback_curve_certified_with_endpoints(
                    curve,
                    [start, end],
                    tolerance,
                );
            }
            return Ok(cubic);
        }
        Err(_) => {
            return surface.try_pullback_curve_certified_with_endpoints(
                curve,
                [start, end],
                tolerance,
            );
        }
    };
    let parameter_tolerance = [
        trim_parameter_epsilon(
            [*surface.domain_u().start(), *surface.domain_u().end()],
            tolerance,
        ),
        trim_parameter_epsilon(
            [*surface.domain_v().start(), *surface.domain_v().end()],
            tolerance,
        ),
    ];
    let actual_start = parameter_curve.start_point()?;
    let actual_end = parameter_curve.end_point()?;
    if !parameter_points_near(actual_start, start, parameter_tolerance)
        || !parameter_points_near(actual_end, end, parameter_tolerance)
    {
        // Clipping an approximate pullback and pulling back the clipped edge
        // independently need not produce identical endpoint parameters. Keep
        // the shared topological endpoints and qualify the adjusted trim in
        // model space, where the caller's approximation budget is defined.
        let Some(adjusted) =
            crate::surface_pullback::constrain_curve_endpoints(parameter_curve, [start, end])?
        else {
            return surface.try_pullback_curve_certified_with_endpoints(
                curve,
                [start, end],
                tolerance,
            );
        };
        if !parameter_curve_matches_spatial_curve(surface, &adjusted, curve, tolerance)? {
            return surface.try_pullback_curve_certified_with_endpoints(
                curve,
                [start, end],
                tolerance,
            );
        }
        return Ok(adjusted);
    }
    if !parameter_curve_matches_spatial_curve(surface, &parameter_curve, curve, tolerance)? {
        return surface.try_pullback_curve_certified_with_endpoints(curve, [start, end], tolerance);
    }
    Ok(parameter_curve)
}

pub(super) fn parameter_curve_matches_spatial_curve(
    surface: &NurbsSurface,
    parameter_curve: &NurbsCurve2,
    spatial_curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    Ok(surface
        .parameter_curve_deviation_bound(parameter_curve, spatial_curve, tolerance.absolute())?
        .is_some())
}
