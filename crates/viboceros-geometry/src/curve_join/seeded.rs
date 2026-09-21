//! One-pass individual-pick connectivity, without a global candidate graph.
use super::*;

pub(super) fn connect(
    curves: &[Curve3],
    endpoints: &[Endpoint],
    ends: &[Option<[usize; 2]>],
    options: CurveJoinOptions,
) -> Result<SeededConnections, GeometryError> {
    let mut connections = SeededConnections {
        partners: vec![None; endpoints.len()],
        closing_edge: None,
    };
    let Some((seed, mut free)) = ends
        .iter()
        .enumerate()
        .find_map(|(source, ends)| ends.map(|ends| (source, ends)))
    else {
        return Ok(connections);
    };
    let mut linear_vertices = assembly::linear_vertex_count(curves[seed].as_ref());
    // Source order precedes distance and tangent rank. A rejected source is
    // never reconsidered, and an unrelated second chain is never started.
    for (source, source_ends) in ends.iter().enumerate().skip(seed + 1) {
        let Some(source_ends) = source_ends else {
            continue;
        };
        let mut sides: [Option<(Candidate, usize)>; 2] = [None, None];
        for side in 0..2 {
            for &other in source_ends {
                if let Some(candidate) = search::candidate(endpoints, free[side], other, options)?
                    && sides[side].is_none_or(|(previous, _)| candidate.compare(&previous).is_lt())
                {
                    sides[side] = Some((candidate, other));
                }
            }
        }
        let side = match sides {
            [None, None] => continue,
            [Some(_), None] => 0,
            [None, Some(_)] => 1,
            [Some(left), Some(right)] => {
                if left.1 != right.1 {
                    // A source matching both free ends has the command's
                    // representation-dependent prepend/append closure policy.
                    usize::from(
                        (is_linear(&curves[source])
                            && !endpoint_is_linear(
                                &curves[endpoints[free[0]].curve],
                                endpoints[free[0]].start,
                            )
                            && endpoint_is_linear(
                                &curves[endpoints[free[1]].curve],
                                endpoints[free[1]].start,
                            ))
                            || (matches!(curves[source], Curve3::Arc(_))
                                && linear_vertices.is_some_and(|n| n > 2)),
                    )
                } else {
                    usize::from(left.0.compare(&right.0).is_gt())
                }
            }
        };
        let (_, other) = sides[side].expect("chosen side has a candidate");
        connections.partners[free[side]] = Some(other);
        connections.partners[other] = Some(free[side]);
        linear_vertices = linear_vertices
            .zip(assembly::linear_vertex_count(curves[source].as_ref()))
            .and_then(|(a, b)| a.checked_add(b - 1));
        free[side] = source_ends[usize::from(endpoints[other].start)];
        if search::candidate(endpoints, free[0], free[1], options)?.is_some() {
            connections.partners[free[0]] = Some(free[1]);
            connections.partners[free[1]] = Some(free[0]);
            connections.closing_edge = Some(free);
            break;
        }
    }
    Ok(connections)
}
