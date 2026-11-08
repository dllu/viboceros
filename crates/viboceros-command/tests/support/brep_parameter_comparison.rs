//! Direct checks for matching affine tensor charts.
use super::*;

// Equal original tensor charts admit a direct parameter comparison. Check both
// retained UV sets as well as boundary stations; other charts use closest points.
pub(super) fn error(a: &BrepFace, b: &BrepFace, t: Tolerance) -> Option<f64> {
    let s = a.surface();
    let q = b.surface();
    let normalized = |knots: &[f64], domain: std::ops::RangeInclusive<f64>| {
        knots[1..knots.len() - 1]
            .iter()
            .map(|&k| (k - domain.start()) / (domain.end() - domain.start()))
            .collect::<Vec<_>>()
    };
    let matching = |a: Vec<f64>, b: Vec<f64>| {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| (a - b).abs() <= 64. * f64::EPSILON)
    };
    if s.degree_u() != q.degree_u()
        || s.degree_v() != q.degree_v()
        || !matching(
            normalized(s.knots_u(), s.domain_u()),
            normalized(q.knots_u(), q.domain_u()),
        )
        || !matching(
            normalized(s.knots_v(), s.domain_v()),
            normalized(q.knots_v(), q.domain_v()),
        )
    {
        return None;
    }
    let map = |p: [f64; 2],
               from: &viboceros_geometry::NurbsSurface,
               to: &viboceros_geometry::NurbsSurface| {
        std::array::from_fn::<_, 2, _>(|i| {
            let a = [from.domain_u(), from.domain_v()][i].clone();
            let b = [to.domain_u(), to.domain_v()][i].clone();
            (*b.start() + (*b.end() - *b.start()) * ((p[i] - a.start()) / (a.end() - a.start())))
                .clamp(*b.start(), *b.end())
        })
    };
    let mut uv = Vec::new();
    for i in 0..=8 {
        for j in 0..=8 {
            uv.push([
                s.parameter_at_u(i as f64 / 8.).unwrap(),
                s.parameter_at_v(j as f64 / 8.).unwrap(),
            ]);
        }
    }
    for face in [a, b] {
        for trim in face.loops().iter().flat_map(|l| l.trims()) {
            for i in 0..=16 {
                let c = trim.curve();
                let p = c.evaluate(c.parameter_at(i as f64 / 16.).unwrap()).unwrap();
                uv.push(map([p.x(), p.y()], face.surface(), s));
            }
        }
    }
    let mut error = 0_f64;
    for [u, v] in uv {
        let [x, y] = map([u, v], s, q);
        let retained = a.contains_parameters(u, v, t).unwrap();
        if retained != b.contains_parameters(x, y, t).unwrap() {
            return None;
        }
        if !retained {
            continue;
        }
        let p = s.evaluate(u, v).unwrap();
        let r = q.evaluate(x, y).unwrap();
        error = error.max(p.distance_to(r).unwrap());
        let singular = [a, b].iter().any(|f| {
            f.loops()
                .iter()
                .flat_map(|l| l.trims())
                .filter(|tr| tr.trim_type() == BrepTrimType::Singular)
                .any(|tr| {
                    let at = tr.curve().start_point().unwrap();
                    let pole = f.surface().evaluate(at.x(), at.y()).unwrap();
                    let scale = p.to_array().into_iter().map(f64::abs).fold(1_f64, f64::max);
                    pole.distance_to(p).unwrap() <= 64. * f64::EPSILON * scale
                })
        });
        if !singular && let (Ok(n), Ok(m)) = (s.normal_at(u, v), q.normal_at(x, y)) {
            let n = n
                .as_vector()
                .scaled(if a.is_reversed() { -1. } else { 1. })
                .unwrap();
            let m = m
                .as_vector()
                .scaled(if b.is_reversed() { -1. } else { 1. })
                .unwrap();
            error = error.max(
                Point3::try_from(n.to_array())
                    .unwrap()
                    .distance_to(Point3::try_from(m.to_array()).unwrap())
                    .unwrap(),
            );
        }
    }
    Some(error)
}
