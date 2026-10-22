//! Preserve singular-side topology and explicit closed seam rows.
use super::*;

pub(super) fn insert_pole_rows(
    points: &[Point3],
    counts: [usize; 2],
    axis: usize,
    start: bool,
    end: bool,
) -> Result<Vec<Point3>, GeometryError> {
    let n = counts[axis];
    let mut plan = Vec::new();
    for i in 0..n {
        plan.push((i, i));
        if i == 0 && start {
            plan.push((0, 1));
        }
        if end && i == n - 2 {
            plan.push((n - 1, n - 2));
        }
    }
    let mut dimensions = counts;
    dimensions[axis] = plan.len();
    let mut output = Vec::with_capacity(dimensions[0] * dimensions[1]);
    for v in 0..dimensions[1] {
        for u in 0..dimensions[0] {
            let (a, b) = plan[if axis == 0 { u } else { v }];
            let index = |i| {
                if axis == 0 {
                    v * counts[0] + i
                } else {
                    i * counts[0] + u
                }
            };
            let p = points[index(a)];
            output.push(if a == b {
                p
            } else {
                let x = p.to_array();
                let y = points[index(b)].to_array();
                Point3::try_from(std::array::from_fn::<_, 3, _>(|c| {
                    x[c] * (2. / 3.) + y[c] / 3.
                }))?
            });
        }
    }
    Ok(output)
}
pub(super) fn append_seam(points: &[Point3], counts: [usize; 2], axis: usize) -> Vec<Point3> {
    let mut out = counts;
    out[axis] += 1;
    (0..out[1])
        .flat_map(|v| (0..out[0]).map(move |u| points[(v % counts[1]) * counts[0] + u % counts[0]]))
        .collect()
}
