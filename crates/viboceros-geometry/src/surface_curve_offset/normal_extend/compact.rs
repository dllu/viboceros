//! Compact native trace scaffolding before differentiating a continued source.
use super::*;

pub(super) fn section(
    surface: &NurbsSurface,
    plane: Plane,
    original: &NurbsCurve,
    outward: Vector3,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<NurbsCurve, GeometryError> {
    let source = original.try_reparameterized(0. ..=1.)?;
    let numerical = Tolerance::try_new(
        tolerance.absolute() * 0.02,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let mut lengths =
        crate::curve::ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), numerical)?;
    lengths.prepare_budgeted_repeated_sampling(32)?;
    let length = lengths.total_length();
    let first = source.evaluate(0.)?;
    let shift = first.vector_to(Point3::try_new(0., 0., 0.)?)?;
    let last = source.evaluate(1.)?;
    let (u, v) = surface.closest_parameters(last, tolerance)?;
    if surface.evaluate(u, v)?.distance_to(last)? > tolerance.absolute() {
        return Err(invalid("normal section endpoint is not on the surface"));
    }
    let d = source.derivative_at(1.)?;
    let direction = surface
        .normal_at(u, v)?
        .as_vector()
        .cross(plane.normal().as_vector())?
        .normalized_nonzero()?
        .as_vector();
    let derivatives = Some([
        outward.scaled(length)?,
        direction.scaled(length * direction.dot(d)?.signum())?,
    ]);
    let mut breaks = seed_breaks(source.degree(), source.knots(), source.domain());
    let mut retained = Vec::with_capacity(breaks.len());
    for knot in breaks.drain(..) {
        if knot.parameter == 0. || knot.parameter == 1. {
            retained.push(knot);
            continue;
        }
        let (p, a) =
            source.evaluate_with_derivative_on_side(knot.parameter, ParameterSide::Left)?;
        let (q, b) =
            source.evaluate_with_derivative_on_side(knot.parameter, ParameterSide::Right)?;
        if p.distance_to(q)? > tolerance.absolute() {
            return Err(invalid("normal section has an interior spatial gap"));
        }
        if extend::direction_gap(a, b)? > tolerance.angular() {
            retained.push(crate::spline_collocation::Break {
                parameter: lengths.distance_at_parameter(knot.parameter)? / length,
                multiplicity: 3,
            });
        }
    }
    // Native Hermite traces can retain hundreds of approximation joins. Only
    // genuine tangent breaks constrain the new basis. Arc length removes the
    // native trace's nonuniform parameter scaffolding. Sampling proposes a fit;
    // the complete surface image and exact plane band admit the result.
    for fraction in [0.25, 0.05, 0.01] {
        let fitting = Tolerance::try_new(
            tolerance.absolute() * fraction,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let candidate = fit_parameterized_curve_with_derivatives(
            &source,
            retained.clone(),
            fitting,
            MAX_CURVE_FIT_CONTROL_POINTS,
            solver,
            derivatives,
            |t, side| {
                if t == 0. {
                    return Point3::try_new(0., 0., 0.);
                }
                source
                    .evaluate_on_side(lengths.parameter_at_distance(t * length)?, side)?
                    .translated(shift.scaled(1. - t)?)
            },
        )?;
        if surface
            .try_pullback_curve_certified(&candidate, tolerance)
            .is_ok()
            && in_plane(&candidate, plane, tolerance.absolute())
        {
            return candidate.try_reparameterized(original.domain());
        }
    }
    Err(invalid("compact normal section could not be certified"))
}
