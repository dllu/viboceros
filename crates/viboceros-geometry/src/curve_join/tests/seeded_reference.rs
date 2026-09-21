// Literal prior graph traversal, retained only as an independent regression reference.
use super::*;

#[test]
fn one_pass_seeded_connections_match_an_exhaustive_graph_reference() {
    use crate::CurveSegment3;
    let mut state = 0x2026_0920_5eed_0001u64;
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for case in 0..1024 {
        let mut curves = Vec::new();
        for _ in 0..case % 23 {
            let a = (random() % 9) as usize;
            let b = (a + 1 + (random() % 8) as usize) % 9;
            let a = p((a % 3) as Real, (a / 3) as Real);
            let b = p((b % 3) as Real, (b / 3) as Real);
            let mid = p(
                (a.x() + b.x()) / 2.0 + (b.y() - a.y()) / 4.0,
                (a.y() + b.y()) / 2.0 - (b.x() - a.x()) / 4.0,
            );
            let curve = match random() % 7 {
                0 => Curve3::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap()),
                1 => {
                    Curve3::NurbsCurve(NurbsCurve::try_clamped_uniform(2, vec![a, mid, b]).unwrap())
                }
                2 => Curve3::Polyline(
                    Polyline3::try_new(vec![a, mid, b], Tolerance::DEFAULT).unwrap(),
                ),
                3 => Curve3::PolyCurve(
                    PolyCurve3::try_new(vec![
                        CurveSegment3::NurbsCurve(
                            NurbsCurve::try_clamped_uniform(2, vec![a, a, mid]).unwrap(),
                        ),
                        CurveSegment3::Line(
                            LineSegment::try_new(mid, b, Tolerance::DEFAULT).unwrap(),
                        ),
                    ])
                    .unwrap(),
                ),
                4 => Curve3::Arc(
                    CircularArc3::try_from_three_points(a, mid, b, Tolerance::DEFAULT).unwrap(),
                ),
                5 => Curve3::Polyline(
                    Polyline3::try_new(vec![a, mid, b, a], Tolerance::DEFAULT).unwrap(),
                ),
                _ => Curve3::NurbsCurve(NurbsCurve::try_clamped_uniform(2, vec![a, a, b]).unwrap()),
            };
            curves.push(if random() & 1 == 0 {
                curve
            } else {
                curve.reversed(Tolerance::DEFAULT).unwrap()
            });
        }
        let mut ends = vec![None; curves.len()];
        let mut endpoints = Vec::new();
        for (source, curve) in curves.iter().enumerate() {
            let curve = curve.as_ref();
            if curve.is_closed().unwrap() {
                continue;
            }
            ends[source] = Some([endpoints.len(), endpoints.len() + 1]);
            for start in [true, false] {
                endpoints.push(Endpoint {
                    curve: source,
                    start,
                    point: if start {
                        curve.start_point()
                    } else {
                        curve.end_point()
                    }
                    .unwrap(),
                    outward_tangent: endpoint_tangent(curve, start).unwrap(),
                });
            }
        }
        for preserve_direction in [false, true] {
            for tolerance in [0.0, 0.01, 0.5, 1.5] {
                let options = CurveJoinOptions {
                    tolerance,
                    preserve_direction,
                    style: CurveJoinStyle::Seeded,
                };
                // Independently enumerate and rank the complete endpoint graph.
                let mut candidates = Vec::new();
                for (left, a) in endpoints.iter().enumerate() {
                    for (right, b) in endpoints.iter().enumerate().skip(left + 1) {
                        if a.curve == b.curve || (preserve_direction && a.start == b.start) {
                            continue;
                        }
                        let distance = (a.point.x() - b.point.x())
                            .hypot(a.point.y() - b.point.y())
                            .hypot(a.point.z() - b.point.z());
                        if distance <= tolerance {
                            let tangent_dot = match (a.outward_tangent, b.outward_tangent) {
                                (Some(a), Some(b)) => a.as_vector().dot(b.as_vector()).unwrap(),
                                _ => 1.0,
                            };
                            candidates.push(Candidate {
                                distance,
                                tangent_dot,
                                left,
                                right,
                            });
                        }
                    }
                }
                candidates.sort_by(|a, b| {
                    a.distance
                        .total_cmp(&b.distance)
                        .then_with(|| a.tangent_dot.total_cmp(&b.tangent_dot))
                        .then_with(|| (a.left, a.right).cmp(&(b.left, b.right)))
                });
                let expected = reference_partners(&curves, &endpoints, &ends, &candidates).unwrap();
                let actual =
                    crate::curve_join::seeded::connect(&curves, &endpoints, &ends, options)
                        .unwrap();
                assert_eq!(
                    actual.partners, expected.partners,
                    "case {case}, {options:?}"
                );
                assert_eq!(
                    actual.closing_edge, expected.closing_edge,
                    "case {case}, {options:?}"
                );
            }
        }
    }
}

