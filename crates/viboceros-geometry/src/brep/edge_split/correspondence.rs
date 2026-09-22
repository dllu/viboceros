use super::*;
use crate::ParameterSide;

pub(super) fn parameters(
    image: &trim_image::LiftedTrim<'_>,
    trim: &BrepTrim,
    edge: &BrepEdge,
    plan: &SplitPlan,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<Real>, GeometryError> {
    let domain = edge.curve.domain();
    let allowed = tolerance.absolute().max(edge.tolerance);
    let epsilon = (allowed * 0.125).max(Real::MIN_POSITIVE);
    let mut samples = None;
    let mut result = Vec::with_capacity(plan.parameters.len());
    for (&parameter, &point) in plan.parameters.iter().zip(&plan.points) {
        let span = *domain.end() - *domain.start();
        let fraction = if span.is_finite() {
            (parameter - *domain.start()) / span
        } else {
            let scale = domain.start().abs().max(domain.end().abs());
            (parameter / scale - domain.start() / scale)
                / (domain.end() / scale - domain.start() / scale)
        };
        let fraction = if trim.reversed_3d {
            1. - fraction
        } else {
            fraction
        };
        let trim_domain = image.curve.domain();
        // Preserve exact existing knots in the usual projected-trim case.
        // Normalizing and denormalizing the same domain can round an integer
        // knot off itself, producing an unnecessary microscopic trim span.
        let direct = if !trim.reversed_3d && trim_domain == domain {
            parameter
        } else if trim.reversed_3d
            && *trim_domain.start() == -*domain.end()
            && *trim_domain.end() == -*domain.start()
        {
            -parameter
        } else {
            image.curve.parameter_at(fraction.clamp(0., 1.))?
        };
        budget.charge(image.curve.degree().saturating_add(1))?;
        let found = if image
            .point(direct, ParameterSide::Right)?
            .distance_to(point)?
            <= epsilon
        {
            direct
        } else {
            if samples.is_none() {
                let mut stations = Vec::new();
                for (start, end) in image.curve.spans() {
                    budget.charge(9usize.saturating_mul(image.curve.degree().saturating_add(1)))?;
                    for i in 0..=8 {
                        let t = normalized_span_parameter([start, end], i as Real / 8.)?;
                        let side = if t == end {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        };
                        stations.push((t, side, image.point(t, side)?));
                    }
                }
                samples = Some(stations);
            }
            let samples = samples.as_ref().expect("initialized trim samples");
            // Every supplied seed can now be refined; do not retain the old
            // fixed sixteen-start allowance when charging correspondence work.
            budget.charge(
                samples.len().saturating_mul(
                    trim_image::MAX_REFINEMENT_STEPS
                        .saturating_mul(image.curve.degree().saturating_add(1))
                        .saturating_add(1),
                ),
            )?;
            let (distance, parameter) = image.distance_witness(point, samples, epsilon)?;
            if distance > allowed {
                return invalid("edge split could not resolve its trim parameter");
            }
            parameter
        };
        result.push(found);
    }
    if trim.reversed_3d {
        result.reverse();
    }
    let domain = image.curve.domain();
    if result
        .iter()
        .any(|&t| t <= *domain.start() || t >= *domain.end())
        || result.windows(2).any(|w| w[0] >= w[1])
    {
        return invalid("edge split trim parameters are not strictly monotone");
    }
    Ok(result)
}
