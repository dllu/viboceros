//! Subdivide query domains without rounding or replacing the source chord.
use super::*;

const TARGET_CANDIDATES: usize = 64;
const MAX_PARTITION_DEPTH: usize = 16;
struct Part {
    range: [Rational; 2],
    candidates: Vec<usize>,
    depth: usize,
}

pub(super) fn trace(
    index: &MeshProjectionIndex<'_>,
    source: [&E; 2],
    candidates: Vec<usize>,
    fallback: Point3,
    tolerance: f64,
    output: &mut Vec<Point3>,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let [origin, delta] = source;
    let mut pending = vec![Part {
        range: [Rational::zero(), Rational::one()],
        candidates,
        depth: 0,
    }];
    while let Some(part) = pending.pop() {
        if part.candidates.len() > TARGET_CANDIDATES && part.depth < MAX_PARTITION_DEPTH {
            let middle = (&part.range[0] + &part.range[1]) / rational(2.);
            let left = [part.range[0].clone(), middle.clone()];
            let right = [middle, part.range[1].clone()];
            if let (Some(a), Some(b)) = (
                candidates_for(index, origin, delta, &left, fallback, budget)?,
                candidates_for(index, origin, delta, &right, fallback, budget)?,
            ) {
                // Duplicate/coincident sheets need not benefit. Keep the full
                // exact path unless the largest local candidate set shrinks.
                if a.len().max(b.len()) < part.candidates.len() {
                    pending.push(Part {
                        range: right,
                        candidates: b,
                        depth: part.depth + 1,
                    });
                    pending.push(Part {
                        range: left,
                        candidates: a,
                        depth: part.depth + 1,
                    });
                    continue;
                }
            }
        }
        let mut features = Vec::new();
        for triangle in part.candidates {
            feature::append(
                index.triangle_points(triangle).unwrap(),
                origin,
                delta,
                &mut features,
                budget,
            )?;
        }
        let mut cuts = part.range.to_vec();
        for f in &mut features {
            f.range[0] = f.range[0].clone().max(part.range[0].clone());
            f.range[1] = f.range[1].clone().min(part.range[1].clone());
            if f.range[0] < f.range[1] {
                cuts.extend(f.range.iter().cloned());
            }
        }
        budget.charge(cuts.len())?;
        cuts.sort();
        cuts.dedup();
        for interval in cuts.windows(2) {
            budget.charge(features.len())?;
            let middle = (&interval[0] + &interval[1]) / rational(2.);
            let active = features
                .iter()
                .filter(|f| f.range[0] < middle && middle < f.range[1])
                .collect::<Vec<_>>();
            if active.is_empty() {
                return Err(GeometryError::AmbiguousMeshPull);
            }
            super::trace(
                &active,
                interval[0].clone(),
                interval[1].clone(),
                tolerance,
                output,
                budget,
            )?;
        }
    }
    Ok(())
}

fn candidates_for(
    index: &MeshProjectionIndex<'_>,
    origin: &E,
    delta: &E,
    range: &[Rational; 2],
    fallback: Point3,
    budget: &mut Budget,
) -> Result<Option<Vec<usize>>, GeometryError> {
    let mut ends = Vec::new();
    for t in range {
        let point: E = std::array::from_fn(|i| &origin[i] + t * &delta[i]);
        budget.check(&point)?;
        let Some(intervals) = point.iter().map(I::exact).collect::<Option<Vec<_>>>() else {
            return Ok(None);
        };
        ends.push(BoundingBox3::from_points([
            Point3::try_from(std::array::from_fn::<_, 3, _>(|i| intervals[i].lo))?,
            Point3::try_from(std::array::from_fn::<_, 3, _>(|i| intervals[i].hi))?,
        ])?);
    }
    let query = ends[0].min();
    // This query only chooses a feasible vertex, never admits geometry. Its
    // rounded location, or a query failure, cannot invalidate the bound.
    let foot = index
        .closest_location(query)
        .ok()
        .flatten()
        .and_then(|location| index.triangle_points(location.triangle_index))
        .and_then(|triangle| {
            triangle
                .into_iter()
                .min_by(|&a, &b| query.compare_distances(a, b))
        })
        .unwrap_or(fallback);
    let first = farthest(ends[0], foot)?;
    let second = farthest(ends[1], foot)?;
    let upper = if foot.compare_distances(first, second).is_gt() {
        first
    } else {
        second
    };
    Ok(Some(index.pull_candidates_in_box(
        ends[0].union(ends[1])?,
        [foot, upper],
        budget,
    )?))
}
fn farthest(bounds: BoundingBox3, foot: Point3) -> Result<Point3, GeometryError> {
    let mut corner = foot.to_array();
    for (axis, value) in corner.iter_mut().enumerate() {
        let mut a = foot.to_array();
        let mut b = a;
        a[axis] = bounds.min().to_array()[axis];
        b[axis] = bounds.max().to_array()[axis];
        *value = if foot
            .compare_distances(Point3::try_from(a)?, Point3::try_from(b)?)
            .is_gt()
        {
            a[axis]
        } else {
            b[axis]
        };
    }
    Point3::try_from(corner)
}
