//! Positive Bezier bases make strict supporting-halfspace tests conclusive.
use super::*;

pub(in crate::brep::curve_network) fn only_shared_ends(
    first: &SurfaceCutArrangementCurve,
    second: &SurfaceCutArrangementCurve,
    tolerance: Tolerance,
) -> bool {
    let a = &first.spatial;
    let b = &second.spatial;
    let nodes = [first, second].map(|c| {
        [*c.spatial.domain().start(), *c.spatial.domain().end()]
            .map(|t| c.breaks.iter().find(|s| s.parameter == t).map(|s| s.node))
    });
    let bezier = |c: &NurbsCurve| {
        let p = c.degree();
        let controls = c.control_points();
        let domain = c.domain();
        controls.len() == p + 1
            && controls
                .iter()
                .all(|q| q.weight().is_sign_positive() == controls[0].weight().is_sign_positive())
            && c.knots()[..=p].iter().all(|k| *k == *domain.start())
            && c.knots()[p + 1..].iter().all(|k| *k == *domain.end())
    };
    if !bezier(a) || !bezier(b) {
        return false;
    }
    let hulls = [a.control_point_bounds(), b.control_point_bounds()];
    let ends = |c: &NurbsCurve| {
        [
            c.control_points()[0].point(),
            c.control_points().last().unwrap().point(),
        ]
    };
    let endpoints = [ends(a), ends(b)];
    for axis in 0..3 {
        let plane = if hulls[0].max().to_array()[axis] == hulls[1].min().to_array()[axis] {
            hulls[0].max().to_array()[axis]
        } else if hulls[1].max().to_array()[axis] == hulls[0].min().to_array()[axis] {
            hulls[1].max().to_array()[axis]
        } else {
            continue;
        };
        let leaves = [a, b].map(|c| {
            c.control_points()
                .iter()
                .any(|q| q.point().to_array()[axis] != plane)
        });
        if !leaves[0] && !leaves[1] {
            continue;
        }
        let mut certified = true;
        for i in 0..2 {
            if !leaves[i] {
                continue;
            }
            for (end, endpoint) in endpoints[i].into_iter().enumerate() {
                if endpoint.to_array()[axis] != plane {
                    continue;
                }
                let inside = endpoint
                    .to_array()
                    .into_iter()
                    .zip(hulls[1 - i].min().to_array())
                    .zip(hulls[1 - i].max().to_array())
                    .all(|((x, lo), hi)| x >= lo && x <= hi);
                let shared = endpoints[1 - i].iter().enumerate().any(|(other, q)| {
                    nodes[i][end].is_some()
                        && nodes[i][end] == nodes[1 - i][other]
                        && endpoint
                            .distance_to(*q)
                            .is_ok_and(|d| d <= tolerance.absolute())
                });
                if inside && !shared {
                    certified = false;
                }
            }
        }
        if certified {
            return true;
        }
    }
    false
}
