//! Recover exact repeated-junction walks without flattening rational segments.
use super::*;
use viboceros_geometry::{CurveSegment3, PolyCurve3};

pub(super) fn contours(
    curve: &PolyCurve3,
    budget: &mut Budget,
) -> Result<Vec<NurbsCurve>, CommandError> {
    let mut positions = BTreeMap::new();
    let mut points = Vec::<Point3>::new();
    let mut segments = Vec::<usize>::new();
    let mut result = Vec::new();
    let key = |p: Point3| p.to_array().map(|x| if x == 0. { 0 } else { x.to_bits() });
    let start = curve.segments()[0].evaluate(*curve.segments()[0].domain().start())?;
    positions.insert(key(start), 0usize);
    points.push(start);
    for (i, segment) in curve.segments().iter().enumerate() {
        budget.work(1)?;
        segments.push(i);
        let end = segment.evaluate(*segment.domain().end())?;
        if let Some(&j) = positions.get(&key(end)) {
            let degree = segments[j..]
                .iter()
                .map(|&k| curve.segments()[k].degree())
                .max()
                .unwrap();
            let count = segments[j..].iter().try_fold(0usize, |n, &k| {
                let leaf = curve.segments()[k].to_nurbs()?;
                Ok::<_, GeometryError>(
                    n + leaf.control_points().len()
                        + leaf.spans().count() * (degree - leaf.degree()),
                )
            })?;
            budget.output(count)?;
            let cycle = PolyCurve3::try_new(
                segments[j..]
                    .iter()
                    .map(|&k| curve.segments()[k].clone())
                    .collect::<Vec<CurveSegment3>>(),
            )?;
            result.push(cycle.to_nurbs()?);
            for p in &points[j + 1..] {
                positions.remove(&key(*p));
            }
            points.truncate(j + 1);
            segments.truncate(j);
        } else {
            positions.insert(key(end), points.len());
            points.push(end);
        }
    }
    Ok(result)
}
