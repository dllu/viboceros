//! Exact convex-hull certificate against one global affine reference.
use super::*;
use crate::WeightedPoint3;
use num_traits::Signed;

struct Reference {
    origin: [Rational; 3],
    axes: [[Rational; 3]; 2],
    domain: [[Rational; 2]; 2],
    error: Rational,
}
impl Reference {
    fn point(&self, uv: [Rational; 2]) -> [Rational; 3] {
        let fractions = std::array::from_fn::<_, 2, _>(|axis| {
            (&uv[axis] - &self.domain[axis][0]) / (&self.domain[axis][1] - &self.domain[axis][0])
        });
        std::array::from_fn(|k| {
            &self.origin[k] + &self.axes[0][k] * &fractions[0] + &self.axes[1][k] * &fractions[1]
        })
    }
}

fn reference(
    surface: &NurbsSurface,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Reference>, GeometryError> {
    if surface.is_rational()
        || surface.degree_u() > MAX_DEGREE
        || surface.degree_v() > MAX_DEGREE
        || surface.control_points().len() > MAX_WORK / 16
    {
        return Ok(None);
    }
    let n = [
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    ];
    let degree = [surface.degree_u(), surface.degree_v()];
    let knots = [surface.knots_u(), surface.knots_v()];
    let domains = [surface.domain_u(), surface.domain_v()];
    let mut stations = [Vec::new(), Vec::new()];
    for axis in 0..2 {
        let p = degree[axis];
        if knots[axis][..=p].iter().any(|k| k != domains[axis].start())
            || knots[axis][n[axis]..]
                .iter()
                .any(|k| k != domains[axis].end())
            || knots[axis].chunk_by(|a, b| a == b).any(|g| {
                g[0] > *domains[axis].start() && g[0] < *domains[axis].end() && g.len() > p
            })
        {
            return Ok(None);
        }
        for i in 0..n[axis] {
            let station = knots[axis][i + 1..=i + p]
                .iter()
                .map(|&k| rational(k))
                .sum::<Rational>()
                / Rational::from_integer(p.into());
            budget.check(&station)?;
            stations[axis].push(station);
        }
    }
    let control = |i: usize| surface.control_points()[i].point().to_array().map(rational);
    let origin = control(0);
    let ends = [control(n[0] - 1), control((n[1] - 1) * n[0])];
    let mut result = Reference {
        axes: ends.map(|p| std::array::from_fn(|k| &p[k] - &origin[k])),
        origin,
        domain: domains.map(|d| [rational(*d.start()), rational(*d.end())]),
        error: Rational::zero(),
    };
    for j in 0..n[1] {
        for i in 0..n[0] {
            let expected = result.point([stations[0][i].clone(), stations[1][j].clone()]);
            let actual = control(j * n[0] + i);
            let error = (0..3)
                .map(|k| (&actual[k] - &expected[k]).abs())
                .sum::<Rational>();
            budget.check(&error)?;
            if error > rational(limit) {
                return Ok(None);
            }
            result.error = result.error.max(error);
        }
    }
    Ok(Some(result))
}

fn eligible_uv(uv: &NurbsCurve2, reference: &Reference) -> bool {
    let first = uv.control_points()[0].weight();
    uv.degree() <= MAX_DEGREE
        && uv.control_points().iter().all(|c| {
            c.weight().is_sign_positive() == first.is_sign_positive()
                && [c.point().x(), c.point().y()]
                    .into_iter()
                    .enumerate()
                    .all(|(axis, x)| {
                        let x = rational(x);
                        x >= reference.domain[axis][0] && x <= reference.domain[axis][1]
                    })
        })
}

pub(super) fn bound(
    surface: &NurbsSurface,
    uv: &NurbsCurve2,
    spatial: &NurbsCurve,
    limit: Real,
) -> Result<Option<Real>, GeometryError> {
    if uv.degree() != spatial.degree()
        || uv.control_points().len() != spatial.control_points().len()
        || uv.control_points().len() > MAX_WORK / 16
    {
        return Ok(None);
    }
    let mut budget = Budget(MAX_WORK);
    let Some(reference) = reference(surface, limit, &mut budget)? else {
        return Ok(None);
    };
    if !eligible_uv(uv, &reference) {
        return Ok(None);
    }
    let domains =
        [uv.domain(), spatial.domain()].map(|d| [rational(*d.start()), rational(*d.end())]);
    for (&a, &b) in uv.knots().iter().zip(spatial.knots()) {
        if (rational(a) - &domains[0][0]) / (&domains[0][1] - &domains[0][0])
            != (rational(b) - &domains[1][0]) / (&domains[1][1] - &domains[1][0])
        {
            return Ok(None);
        }
    }
    let gauges = [
        rational(uv.control_points()[0].weight()),
        rational(spatial.control_points()[0].weight()),
    ];
    let mut curve_error = Rational::zero();
    for (uv, spatial) in uv.control_points().iter().zip(spatial.control_points()) {
        if rational(uv.weight()) / &gauges[0] != rational(spatial.weight()) / &gauges[1] {
            return Ok(None);
        }
        let expected = reference.point([rational(uv.point().x()), rational(uv.point().y())]);
        let actual = spatial.point().to_array().map(rational);
        let error = (0..3)
            .map(|k| (&actual[k] - &expected[k]).abs())
            .sum::<Rational>();
        budget.check(&error)?;
        curve_error = curve_error.max(error);
    }
    // Positive basis partitions and same-sign rational curve weights make
    // both errors convex combinations of their exact control residuals.
    let error = reference.error + curve_error;
    if error > rational(limit) {
        return Ok(None);
    }
    let mut upper = scalar(&error)?;
    if rational(upper) < error {
        upper = upper.next_up();
    }
    Ok(Some(upper))
}

pub(super) fn image(
    surface: &NurbsSurface,
    uv: &NurbsCurve2,
    limit: Real,
) -> Result<Option<(NurbsCurve, Real)>, GeometryError> {
    let mut budget = Budget(MAX_WORK);
    let Some(reference) = reference(surface, limit, &mut budget)? else {
        return Ok(None);
    };
    if !eligible_uv(uv, &reference) || uv.control_points().len() > MAX_WORK / 16 {
        return Ok(None);
    }
    let controls = uv
        .control_points()
        .iter()
        .map(|p| {
            let point = reference.point([rational(p.point().x()), rational(p.point().y())]);
            let xyz = point.iter().map(scalar).collect::<Result<Vec<_>, _>>()?;
            WeightedPoint3::try_new(Point3::try_new(xyz[0], xyz[1], xyz[2])?, p.weight())
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let candidate = NurbsCurve::try_new_rational(uv.degree(), controls, uv.knots().to_vec())?;
    Ok(bound(surface, uv, &candidate, limit)?.map(|bound| (candidate, bound)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn many_span_planar_circle_has_a_continuous_affine_bound_and_rejects_warped_image() {
        let tolerance = Tolerance::DEFAULT;
        let source = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(10., 0., 0.).unwrap(),
            Point3::try_new(10., 10., 0.).unwrap(),
            Point3::try_new(0., 10., 0.).unwrap(),
        ])
        .unwrap();
        let surface = crate::try_rebuild_nurbs_surface(&source, [10, 10], [3, 3])
            .unwrap()
            .try_reparameterized(0. ..=1., 0. ..=1.)
            .unwrap();
        let circle = crate::Circle3::try_new(
            Point3::try_new(0.5, 0.5, 0.).unwrap(),
            0.3,
            crate::UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let uv = NurbsCurve2::try_new_rational(
            circle.degree(),
            circle
                .control_points()
                .iter()
                .map(|p| {
                    WeightedPoint2::try_new(
                        Point2::try_new(p.point().x(), p.point().y()).unwrap(),
                        p.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            circle.knots().to_vec(),
        )
        .unwrap();
        let (image, error_bound) = surface
            .try_pushup_curve_certified_with_bound(&uv, tolerance)
            .unwrap();
        assert_eq!(image.degree(), uv.degree());
        assert_eq!(image.control_points().len(), uv.control_points().len());
        assert!(error_bound < 1e-12);
        assert!(
            surface
                .parameter_curve_deviation_bound(&uv, &image, 1e-12)
                .unwrap()
                .is_some()
        );
        for i in 0..=128 {
            let t = uv.parameter_at(i as Real / 128.).unwrap();
            let p = uv.evaluate(t).unwrap();
            assert!(
                surface
                    .evaluate(p.x(), p.y())
                    .unwrap()
                    .distance_to(image.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
        let wrong = image
            .transformed(crate::AffineTransform3::from_translation(
                Vector3::try_new(0., 0., 0.001).unwrap(),
            ))
            .unwrap();
        assert!(bound(&surface, &uv, &wrong, 1e-9).unwrap().is_none());
        let mixed = NurbsCurve2::try_new_rational(
            uv.degree(),
            uv.control_points()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    WeightedPoint2::try_new(
                        p.point(),
                        if i == 1 { -p.weight() } else { p.weight() },
                    )
                    .unwrap()
                })
                .collect(),
            uv.knots().to_vec(),
        )
        .unwrap();
        assert!(bound(&surface, &mixed, &image, 1e-9).unwrap().is_none());
    }
}
