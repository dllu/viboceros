//! No tolerance-based planarity, edge straightness, or trim-closure decisions.
use super::*;

pub(super) fn extract(face: &BrepFace, remaining: &mut usize) -> Option<PolygonFace> {
    let s = &face.surface;
    if s.degree_u() != 1
        || s.degree_v() != 1
        || s.control_point_count_u() != 2
        || s.control_point_count_v() != 2
    {
        return None;
    }
    spend(remaining, 16)?;
    let controls = s.control_points();
    if controls.iter().any(|p| p.weight() != controls[0].weight()) {
        return None;
    }
    for knots in [s.knots_u(), s.knots_v()] {
        if knots[0] != knots[1] || knots[2] != knots[3] {
            return None;
        }
    }
    let p = controls
        .iter()
        .map(|c| point(c.point()))
        .collect::<Vec<_>>();
    let u = sub(&p[1], &p[0]);
    let v = sub(&p[2], &p[0]);
    let twist: ExactPoint = std::array::from_fn(|i| &p[3][i] - &p[1][i] - &p[2][i] + &p[0][i]);
    let u0 = rational(*s.domain_u().start());
    let v0 = rational(*s.domain_v().start());
    let du = rational(*s.domain_u().end()) - &u0;
    let dv = rational(*s.domain_v().end()) - &v0;
    let evaluate = |uv: [Real; 2]| {
        let a = (rational(uv[0]) - &u0) / &du;
        let b = (rational(uv[1]) - &v0) / &dv;
        std::array::from_fn(|i| &p[0][i] + &a * &u[i] + &b * &v[i] + &a * &b * &twist[i])
    };
    if zero(&twist) {
        let normal = cross(&u, &v);
        if zero(&normal) {
            return None;
        }
        let mut loops = Vec::with_capacity(face.loops.len());
        for (index, boundary) in face.loops.iter().enumerate() {
            let uv = linear_loop(boundary, remaining)?;
            let area: Rational = (0..uv.len())
                .map(|i| {
                    let a = uv[i];
                    let b = uv[(i + 1) % uv.len()];
                    rational(a[0]) * rational(b[1]) - rational(a[1]) * rational(b[0])
                })
                .sum();
            if area.is_zero() || area.is_positive() != (index == 0) {
                return None;
            }
            if uv
                .iter()
                .any(|p| !s.domain_u().contains(&p[0]) || !s.domain_v().contains(&p[1]))
            {
                return None;
            }
            loops.push(uv.into_iter().map(evaluate).collect());
        }
        return PolygonFace::new(loops, normal, face.reversed);
    }
    // A convex planar bilinear rectangle is exactly its corner polygon, even
    // with a collapsed side. A general diagonal UV trim is not a straight edge.
    let [[u0, u1], [v0, v1]] = super::super::rectangle::bounds(face, remaining)?;
    spend(remaining, 32)?;
    let mut corners = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]]
        .map(evaluate)
        .to_vec();
    corners.dedup();
    if corners.first() == corners.last() {
        corners.pop();
    }
    if corners.len() < 3 {
        return None;
    }
    let normal = cross(
        &sub(&corners[1], &corners[0]),
        &sub(&corners[2], &corners[0]),
    );
    if zero(&normal)
        || corners
            .iter()
            .any(|p| !dot(&normal, &sub(p, &corners[0])).is_zero())
    {
        return None;
    }
    for i in 0..corners.len() {
        let a = &corners[i];
        let b = &corners[(i + 1) % corners.len()];
        let c = &corners[(i + 2) % corners.len()];
        if !dot(&normal, &cross(&sub(b, a), &sub(c, b))).is_positive() {
            return None;
        }
    }
    PolygonFace::new(vec![corners], normal, face.reversed)
}

fn linear_loop(boundary: &BrepLoop, remaining: &mut usize) -> Option<Vec<[Real; 2]>> {
    let mut polygon = Vec::new();
    let mut previous = None;
    for trim in &boundary.trims {
        let mut points = super::super::trim::polygon(&trim.curve, remaining)?;
        let end = points.next_back()?;
        let start = points.next()?;
        if previous.is_some_and(|p| p != start) {
            return None;
        }
        previous = Some(end);
        polygon.push(start);
        polygon.extend(points);
    }
    if previous != polygon.first().copied() {
        return None;
    }
    polygon.dedup();
    if polygon.first() == polygon.last() {
        polygon.pop();
    }
    (polygon.len() >= 3).then_some(polygon)
}
