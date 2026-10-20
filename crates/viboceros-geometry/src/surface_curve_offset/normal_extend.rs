//! Endpoint normal sections reuse the exact surface/plane intersection layer.
use super::*;
use crate::{
    CurveExtensionSide, Frame3, Plane, Point3, SurfaceSurfaceIntersectionEvent, Vector3,
    WeightedPoint3,
};
mod compact;
#[cfg(test)]
mod tests;

/// Continue each requested open end along its tangent normal section, up to
/// the first natural rectangle edge or half a closed section. Source admission
/// and the returned surface image are certified; unsupported plane sections
/// fail without returning partial geometry. The section policy is inferred
/// from public SDK observations, independently of UV span extrapolation.
pub fn try_extend_curve_normally_on_surface(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    side: CurveExtensionSide,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<SurfaceCurveExtension, GeometryError> {
    try_extend_curve_normally_on_surface_with_operation(
        original_surface,
        original_source,
        side,
        tolerance,
        solver,
        &mut crate::surface_surface_intersection_events,
    )
}

/// Use a shared surface intersection producer when the portable dispatcher
/// has no implementation for a normal section.
/// Source admission, branch orientation, section limits and complete image
/// checks remain in the geometry core. Each retained section must also prove
/// its complete image in the cutting plane; a supplied producer is not trusted.
pub fn try_extend_curve_normally_on_surface_with_operation(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    side: CurveExtensionSide,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
    intersect: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Tolerance,
    ) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError>,
) -> Result<SurfaceCurveExtension, GeometryError> {
    let (surface, mut source, numerical) =
        normalized_sources(original_surface, original_source, 0., tolerance)?;
    let closed = [
        crate::surface_geodesic::chart_closes(&surface, 0)?,
        crate::surface_geodesic::chart_closes(&surface, 1)?,
    ];
    let admission = Tolerance::try_new(
        tolerance.absolute() * 0.25,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    // Preserve an already on-surface spatial representation. A looser UV
    // approximation can exhaust the tight image budget even for an exact
    // rational source; refine its pullback before proposing any projection.
    let (uv, bound) = surface
        .try_pullback_curve_certified_with_bound(&source, None, numerical)
        .or_else(|_| surface.try_pullback_curve_certified_with_bound(&source, None, admission))?;
    if source.is_closed()? {
        return extend::extension(surface, uv, original_source.clone(), closed);
    }
    if bound > numerical.absolute() {
        source = extend::smooth_image(&surface, &uv, numerical, solver)?;
    }
    let parameter_tolerance = clip::uv_tolerance(&surface, [false; 2], tolerance)?.absolute();
    let original = source.clone();
    for at_start in [true, false] {
        if at_start && matches!(side, CurveExtensionSide::End)
            || !at_start && matches!(side, CurveExtensionSide::Start)
        {
            continue;
        }
        let t = if at_start { 0. } else { 1. };
        let (q, motion) = uv.evaluate_with_derivative(t)?;
        let motion = motion.map(|v| if at_start { -v } else { v });
        let p = q.to_array();
        if (0..2).any(|axis| {
            p[axis] <= parameter_tolerance && motion[axis] < 0.
                || p[axis] >= 1. - parameter_tolerance && motion[axis] > 0.
        }) {
            continue;
        }
        // A global UV derivative bound can be much more conservative than
        // this endpoint's physical metric. Prove a nearby outward boundary
        // contact before asking an intersector for a sub-budget sliver.
        let mut leaving = false;
        let endpoint = original.evaluate(t)?;
        for axis in 0..2 {
            if motion[axis] == 0. {
                continue;
            }
            let mut edge = p;
            edge[axis] = if motion[axis] < 0. { 0. } else { 1. };
            let edge = surface.evaluate(edge[0], edge[1])?;
            if endpoint.distance_to(edge)? <= numerical.absolute()
                && original.positions_within_distance(
                    &[(
                        t,
                        if at_start {
                            ParameterSide::Right
                        } else {
                            ParameterSide::Left
                        },
                        edge,
                    )],
                    numerical.absolute() * 0.9,
                )?
            {
                leaving = true;
                break;
            }
        }
        if leaving {
            continue;
        }
        let point = surface.evaluate(q.x(), q.y())?;
        let normal = surface.normal_at(q.x(), q.y())?.as_vector();
        let tangent = original.derivative_at(t)?;
        // Project the admitted tangent into the tangent plane. Plane-section
        // producers may return several branches; position and tangent choose
        // the local continuation, never a nearest unrelated component.
        let binormal = normal.cross(tangent)?.normalized_nonzero()?.as_vector();
        let forward = binormal.cross(normal)?.normalized_nonzero()?.as_vector();
        let outward = forward.scaled(if at_start { -1. } else { 1. })?;
        let mut chosen = None;
        for section in sections(
            &surface, point, outward, normal, numerical, solver, intersect,
        )? {
            let mut section = section.try_reparameterized(0. ..=1.)?;
            // The intersection layer treats a periodic seam as interior. The
            // endpoint extension policy stops at its first rectangle contact.
            let chart = surface.try_pullback_curve_certified(&section, numerical)?;
            if let Some(cut) = chart
                .knots()
                .chunk_by(|a, b| a == b)
                .filter(|k| k.len() == chart.degree() + 1 && k[0] > 0. && k[0] < 1.)
                .map(|k| k[0])
                .map(|t| {
                    let a = chart.evaluate_on_side(t, ParameterSide::Left)?;
                    let b = chart.evaluate_on_side(t, ParameterSide::Right)?;
                    Ok((
                        t,
                        closed[0] && (a.x() - b.x()).abs() > 0.5
                            || closed[1] && (a.y() - b.y()).abs() > 0.5,
                    ))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
                .into_iter()
                .find_map(|(t, seam)| seam.then_some(t))
            {
                section = section.try_trimmed(0. ..=cut)?;
            }
            if chosen.replace(section).is_some() {
                return Err(invalid("ambiguous endpoint normal section branches"));
            }
        }
        let Some(mut section) = chosen else {
            return Err(invalid("no regular outward endpoint normal section"));
        };
        if at_start {
            section = section.reversed()?;
        }
        section = section.try_reparameterized(0. ..=1.)?;
        let duration = section
            .derivative_at(if at_start { 1. } else { 0. })?
            .length()?
            / tangent.length()?;
        if !duration.is_finite() || duration <= 0. {
            return Err(invalid("normal section duration cannot be represented"));
        }
        section = if at_start {
            section.try_reparameterized(-duration..=0.)?
        } else {
            section.try_reparameterized(1. ..=1. + duration)?
        };
        let degree = source.degree().max(section.degree());
        source = source.try_change_degree(degree, false)?;
        section = section.try_change_degree(degree, false)?;
        let interval = section.domain();
        source = if at_start {
            section.try_append_clamped(&source)?
        } else {
            source.try_append_clamped(&section)?
        };
        let plane = Plane::new(point, outward.cross(normal)?.normalized_nonzero()?);
        if !in_plane(&source.try_trimmed(interval)?, plane, numerical.absolute()) {
            return Err(invalid(
                "normal section encoding exceeded its plane tolerance",
            ));
        }
    }
    let uv = surface.try_pullback_curve_certified(&source, numerical)?;
    let domain = [
        *original_source.domain().start(),
        *original_source.domain().end(),
    ];
    let restored = source.try_reparameterized(
        crate::remap_scalar(*source.domain().start(), [0., 1.], domain)?
            ..=crate::remap_scalar(*source.domain().end(), [0., 1.], domain)?,
    )?;
    preserve_spans(source.knots(), restored.knots(), source.degree())?;
    if surface
        .parameter_curve_deviation_bound(&uv, &restored, numerical.absolute())?
        .is_none()
    {
        return Err(invalid(
            "normal extension restoration exceeded image tolerance",
        ));
    }
    extend::extension(surface, uv, restored, closed)
}

fn sections(
    surface: &NurbsSurface,
    origin: Point3,
    outward: Vector3,
    normal: Vector3,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
    intersect: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Tolerance,
    ) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    // Solve sections near the endpoint at unit physical scale. General
    // intersections have coordinate-relative budgets and primitive predicates;
    // world units must not make a regular tiny cylinder degenerate.
    let scale = surface
        .control_points()
        .iter()
        .try_fold(0_f64, |scale, p| {
            Ok::<_, GeometryError>(scale.max(origin.distance_to(p.point())?))
        })?;
    if scale == 0. {
        return Err(invalid("constant normal-section surface"));
    }
    let o = origin.to_array();
    let controls = surface
        .control_points()
        .iter()
        .map(|c| {
            let p = c.point().to_array();
            WeightedPoint3::try_new(
                Point3::try_from(std::array::from_fn(|i| (p[i] - o[i]) / scale))?,
                c.weight(),
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let local = NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        surface.control_point_count_u(),
        surface.control_point_count_v(),
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )?;
    let absolute = tolerance.absolute() / scale;
    let tolerance = Tolerance::try_new(
        absolute,
        tolerance.relative().min(absolute * 0.125),
        tolerance.angular(),
    )?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let frame = Frame3::try_from_directions(zero, outward, normal, tolerance)?;
    let plane = Plane::new(zero, frame.z_axis());
    let cutter = cutter(&local, frame, tolerance)?;
    let mut result = Vec::new();
    let (events, external) =
        match crate::surface_surface_intersection_events(&local, &cutter, tolerance) {
            Ok(events) => (events, false),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "non-planar surfaces",
            }) => (intersect(&local, &cutter, tolerance)?, true),
            Err(error) => return Err(error),
        };
    for event in events {
        let SurfaceSurfaceIntersectionEvent::Curve(section) = event else {
            continue;
        };
        let anchor = section.closest_parameter(zero, tolerance)?;
        if section.evaluate(anchor)?.distance_to(zero)? > tolerance.absolute() {
            continue;
        }
        let direction = section
            .derivative_at(anchor)?
            .normalized_nonzero()?
            .as_vector();
        if direction.dot(outward)?.abs() < 1. - 1e-8 {
            continue;
        }
        let section = if direction.dot(outward)? < 0. {
            section.reversed()?
        } else {
            section
        };
        let anchor = section.closest_parameter(zero, tolerance)?;
        let capped = section.is_closed()?;
        let section = if capped {
            let section = section.try_change_closed_seam(anchor)?;
            let stop = section
                .parameter_at_arc_length_from(
                    *section.domain().start(),
                    section.length(tolerance)? * 0.5,
                    tolerance,
                )?
                .ok_or_else(|| invalid("closed normal section length inversion failed"))?;
            section.try_trimmed(*section.domain().start()..=stop)?
        } else {
            if section
                .evaluate(*section.domain().end())?
                .distance_to(zero)?
                <= tolerance.absolute()
            {
                continue;
            }
            section.try_trimmed(anchor..=*section.domain().end())?
        };
        if !capped {
            let end = section.evaluate(*section.domain().end())?;
            let (u, v) = local.closest_parameters(end, tolerance)?;
            let allowed = clip::uv_tolerance(&local, [false; 2], tolerance)?.absolute();
            if end.distance_to(local.evaluate(u, v)?)? > tolerance.absolute()
                || u.min(1. - u).min(v.min(1. - v)) > allowed
            {
                return Err(invalid("normal section ended before a natural boundary"));
            }
        }
        // The source-surface proof follows after reconstruction. Independently
        // reject a candidate that wanders out of the endpoint normal plane.
        if !in_plane(&section, plane, tolerance.absolute()) {
            return Err(invalid(
                "normal section exceeded its cutting plane tolerance",
            ));
        }
        let section = if external && section.control_points().len() > 8 {
            compact::section(&local, plane, &section, outward, tolerance, solver)?
        } else {
            section
        };
        let controls = section
            .control_points()
            .iter()
            .map(|c| {
                let p = c.point().to_array();
                WeightedPoint3::try_new(
                    Point3::try_from(std::array::from_fn(|i| p[i].mul_add(scale, o[i])))?,
                    c.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        result.push(NurbsCurve::try_new_rational(
            section.degree(),
            controls,
            section.knots().to_vec(),
        )?);
    }
    Ok(result)
}

fn in_plane(curve: &NurbsCurve, plane: Plane, limit: Real) -> bool {
    use crate::exact_scalar::rational;
    use num_traits::Zero;
    let weight = curve.control_points()[0].weight();
    if weight == 0.
        || curve
            .control_points()
            .iter()
            .any(|p| p.weight() == 0. || p.weight().is_sign_positive() != weight.is_sign_positive())
    {
        return false;
    }
    let n = plane.normal().as_vector().to_array().map(rational);
    let o = plane.origin().to_array().map(rational);
    let squared = rational(limit)
        * rational(limit)
        * n.iter()
            .map(|q| q * q)
            .sum::<crate::exact_scalar::Rational>();
    // Same-sign weights put the complete rational curve in its Euclidean
    // control hull. An exact linear-functional band proves plane distance
    // without discovering a second UV curve on an almost-affine cutter.
    curve.control_points().iter().all(|c| {
        let p = c.point().to_array();
        let mut d = crate::exact_scalar::Rational::zero();
        for i in 0..3 {
            d += (rational(p[i]) - &o[i]) * &n[i];
        }
        &d * &d <= squared
    })
}

fn cutter(
    surface: &NurbsSurface,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<NurbsSurface, GeometryError> {
    let mut low = [Real::INFINITY; 2];
    let mut high = [Real::NEG_INFINITY; 2];
    for control in surface.control_points() {
        let q = frame.coordinates_of(control.point())?;
        for i in 0..2 {
            low[i] = low[i].min(q[i]);
            high[i] = high[i].max(q[i]);
        }
    }
    for i in 0..2 {
        let margin = (high[i] - low[i]).max(tolerance.absolute() * 8.);
        low[i] -= margin;
        high[i] += margin;
    }
    NurbsSurface::try_clamped_uniform(
        1,
        1,
        2,
        2,
        [
            [low[0], low[1], 0.],
            [low[0], high[1], 0.],
            [high[0], low[1], 0.],
            [high[0], high[1], 0.],
        ]
        .map(|q| frame.point_at(q))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?,
    )
}
