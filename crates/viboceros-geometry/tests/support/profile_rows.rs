//! Complete images: certify every profile row under one shared V map.
use num_rational::BigRational as Rational;
use num_traits::{Signed, ToPrimitive, Zero};
use viboceros_geometry::{GeometryError, NurbsCurve, NurbsSurface};

fn rat(x: f64) -> Rational {
    Rational::from_float(x).unwrap()
}
fn normalized_u(s: &NurbsSurface) -> Vec<Rational> {
    let start = rat(*s.domain_u().start());
    let span = rat(*s.domain_u().end()) - &start;
    s.knots_u()
        .iter()
        .map(|x| (rat(*x) - &start) / &span)
        .collect()
}
fn radius(s: &NurbsSurface) -> Rational {
    let origin = s.control_points()[0].point().to_array().map(rat);
    s.control_points()
        .iter()
        .map(|p| {
            p.point()
                .to_array()
                .into_iter()
                .zip(&origin)
                .map(|(x, o)| (rat(x) - o).abs())
                .sum()
        })
        .max()
        .unwrap()
}
struct Rows {
    curves: Vec<NurbsCurve>,
    u_weights: Vec<Rational>,
    radius: Rational,
    weight_error: Rational,
}
fn rows(s: &NurbsSurface) -> Rows {
    let weight = |u, v| rat(s.control_point(u, v).unwrap().weight());
    let origin = weight(0, 0);
    assert!(s.control_points().iter().all(|p| p.weight() > 0.));
    let reference = (0..s.control_point_count_v())
        .map(|v| s.control_point(0, v).unwrap().weight())
        .collect::<Vec<_>>();
    let u_weights = (0..s.control_point_count_u())
        .map(|u| weight(u, 0) / &origin)
        .collect::<Vec<_>>();
    let mut delta = Rational::zero();
    let mut minimum = origin;
    let mut curves = Vec::new();
    for (u, scale) in u_weights.iter().enumerate() {
        let mut points = Vec::new();
        for (v, w) in reference.iter().enumerate() {
            let actual = weight(u, v);
            minimum = minimum.min(actual.clone());
            delta = delta.max((actual - scale * rat(*w)).abs());
            points.push(
                viboceros_geometry::WeightedPoint3::try_new(
                    s.control_point(u, v).unwrap().point(),
                    *w,
                )
                .unwrap(),
            );
        }
        curves.push(
            NurbsCurve::try_new_rational(s.degree_v(), points, s.knots_v().to_vec()).unwrap(),
        );
    }
    let radius = radius(s);
    // Compare to an exactly separable ideal tensor with weights r[u]*w[0,v].
    // Nonnegative tensor bases give |S-I| <= 2*max|w-w_ideal|*R/min(w).
    // All arithmetic, including factorization error from stored floats, is exact.
    let weight_error = Rational::from_integer(2.into()) * delta * &radius / minimum;
    Rows {
        curves,
        u_weights,
        radius,
        weight_error,
    }
}
fn upper(x: &Rational) -> Option<f64> {
    let mut value = x.to_f64()?;
    if !value.is_finite() {
        return None;
    }
    for _ in 0..16 {
        if rat(value) >= *x {
            return Some(value);
        }
        value = value.next_up();
    }
    None
}

pub fn complete_bound(a: &NurbsSurface, b: &NurbsSurface, limit: f64) -> Option<f64> {
    complete_oriented_bound(a, b, limit).or_else(|| {
        // Negation and reordering of stored knots/controls preserve the exact
        // surface image. Rhino may choose the opposite rail direction.
        let reversed = b.try_reversed_u().ok()?;
        complete_oriented_bound(a, &reversed, limit)
    })
}

