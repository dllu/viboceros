//! Native Used deletion charts are distinct from exact arrangement parameters.
use super::*;
pub(super) fn remnants(
    curve: &NurbsCurve,
    intervals: Vec<std::ops::RangeInclusive<Real>>,
) -> Result<Vec<Geometry>, GeometryError> {
    let domain = curve.domain();
    let mut start = *domain.start();
    let mut pieces = Vec::new();
    let epsilon = (domain.end() - domain.start()) * Real::EPSILON * 128.;
    for interval in intervals {
        let lo = deletion_parameter(curve, *interval.start());
        let hi = deletion_parameter(curve, *interval.end());
        if lo - start > epsilon {
            pieces.push(Geometry::NurbsCurve(curve.try_trimmed(start..=lo)?));
        }
        start = start.max(hi);
    }
    if domain.end() - start > epsilon {
        pieces.push(Geometry::NurbsCurve(
            curve.try_trimmed(start..=*domain.end())?,
        ));
    }
    Ok(pieces)
}
fn deletion_parameter(curve: &NurbsCurve, t: Real) -> Real {
    if curve.degree() != 1 {
        return t;
    }
    let knots = curve.knots();
    let span = knots
        .partition_point(|k| *k <= t)
        .saturating_sub(1)
        .clamp(1, curve.control_points().len() - 1);
    let [a, b] = [knots[span], knots[span + 1]];
    let [w0, w1] = [
        curve.control_points()[span - 1].weight(),
        curve.control_points()[span].weight(),
    ];
    if w0 == w1 || a == b {
        return t;
    }
    // Rhino CurveBoolean maps its graph line fraction affinely into each
    // original degree-one knot span before trimming Used input. For unequal
    // weights this differs from the exact rational inverse used by the graph
    // and other curve operations. The command policy retains original weights.
    let u = ((t - a) / (b - a)).clamp(0., 1.);
    let scale = w0.abs().max(w1.abs());
    let (w0, w1) = (w0 / scale, w1 / scale);
    let fraction = w1 * u / ((1. - u) * w0 + u * w1);
    a.mul_add(1. - fraction, b * fraction)
}
