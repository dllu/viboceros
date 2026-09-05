use super::*;
use crate::nurbs::stable_knot_mean;

pub(super) fn check_count(curve: &NurbsCurve, degree: usize) -> Result<(), GeometryError> {
    let count = degree
        + 1
        + curve
            .interior_knot_groups()
            .iter()
            .map(|(_, m)| m + degree - curve.degree())
            .sum::<usize>();
    if count > MAX_EDGE_CONTROLS {
        return Err(GeometryError::EdgeSurfaceResourceLimit {
            maximum: MAX_EDGE_CONTROLS,
        });
    }
    Ok(())
}

pub(super) fn normalized(curve: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    let scale = curve
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max);
    NurbsCurve::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), c.weight() / scale))
            .collect::<Result<Vec<_>, _>>()?,
        curve.knots().to_vec(),
    )?
    .try_normalized_end_weights()
}

pub(super) fn compatible(a: &NurbsCurve, b: &NurbsCurve) -> Result<[NurbsCurve; 2], GeometryError> {
    let degree = a.degree().max(b.degree());
    check_count(a, degree)?;
    check_count(b, degree)?;
    let da = a.domain();
    let db = b.domain();
    let domain = if da.end() - da.start() >= db.end() - db.start() {
        da
    } else {
        db
    };
    let mut curves = [a.clone(), b.clone()];
    for curve in &mut curves {
        *curve = curve
            .clamped_to_active_domain()?
            .try_change_degree(degree, false)?
            .try_reparameterized(domain.clone())?;
    }
    // OpenNURBS pair matching coalesces a near-coincident next knot to the
    // smaller value. Keep this policy separate from Loft's exact knot union.
    let groups = curves.each_ref().map(|c| c.interior_knot_groups());
    let mut mapped = groups.clone();
    let mut positions = [0, 0];
    let mut union = Vec::<(Real, usize)>::new();
    let mut previous = *domain.start();
    while positions[0] < groups[0].len() || positions[1] < groups[1].len() {
        let next =
            std::array::from_fn::<_, 2, _>(|axis| groups[axis].get(positions[axis]).copied());
        let (knot, multiplicity, consume) = match next {
            [Some(a), Some(b)] => {
                let high = a.0.max(b.0);
                let epsilon =
                    Real::EPSILON.sqrt() * ((high - previous).abs() + previous.abs() + high.abs());
                if (a.0 - b.0).abs() <= epsilon {
                    (a.0.min(b.0), a.1.max(b.1), [true, true])
                } else if a.0 < b.0 {
                    (a.0, a.1, [true, false])
                } else {
                    (b.0, b.1, [false, true])
                }
            }
            [Some(a), None] => (a.0, a.1, [true, false]),
            [None, Some(b)] => (b.0, b.1, [false, true]),
            [None, None] => unreachable!(),
        };
        for axis in 0..2 {
            if consume[axis] {
                mapped[axis][positions[axis]].0 = knot;
                positions[axis] += 1;
            }
        }
        union.push((knot, multiplicity));
        previous = knot;
    }
    if degree + 1 + union.iter().map(|x| x.1).sum::<usize>() > MAX_EDGE_CONTROLS {
        return Err(GeometryError::EdgeSurfaceResourceLimit {
            maximum: MAX_EDGE_CONTROLS,
        });
    }
    for (axis, curve) in curves.iter_mut().enumerate() {
        let mut knots = curve.knots().to_vec();
        for (old, target) in groups[axis].iter().zip(&mapped[axis]) {
            for k in &mut knots {
                if *k == old.0 {
                    *k = target.0;
                }
            }
        }
        *curve = NurbsCurve::try_new_rational(degree, curve.control_points().to_vec(), knots)?;
        for &(k, m) in &union {
            *curve = curve.try_insert_knot(k, m)?;
        }
    }
    if curves[0].knots() != curves[1].knots()
        || curves[0].control_points().len() != curves[1].control_points().len()
    {
        return Err(GeometryError::InvalidControlNet {
            context: "edge surface boundaries did not reach a common basis",
        });
    }
    Ok(curves)
}

pub(super) fn greville(curve: &NurbsCurve) -> Result<Vec<Real>, GeometryError> {
    let domain = curve.domain();
    (0..curve.control_points().len())
        .map(|i| {
            let t = stable_knot_mean(&curve.knots()[i + 1..=i + curve.degree()])?;
            Ok((t - domain.start()) / (domain.end() - domain.start()))
        })
        .collect()
}
