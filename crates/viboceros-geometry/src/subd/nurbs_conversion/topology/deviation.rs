//! Certify the full boundary correspondence of finite SubD approximations.
use super::*;

pub(super) fn bound(a: &NurbsCurve, b: &NurbsCurve, minimum: f64) -> Result<f64, GeometryError> {
    if a.degree() == b.degree()
        && a.knots() == b.knots()
        && a.control_points()
            .iter()
            .chain(b.control_points())
            .all(|p| p.weight() == 1.)
    {
        let anchor = a.control_points()[0].point();
        let mut radius = 0_f64;
        let mut delta = 0_f64;
        for (a, b) in a.control_points().iter().zip(b.control_points()) {
            delta = delta.max(a.point().distance_to(b.point())?);
            radius = radius
                .max(anchor.distance_to(a.point())?)
                .max(anchor.distance_to(b.point())?);
        }
        // A common positive polynomial basis bounds its entire curve image.
        return Ok((delta + 128. * f64::EPSILON * radius.max(1.)).max(minimum));
    }
    let domain_a = a.domain();
    let domain_b = b.domain();
    let mut proposal = minimum;
    for i in 0..=16 {
        let f = i as f64 / 16.;
        let pa = a.evaluate(*domain_a.start() + f * (*domain_a.end() - *domain_a.start()))?;
        let pb = b.evaluate(*domain_b.start() + f * (*domain_b.end() - *domain_b.start()))?;
        proposal = proposal.max(2. * pa.distance_to(pb)?);
    }
    let anchor = a.control_points()[0].point();
    let radius = |curve: &NurbsCurve| {
        curve.control_points().iter().try_fold(0_f64, |r, p| {
            Ok::<_, GeometryError>(r.max(anchor.distance_to(p.point())?))
        })
    };
    let cap = (radius(a)? + radius(b)?).next_up().max(minimum);
    // Samples only propose a limit. Every accepted result covers the complete
    // curve pair through the exact rational Bernstein certificate.
    for _ in 0..64 {
        let limit = proposal.min(cap);
        if let Some(bound) = a.parameter_curve_deviation_bound(b, limit)? {
            return Ok(bound.next_up().max(minimum));
        }
        if limit == cap {
            break;
        }
        proposal = (2. * proposal).min(cap);
    }
    Err(GeometryError::CurveApproximationLimit)
}
