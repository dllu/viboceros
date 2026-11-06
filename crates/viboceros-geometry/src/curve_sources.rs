//! Locate positive source intervals on staged curve boundaries.
use crate::curve_hull_index::HullIndex;
use crate::{CurveCurveIntersectionEvent, GeometryError, NurbsCurve, Tolerance};

/// Original curve indices contributing a nonzero overlap to each boundary.
/// Point contacts are excluded. The spatial index is conservative; interval
/// events also require a checked closest-point witness at model tolerance.
pub fn curve_overlap_sources(
    boundaries: &[NurbsCurve],
    sources: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<Vec<Vec<usize>>, GeometryError> {
    const MAX: usize = 1_000_000;
    if boundaries.is_empty() {
        return Ok(Vec::new());
    }
    if boundaries.len() > 100_000 || sources.len() > 100_000 {
        return Err(GeometryError::CurveJoinLimit {
            resource: "input curves",
            maximum: 100_000,
        });
    }
    let mut index = HullIndex::default();
    for source in sources {
        index.insert(HullIndex::hull(source));
    }
    let mut work = 0usize;
    let mut result = Vec::with_capacity(boundaries.len());
    for boundary in boundaries {
        let mut members = Vec::new();
        for i in index.candidates(HullIndex::hull(boundary), tolerance) {
            work += 1;
            if work > MAX {
                return Err(GeometryError::CurveJoinLimit {
                    resource: "boundary/source comparisons",
                    maximum: MAX,
                });
            }
            for event in boundary.intersection_events_with_curve(&sources[i], tolerance)? {
                let CurveCurveIntersectionEvent::Overlap(overlap) = event else {
                    continue;
                };
                let interval = overlap.first_interval();
                if interval.start() >= interval.end() {
                    continue;
                }
                let t =
                    crate::interpolate_scalar([*interval.start(), *interval.end()], [0., 1.], 0.5)?;
                let point = boundary.evaluate(t)?;
                let s = sources[i].closest_parameter(point, tolerance)?;
                if point.distance_to(sources[i].evaluate(s)?)? <= tolerance.absolute() {
                    members.push(i);
                    break;
                }
            }
        }
        result.push(members);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point3;
    fn curve(points: &[[f64; 3]]) -> NurbsCurve {
        let n = points.len();
        NurbsCurve::try_new(
            1,
            points
                .iter()
                .map(|&p| Point3::try_from(p).unwrap())
                .collect(),
            std::iter::once(0.)
                .chain((0..n).map(|i| i as f64))
                .chain(std::iter::once((n - 1) as f64))
                .collect(),
        )
        .unwrap()
    }
    #[test]
    fn overlaps_include_partial_aliases_and_exclude_point_contacts() {
        let boundary = curve(&[
            [0., 0., 0.],
            [10., 0., 0.],
            [10., 10., 0.],
            [0., 10., 0.],
            [0., 0., 0.],
        ]);
        let sources = [
            curve(&[[2., 0., 0.], [8., 0., 0.]]),
            curve(&[[8., 0., 0.], [2., 0., 0.]]),
            curve(&[[-2., 0., 0.], [0., 0., 0.]]),
            curve(&[[0., 0., 0.], [10., 10., 0.]]),
        ];
        assert_eq!(
            curve_overlap_sources(&[boundary], &sources, Tolerance::DEFAULT).unwrap(),
            vec![vec![0, 1]]
        );
    }
}
