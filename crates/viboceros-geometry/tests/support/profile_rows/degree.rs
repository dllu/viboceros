//! Charge rounded U elevation before comparing the resulting profile rows.
use super::*;

pub(super) fn elevated_u(
    source: &NurbsSurface,
    degree: usize,
    limit: f64,
) -> Option<(NurbsSurface, f64)> {
    let elevated = source
        .try_change_degree(degree, source.degree_v(), false)
        .ok()?;
    if source.knots_v() != elevated.knots_v()
        || source.degree_v() != elevated.degree_v()
        || source.control_point_count_v() != elevated.control_point_count_v()
    {
        return None;
    }
    let mut maximum = 0_f64;
    let gauge =
        rat(elevated.control_point(0, 0)?.weight()) / rat(source.control_point(0, 0)?.weight());
    let mut delta = Rational::zero();
    let mut minimum = rat(elevated.control_point(0, 0)?.weight());
    for v in 0..source.control_point_count_v() {
        let weight = source.control_point(0, v)?.weight();
        // These tensors have the same positive rational V coefficients,
        // independent of U. A maximum complete U-column bound therefore
        // bounds their entire surface difference by convexity. Decline
        // coupled U/V weights instead of assuming this separation.
        if weight <= 0.
            || (0..source.control_point_count_u())
                .any(|u| source.control_point(u, v).unwrap().weight() != weight)
        {
            return None;
        }
        for u in 0..elevated.control_point_count_u() {
            let actual = rat(elevated.control_point(u, v)?.weight());
            if actual <= Rational::zero() {
                return None;
            }
            minimum = minimum.min(actual.clone());
            delta = delta.max((actual - &gauge * rat(weight)).abs());
        }
        let column = |s: &NurbsSurface| {
            NurbsCurve::try_new_rational(
                s.degree_u(),
                (0..s.control_point_count_u())
                    .map(|u| {
                        viboceros_geometry::WeightedPoint3::try_new(
                            s.control_point(u, v).unwrap().point(),
                            weight,
                        )
                        .unwrap()
                    })
                    .collect(),
                s.knots_u().to_vec(),
            )
            .ok()
        };
        let before = column(source)?;
        let after = column(&elevated)?;
        let bound = match before.parameter_curve_deviation_bound(&after, limit) {
            Ok(Some(bound)) => bound,
            Ok(None) | Err(GeometryError::CurveApproximationLimit) => return None,
            Err(error) => panic!("invalid degree-elevation certificate: {error}"),
        };
        maximum = maximum.max(bound);
    }
    let weight_error =
        upper(&(Rational::from_integer(2.into()) * delta * radius(&elevated) / minimum))?;
    Some((elevated, (maximum + weight_error).next_up()))
}
