//! Natural-span proposals for coarse closed fits; final certificates still apply.
use super::*;

// A coarse closed fit retains the source's bounded span layout. Otherwise
// a single endpoint-Hermite interval could collapse an entire closed loop.
pub(super) fn control_hull_within(source: &NurbsCurve, tolerance: Real) -> bool {
    let bounds = source.control_point_bounds();
    bounds
        .min()
        .distance_to(bounds.max())
        .is_ok_and(|diameter| tolerance >= diameter)
}

pub(super) fn span_intervals(
    source: &NurbsCurve,
    lookup: &[Real],
) -> Result<Vec<Interval>, GeometryError> {
    let cuts = source.knots()[source.degree()..=source.control_points().len()]
        .chunk_by(|a, b| a == b)
        .map(|g| g[0])
        .collect::<Vec<_>>();
    let fraction = |parameter| -> Result<Real, GeometryError> {
        let i = lookup
            .partition_point(|t| *t <= parameter)
            .saturating_sub(1)
            .min(7);
        Ok(
            (i as Real + crate::nurbs::interval_fraction(parameter, lookup[i], lookup[i + 1])?)
                / 8.,
        )
    };
    cuts.windows(2)
        .rev()
        .map(|p| {
            Ok(Interval {
                start: p[0],
                end: p[1],
                from: fraction(p[0])?,
                to: fraction(p[1])?,
                depth: 0,
            })
        })
        .collect()
}