fn reference_partners(
    curves: &[Curve3],
    endpoints: &[Endpoint],
    ends: &[Option<[usize; 2]>],
    candidates: &[Candidate],
) -> Result<SeededConnections, GeometryError> {
    let mut adjacent = vec![Vec::new(); endpoints.len()];
    for (index, candidate) in candidates.iter().enumerate() {
        adjacent[candidate.left].push(index);
        adjacent[candidate.right].push(index);
    }
    let mut partners = vec![None; endpoints.len()];
    let mut assigned = vec![false; ends.len()];
    let mut scans = 0;
    let mut closing_edge = None;
    if let Some((seed, Some(mut free))) = ends
        .iter()
        .copied()
        .enumerate()
        .find(|(_, ends)| ends.is_some())
    {
        assigned[seed] = true;
        let mut last_source = seed;
        let mut linear_vertices = assembly::linear_vertex_count(curves[seed].as_ref());
        loop {
            let mut sides: [Option<(usize, usize, usize, usize)>; 2] = [None, None];
            for side in 0..2 {
                for &index in &adjacent[free[side]] {
                    scans += 1;
                    if scans > MAX_JOIN_SCANS {
                        return Err(GeometryError::CurveJoinLimit {
                            resource: "seeded candidate scans",
                            maximum: MAX_JOIN_SCANS,
                        });
                    }
                    let candidate = candidates[index];
                    let other = if candidate.left == free[side] {
                        candidate.right
                    } else {
                        candidate.left
                    };
                    let source = endpoints[other].curve;
                    if source <= last_source || assigned[source] {
                        continue;
                    }
                    // Candidates already have distance/tangent order. Source
                    // order takes precedence in a seeded one-pass extension.
                    let key = (source, index, side, other);
                    if sides[side].is_none_or(|previous| key < previous) {
                        sides[side] = Some(key);
                    }
                }
            }
            let mut best = sides.into_iter().flatten().min();
            if let [Some(left), Some(right)] = sides
                && left.0 == right.0
                && left.3 != right.3
            {
                // A curve closing both free ends is prepended by individual
                // Join picking. Copy commands may restore the seed seam later.
                best = Some(
                    if (is_linear(&curves[left.0])
                        && !endpoint_is_linear(
                            &curves[endpoints[free[0]].curve],
                            endpoints[free[0]].start,
                        )
                        && endpoint_is_linear(
                            &curves[endpoints[free[1]].curve],
                            endpoints[free[1]].start,
                        ))
                        || (matches!(curves[left.0], Curve3::Arc(_))
                            && linear_vertices.is_some_and(|n| n > 2))
                    {
                        right
                    } else {
                        left
                    },
                );
            }
            let Some((source, _, side, other)) = best else {
                break;
            };
            partners[free[side]] = Some(other);
            partners[other] = Some(free[side]);
            assigned[source] = true;
            linear_vertices = linear_vertices
                .zip(assembly::linear_vertex_count(curves[source].as_ref()))
                .and_then(|(a, b)| a.checked_add(b - 1));
            last_source = source;
            free[side] = ends[source].expect("open source")[usize::from(endpoints[other].start)];
            if adjacent[free[0]].iter().any(|&index| {
                let candidate = candidates[index];
                candidate.left == free[1] || candidate.right == free[1]
            }) {
                partners[free[0]] = Some(free[1]);
                partners[free[1]] = Some(free[0]);
                closing_edge = Some(free);
                break;
            }
        }
    }
    Ok(SeededConnections {
        partners,
        closing_edge,
    })
}
