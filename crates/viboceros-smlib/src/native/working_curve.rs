//! Canonical NURBS preparation in a translated, scaled geometry and unit parameter frame.
use super::*;

pub(super) fn normalized(
    source: &NurbsCurve,
    origin: Point3,
    scale: f64,
) -> Result<Option<NurbsCurve>, Error> {
    if source.degree() > 17
        || source.knots()[source.degree() + 1..source.control_points().len()]
            .windows(source.degree() + 1)
            .any(|group| group.iter().all(|k| *k == group[0]))
    {
        return Ok(None);
    }
    let reference = source
        .control_points()
        .iter()
        .map(|p| p.weight())
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
        .ok_or(Error::InvalidOutput)?;
    let minimum = source
        .control_points()
        .iter()
        .map(|p| p.weight() / reference)
        .fold(1., f64::min);
    if minimum < 1e-5 {
        return Ok(None);
    }
    let weight_scale = if minimum >= 0.001 {
        1.
    } else {
        0.001 / minimum
    };
    let mut points = Vec::with_capacity(source.control_points().len());
    for p in source.control_points() {
        let weight = (p.weight() / reference) * weight_scale;
        if !(0.001..=100.).contains(&weight) {
            return Ok(None);
        }
        let q = p.point();
        let xyz = [
            (q.x() - origin.x()) / scale,
            (q.y() - origin.y()) / scale,
            (q.z() - origin.z()) / scale,
        ];
        if xyz.iter().any(|p| !p.is_finite()) {
            return Ok(None);
        }
        points.push(WeightedPoint3::try_new(Point3::try_from(xyz)?, weight)?);
    }
    let prepared = NurbsCurve::try_new_rational(source.degree(), points, source.knots().to_vec())?
        .try_reparameterized(0. ..=1.)?;
    if source
        .knots()
        .windows(2)
        .zip(prepared.knots().windows(2))
        .any(|(a, b)| a[0] < a[1] && b[0] >= b[1])
    {
        return Ok(None);
    }
    Ok(Some(prepared))
}