fn complete_oriented_bound(a: &NurbsSurface, b: &NurbsSurface, limit: f64) -> Option<f64> {
    if !limit.is_finite() || limit <= 0. {
        return None;
    }
    // Every curve certificate receives the original binary knots. Charge U
    // clock changes through complete column proofs before matching V rows.
    if a.degree_u() != b.degree_u() || a.control_point_count_u() != b.control_point_count_u() {
        return None;
    }
    let u_error = u_clock_bound(a, b, limit)?;
    let (a, b) = (rows(a), rows(b));
    let delta = a
        .u_weights
        .iter()
        .zip(&b.u_weights)
        .map(|(a, b)| (a - b).abs())
        .max()
        .unwrap();
    let minimum = a.u_weights.iter().min().unwrap();
    // The ideal tensors' positive rational U coefficients are convex weights.
    // Account for their difference too, rather than silently rounding weights.
    let error = upper(
        &(a.weight_error
            + b.weight_error
            + rat(u_error)
            + Rational::from_integer(2.into()) * delta * &b.radius / minimum),
    )?;
    if error >= limit {
        return None;
    }
    let remaining = (limit - error).next_down();
    let rows = a.curves.iter().zip(&b.curves).collect::<Vec<_>>();
    let squared_limit = rat(remaining).pow(2);
    for (a, b) in &rows {
        let clamped = |c: &NurbsCurve| {
            c.knots()[..=c.degree()]
                .iter()
                .all(|k| k == c.domain().start())
                && c.knots()[c.control_points().len()..]
                    .iter()
                    .all(|k| k == c.domain().end())
        };
        if clamped(a) && clamped(b) {
            for (a, b) in [
                (a.control_points().first()?, b.control_points().first()?),
                (a.control_points().last()?, b.control_points().last()?),
            ] {
                let distance: Rational = a
                    .point()
                    .to_array()
                    .into_iter()
                    .zip(b.point().to_array())
                    .map(|(a, b)| (rat(a) - rat(b)).pow(2))
                    .sum();
                if distance > squared_limit {
                    return None;
                }
            }
        }
    }
    // Poles make some rows constant. Propose a map from the largest control
    // hull, then validate the SAME map on every complete row and knot limit.
    let anchor = rows
        .iter()
        .max_by(|a, b| extent(a.0).total_cmp(&extent(b.0)))
        .unwrap();
    for segments in [1, 32, 64, 128, 256, 512, 1024] {
        let Some(pairs) = correspondence(anchor.0, anchor.1, segments) else {
            continue;
        };
        let mut maximum = 0_f64;
        let mut accepted = true;
        for (a, b) in &rows {
            match a.mapped_curve_deviation_bound(b, &pairs, remaining) {
                Ok(Some(bound)) => maximum = maximum.max(bound),
                Ok(None) | Err(GeometryError::CurveApproximationLimit) => {
                    accepted = false;
                    break;
                }
                Err(e) => panic!("invalid surface-row certificate: {e}"),
            }
        }
        if accepted {
            let bound = (maximum + error).next_up();
            return (bound <= limit).then_some(bound);
        }
    }
    None
}

fn u_clock_bound(a: &NurbsSurface, b: &NurbsSurface, limit: f64) -> Option<f64> {
    if normalized_u(a) == normalized_u(b) {
        return Some(0.);
    }
    let mut maximum = 0_f64;
    for v in 0..b.control_point_count_v() {
        // The ideal separable tensor uses weights r[u]*w[v]. Its U curves
        // have the original first-column weights up to a common gauge.
        let controls = (0..b.control_point_count_u())
            .map(|u| {
                viboceros_geometry::WeightedPoint3::try_new(
                    b.control_point(u, v).unwrap().point(),
                    b.control_point(u, 0).unwrap().weight(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let original =
            NurbsCurve::try_new_rational(b.degree_u(), controls.clone(), b.knots_u().to_vec())
                .ok()?;
        let retimed =
            NurbsCurve::try_new_rational(b.degree_u(), controls, a.knots_u().to_vec()).ok()?;
        let bound = match original.parameter_curve_deviation_bound(&retimed, limit) {
            Ok(Some(bound)) => bound,
            Ok(None) | Err(GeometryError::CurveApproximationLimit) => return None,
            Err(e) => panic!("invalid U-clock certificate: {e}"),
        };
        maximum = maximum.max(bound);
    }
    // The positive rational V coefficients are convex weights. The maximum
    // of these complete U-column bounds covers the entire ideal tensor.
    Some(maximum)
}
fn correspondence(a: &NurbsCurve, b: &NurbsCurve, segments: usize) -> Option<Vec<[f64; 2]>> {
    let mut pairs = vec![[0., 0.]];
    for i in 1..segments {
        let f = i as f64 / segments as f64;
        let target = b.evaluate(b.parameter_at(f).unwrap()).unwrap();
        let mut t = a.parameter_at(f).unwrap();
        for _ in 0..8 {
            let (p, d) = a.evaluate_with_derivative(t).unwrap();
            let norm = d.dot(d).unwrap();
            if norm == 0. {
                return None;
            }
            let step = p.vector_to(target).unwrap().dot(d).unwrap() / norm;
            let domain = a.domain();
            let next = (t + step).clamp(*domain.start(), *domain.end());
            if next == t {
                break;
            }
            t = next;
        }
        let domain = a.domain();
        pairs.push([(t - domain.start()) / (domain.end() - domain.start()), f]);
    }
    pairs.push([1., 1.]);
    pairs.windows(2).all(|p| p[0][0] < p[1][0]).then_some(pairs)
}
fn extent(c: &NurbsCurve) -> f64 {
    let origin = c.control_points()[0].point();
    c.control_points()
        .iter()
        .map(|p| origin.distance_to(p.point()).unwrap())
        .fold(0., f64::max)
}
