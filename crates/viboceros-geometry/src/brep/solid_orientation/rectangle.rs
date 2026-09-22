//! Exact trim containment for four axis-aligned linear boundary curves.
use super::*;

pub(super) fn bounds(face: &BrepFace) -> Option<[[Real; 2]; 2]> {
    if face.loops.len() != 1 || face.loops[0].trims.len() != 4 {
        return None;
    }
    let mut ends = Vec::with_capacity(4);
    for trim in &face.loops[0].trims {
        let curve = &trim.curve;
        let controls = curve.control_points();
        let knots = curve.knots();
        if curve.degree() != 1
            || knots[0] != knots[1]
            || knots[knots.len() - 1] != knots[knots.len() - 2]
            || knots[1..knots.len() - 1]
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return None;
        }
        let sign = controls[0].weight().is_sign_positive();
        if controls
            .iter()
            .any(|c| c.weight().is_sign_positive() != sign)
        {
            return None;
        }
        let a = controls.first()?.point().to_array();
        let b = controls.last()?.point().to_array();
        let axis = if a[0] == b[0] && a[1] != b[1] {
            0
        } else if a[1] == b[1] && a[0] != b[0] {
            1
        } else {
            return None;
        };
        if controls.iter().any(|c| {
            let p = c.point().to_array();
            p[axis] != a[axis]
                || p[1 - axis] < a[1 - axis].min(b[1 - axis])
                || p[1 - axis] > a[1 - axis].max(b[1 - axis])
        }) {
            return None;
        }
        ends.push([a, b]);
    }
    if (0..4).any(|i| ends[i][1] != ends[(i + 1) % 4][0]) {
        return None;
    }
    let limits: [[Real; 2]; 2] = std::array::from_fn(|axis| {
        [
            ends.iter()
                .map(|e| e[0][axis])
                .fold(Real::INFINITY, Real::min),
            ends.iter()
                .map(|e| e[0][axis])
                .fold(Real::NEG_INFINITY, Real::max),
        ]
    });
    let [[u0, u1], [v0, v1]] = limits;
    if !(u0 < u1 && v0 < v1) {
        return None;
    }
    let corners = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    let mut seen = [false; 4];
    for [a, b] in ends {
        let side = (0..4).find(|&i| a == corners[i] && b == corners[(i + 1) % 4])?;
        if seen[side] {
            return None;
        }
        seen[side] = true;
    }
    let domains = [face.surface.domain_u(), face.surface.domain_v()];
    (0..2)
        .all(|i| limits[i][0] >= *domains[i].start() && limits[i][1] <= *domains[i].end())
        .then_some(limits)
}
