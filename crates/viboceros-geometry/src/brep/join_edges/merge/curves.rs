//! Concatenation proposals are accepted only after whole-piece certificates.
use super::*;

pub(super) struct Appended {
    pub curve: NurbsCurve,
    pub bounds: [Real; 2],
    pub displacement: Real,
}

pub(super) fn append(
    a: &NurbsCurve,
    b: &NurbsCurve,
    limit: Real,
    previous: [Real; 2],
    budget: &mut Budget,
) -> Result<Option<Appended>, GeometryError> {
    let degree = a.degree().max(b.degree());
    if degree > 16 {
        return Ok(None);
    }
    budget.charge(
        a.control_points()
            .len()
            .saturating_add(b.control_points().len())
            .saturating_mul(degree + 1),
    )?;
    // Degree conversion currently uses a dense basis solve. Charge its cubic
    // upper bound before allocation, not merely the input control count.
    for curve in [a, b] {
        if curve.degree() != degree {
            let count = curve
                .control_points()
                .len()
                .saturating_mul(degree - curve.degree() + 1)
                .saturating_add(2 * degree);
            budget.charge(count.saturating_pow(3))?;
        }
    }
    let proposal = || -> Result<_, GeometryError> {
        let ad = a.domain();
        let bd = b.domain();
        let end = if ad.end() == bd.start() {
            *bd.end()
        } else {
            ad.end() + (bd.end() - bd.start())
        };
        let (a, b) = if a.degree() == b.degree()
            && end.is_finite()
            && end > *ad.end()
            && (end - ad.start()).is_finite()
        {
            (a.clone(), b.try_reparameterized(*ad.end()..=end)?)
        } else {
            (
                a.try_reparameterized(0.0..=1.0)?,
                b.try_reparameterized(1.0..=2.0)?,
            )
        };
        let a = a
            .clamped_to_active_domain()?
            .try_change_degree(degree, false)?;
        let b = b
            .clamped_to_active_domain()?
            .try_change_degree(degree, false)?;
        let split = *a.domain().end();
        let curve = a.try_append_clamped(&b)?;
        // A new full-order junction is not representable by the current
        // OpenNURBS B-rep interchange path. Keep these edges separate.
        if curve.knots().iter().filter(|&&t| t == split).count() > degree {
            return Ok(None);
        }
        Ok(Some((curve, split)))
    };
    let Ok(Some((curve, split))) = proposal() else {
        return Ok(None);
    };
    let domain = curve.domain();
    let mut bounds = [0.; 2];
    let mut displacement: Real = 0.;
    for (i, (source, interval)) in [(a, [*domain.start(), split]), (b, [split, *domain.end()])]
        .into_iter()
        .enumerate()
    {
        let allowance = if previous[i] == 0. {
            limit
        } else {
            (limit - previous[i]).next_down().max(0.)
        };
        let Some(bound) =
            certificate::restricted_curve_bound(source, &curve, interval, allowance, true, |n| {
                budget.charge(n)
            })?
        else {
            return Ok(None);
        };
        let total = certificate::add_bound(previous[i], bound)?;
        if total > limit {
            return Ok(None);
        }
        bounds[i] = bound;
        displacement = displacement.max(total);
    }
    Ok(Some(Appended {
        curve,
        bounds,
        displacement,
    }))
}

pub(super) fn append_uv(
    a: &NurbsCurve2,
    b: &NurbsCurve2,
    budget: &mut Budget,
) -> Result<Option<NurbsCurve2>, GeometryError> {
    let lift = |curve: &NurbsCurve2| {
        NurbsCurve::try_new_rational(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                        p.weight(),
                    )
                })
                .collect::<Result<_, GeometryError>>()?,
            curve.knots().to_vec(),
        )
    };
    let Some(result) = append(&lift(a)?, &lift(b)?, 0., [0.; 2], budget)? else {
        return Ok(None);
    };
    let curve = result.curve;
    Ok(Some(NurbsCurve2::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint2::try_new(Point2::try_new(p.point().x(), p.point().y())?, p.weight())
            })
            .collect::<Result<_, GeometryError>>()?,
        curve.knots().to_vec(),
    )?))
}
