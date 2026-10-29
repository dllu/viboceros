//! Exact projected surface counterexamples, with numerical witness proposals.
use super::*;
use viboceros_geometry::{ParameterSide, Point3, Tolerance, WeightedPoint3};

fn projected(curve: &NurbsCurve, axis: usize) -> Option<NurbsCurve> {
    let controls = curve
        .control_points()
        .iter()
        .map(|control| {
            let mut point = control.point().to_array();
            point[axis] = 0.;
            WeightedPoint3::try_new(Point3::try_from(point).ok()?, control.weight()).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec()).ok()
}

pub(super) fn lower_bound(a: &NurbsSurface, b: &NurbsSurface, limit: f64) -> Option<f64> {
    if !limit.is_finite()
        || limit <= 0.
        || !a.knots_u()[..=a.degree_u()]
            .iter()
            .all(|knot| knot == a.domain_u().start())
        || [a, b]
            .into_iter()
            .any(|s| s.control_points().iter().any(|p| p.weight() <= 0.))
    {
        return None;
    }
    // Clamping puts the first source U row on the actual tensor image. Its
    // weights are copied directly, so the source needs no separability estimate.
    let source = rows(a).curves.remove(0);
    let target = rows(b);
    let reference = &target.curves[0];
    let mut chosen = None;
    for axis in 0..3 {
        let mut delta = Rational::zero();
        for curve in &target.curves {
            for (p, q) in curve
                .control_points()
                .iter()
                .zip(reference.control_points())
            {
                let p = p.point().to_array();
                let q = q.point().to_array();
                let distance = (0..3)
                    .filter(|&i| i != axis)
                    .map(|i| (rat(p[i]) - rat(q[i])).abs())
                    .sum();
                delta = delta.max(distance);
            }
        }
        // Positive tensor bases are convex coefficients. Charge both the
        // original tensor's weight-factorization error and the projected ideal
        // rows' maximum exact L1 control difference from the reference row.
        let error = upper(&(delta + &target.weight_error))?;
        if chosen.is_none_or(|(_, previous)| error < previous) {
            chosen = Some((axis, error));
        }
    }
    let (axis, projection_error) = chosen?;
    if projection_error >= limit {
        return None;
    }
    let source = projected(&source, axis)?;
    let target = projected(reference, axis)?;
    let numerical = Tolerance::try_new((limit * 1e-4).max(f64::from_bits(1)), 1e-12, 1e-10).ok()?;
    let mut proposal = None;
    for index in 1..64 {
        let fraction = f64::from(index) / 64.;
        let parameter = source.parameter_at(fraction).ok()?;
        let point = source.evaluate(parameter).ok()?;
        // These related profiles usually have similar clocks. A cheap local
        // refinement only proposes a witness; the global bracket below is
        // required even if this seed converges to an unrelated local minimum.
        let nearest = target
            .point_getter_parameter_from_seed(point, target.parameter_at(fraction).ok()?, numerical)
            .ok()?;
        let distance = point.distance_to(target.evaluate(nearest).ok()?).ok()?;
        if proposal.is_none_or(|(_, _, _, previous)| distance > previous) {
            proposal = Some((parameter, point, nearest, distance));
        }
    }
    let (parameter, point, nearest, distance) = proposal?;
    let evaluation_error = (limit * 1e-5).max(f64::from_bits(1));
    if distance <= limit + projection_error + evaluation_error {
        return None;
    }
    if !source
        .positions_within_distance(
            &[(parameter, ParameterSide::Right, point)],
            evaluation_error,
        )
        .ok()?
    {
        return None;
    }
    let constant = NurbsCurve::try_new(1, vec![point; 2], vec![0., 0., 1., 1.]).ok()?;
    let bracket = target
        .minimum_distance_with_curve_seeded(
            &constant,
            Tolerance::try_new((limit * 0.01).max(f64::from_bits(1)), 1e-12, 1e-10).ok()?,
            [nearest, 0.],
        )
        .ok()??;
    // A numerical nearest point is only a seed. The complete global minimum
    // bracket proves distance to every point of the projected reference curve.
    // Orthogonal projection is 1-Lipschitz; subtract every outward error reserve
    // before claiming separation from the complete original tensor image.
    let lower = (bracket.upper_distance - bracket.maximum_error).next_down();
    let lower = (lower - evaluation_error).next_down();
    let lower = (lower - projection_error).next_down();
    (lower > limit).then_some(lower)
}
