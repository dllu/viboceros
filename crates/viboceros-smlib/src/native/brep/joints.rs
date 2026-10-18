//! Weld roundoff-sized UV joints only after proving the spatial trim image.
use super::*;

pub(super) fn close_numerical_joints(
    surface: &NurbsSurface,
    edges: &[BrepEdge],
    mut trims: Vec<BrepTrim>,
    tolerance: Tolerance,
) -> Result<Vec<BrepTrim>, Error> {
    let domains = [surface.domain_u(), surface.domain_v()];
    let numerical = domains.map(|d| (d.end() - d.start()).abs() * (256. * f64::EPSILON));
    for i in 0..trims.len() {
        let current = &trims[i];
        let next = &trims[(i + 1) % trims.len()];
        if current.vertices()[1] != next.vertices()[0] {
            continue;
        }
        let end = current.curve().end_point()?;
        let target = next.curve().start_point()?;
        if end == target {
            continue;
        }
        if (end.x() - target.x()).abs() > numerical[0]
            || (end.y() - target.y()).abs() > numerical[1]
        {
            continue;
        }
        let Some(edge) = current.edge().and_then(|e| edges.get(e)) else {
            continue;
        };
        let source = current.curve();
        let degree = source.degree();
        let knots = source.knots();
        let last = *knots.last().ok_or(Error::InvalidOutput)?;
        if knots[knots.len() - degree - 1..].iter().any(|k| *k != last) {
            continue;
        }
        let mut points = source.control_points().to_vec();
        let control = points.last_mut().ok_or(Error::InvalidOutput)?;
        *control = WeightedPoint2::try_new(target, control.weight())?;
        let candidate = NurbsCurve2::try_new_rational(degree, points, knots.to_vec())?;
        let witness = if current.is_reversed_3d() {
            candidate.reversed()?
        } else {
            candidate.clone()
        };
        if surface
            .parameter_curve_deviation_bound(
                &witness,
                edge.curve(),
                tolerance.absolute().max(edge.tolerance()),
            )?
            .is_none()
        {
            return Err(Error::Kernel(
                "Numerical native UV joint cannot be continuously certified".into(),
            ));
        }
        let iso = classify_iso(surface, &candidate);
        trims[i] = BrepTrim::try_new(
            current.vertices(),
            current.edge(),
            current.is_reversed_3d(),
            candidate,
            current.trim_type(),
            iso,
            current.tolerance(),
        )?;
    }
    Ok(trims)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(scale: f64, gap: f64) -> (NurbsSurface, Vec<BrepEdge>, Vec<BrepTrim>) {
        let s = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            [
                [0., 0., 0.],
                [scale, 0., 0.],
                [0., scale, 0.],
                [scale, scale, 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let mut edges = Vec::new();
        let mut trims = Vec::new();
        for i in 0..2 {
            let points = if i == 0 {
                [[0., 0., 0.], [scale, 0., 0.]]
            } else {
                [[scale, 0., 0.], [0., 0., 0.]]
            };
            let uv = if i == 0 {
                [[0., 0.], [1., 0.]]
            } else {
                [[1. - gap, 0.], [0., 0.]]
            };
            let vertices = if i == 0 { [0, 1] } else { [1, 0] };
            let edge = NurbsCurve::try_new(
                1,
                points.map(|p| Point3::try_from(p).unwrap()).to_vec(),
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            edges.push(BrepEdge::try_new(vertices, edge, 0.).unwrap());
            trims.push(
                BrepTrim::try_new(
                    vertices,
                    Some(i),
                    false,
                    NurbsCurve2::try_line(
                        Point2::try_from(uv[0]).unwrap(),
                        Point2::try_from(uv[1]).unwrap(),
                    )
                    .unwrap(),
                    BrepTrimType::Boundary,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )
                .unwrap(),
            );
        }
        (s, edges, trims)
    }
    #[test]
    fn small_uv_joint_is_welded_with_a_continuous_spatial_certificate() {
        let (s, e, t) = fixture(1., 4. * f64::EPSILON);
        let output = close_numerical_joints(&s, &e, t, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            output[0].curve().end_point().unwrap(),
            output[1].curve().start_point().unwrap()
        );
        assert_eq!(output[0].tolerance(), [0.; 2]);
    }
    #[test]
    fn small_uv_gap_that_exceeds_the_physical_error_budget_is_rejected() {
        let (s, e, t) = fixture(1e9, 4. * f64::EPSILON);
        assert!(
            close_numerical_joints(&s, &e, t, Tolerance::try_new(1e-10, 1e-14, 1e-10).unwrap())
                .is_err()
        );
    }
    #[test]
    fn separate_parameter_branches_are_not_joined() {
        let (s, e, t) = fixture(1., 0.5);
        assert_eq!(
            close_numerical_joints(&s, &e, t.clone(), Tolerance::DEFAULT).unwrap(),
            t
        );
    }
    #[test]
    fn a_welded_curve_gets_its_actual_isoparametric_classification() {
        let (surface, mut edges, mut trims) = fixture(1., 4. * f64::EPSILON);
        let gap = 4. * f64::EPSILON;
        for i in 0..2 {
            let vertices = if i == 0 { [0, 1] } else { [1, 0] };
            let spatial = if i == 0 {
                [[0.5, 0., 0.], [0.5, 1., 0.]]
            } else {
                [[0.5, 1., 0.], [0.5, 0., 0.]]
            };
            let uv = if i == 0 {
                [[0.5, 0.], [0.5, 1.]]
            } else {
                [[0.5 - gap, 1.], [0.5, 0.]]
            };
            edges[i] = BrepEdge::try_new(
                vertices,
                NurbsCurve::try_new(
                    1,
                    spatial.map(|p| Point3::try_from(p).unwrap()).to_vec(),
                    vec![0., 0., 1., 1.],
                )
                .unwrap(),
                0.,
            )
            .unwrap();
            trims[i] = BrepTrim::try_new(
                vertices,
                Some(i),
                false,
                NurbsCurve2::try_line(
                    Point2::try_from(uv[0]).unwrap(),
                    Point2::try_from(uv[1]).unwrap(),
                )
                .unwrap(),
                BrepTrimType::Boundary,
                if i == 0 {
                    SurfaceIso::InteriorUConstant
                } else {
                    SurfaceIso::NotIso
                },
                [0.; 2],
            )
            .unwrap();
        }
        let output = close_numerical_joints(&surface, &edges, trims, Tolerance::DEFAULT).unwrap();
        assert_eq!(output[0].iso(), SurfaceIso::NotIso);
        assert_eq!(
            output[0].curve().end_point().unwrap(),
            output[1].curve().start_point().unwrap()
        );
    }
}
