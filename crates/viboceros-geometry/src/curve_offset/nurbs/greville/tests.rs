use super::*;
use crate::Ellipse3;

fn ellipse(center: Point3, rx: Real, ry: Real) -> Ellipse3 {
    let axis = |x, y, z| {
        Vector3::try_new(x, y, z)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
    };
    Ellipse3::try_new(
        center,
        rx,
        ry,
        axis(1., 0., 0.),
        axis(0., 1., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn circle_loci_retain_degree_and_accurate_radius_after_translation() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    for center in [
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(100_000., -200_000., 300_000.).unwrap(),
    ] {
        let circle = ellipse(center, 5., 5.);
        for degree in [2, 3, 7, 17] {
            let source = circle
                .to_nurbs()
                .unwrap()
                .try_change_degree(degree, false)
                .unwrap();
            for distance in [-1.5, 1.5] {
                let offset =
                    fit_normal_locus(&source, distance, circle.normal().unwrap(), tolerance, None)
                        .unwrap();
                assert_eq!(offset.degree(), degree);
                assert_eq!(offset.control_points().len(), source.control_points().len());
                let domain = offset.domain();
                for i in 0..=256 {
                    let t = domain.start() + (domain.end() - domain.start()) * i as Real / 256.;
                    let radius = center.distance_to(offset.evaluate(t).unwrap()).unwrap();
                    assert!(
                        (radius - (5. - distance)).abs() < tolerance.absolute() * 0.1,
                        "degree {degree}: {radius}"
                    );
                }
            }
        }
    }
}

#[test]
fn refined_ellipse_locus_has_constant_analytic_nearest_distance() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let ellipse = ellipse(Point3::try_new(0., 0., 0.).unwrap(), 6., 2.);
    for degree in [2, 3, 7] {
        let source = ellipse
            .to_nurbs()
            .unwrap()
            .try_change_degree(degree, false)
            .unwrap();
        let offset =
            fit_normal_locus(&source, -2., ellipse.normal().unwrap(), tolerance, None).unwrap();
        assert_eq!(offset.degree(), degree);
        assert!(offset.control_points().len() > source.control_points().len());
        let domain = offset.domain();
        for i in 0..1024 {
            let t = domain.start() + (domain.end() - domain.start()) * (i as Real + 0.37) / 1024.;
            let point = offset.evaluate(t).unwrap();
            let closest = ellipse
                .evaluate(ellipse.closest_parameter(point).unwrap())
                .unwrap();
            let distance = point.distance_to(closest).unwrap();
            assert!(
                (distance - 2.).abs() <= 2. * tolerance.absolute(),
                "degree {degree}: {distance}"
            );
        }
    }
}

#[test]
fn oversized_source_basis_declines_before_interpolation() {
    fn forbidden(_: &crate::CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, GeometryError> {
        panic!("oversized sources must decline before calling the solver")
    }
    let circle = ellipse(Point3::try_new(0., 0., 0.).unwrap(), 5., 5.);
    let high_degree = circle
        .to_nurbs()
        .unwrap()
        .try_change_degree(18, false)
        .unwrap();
    let controls = (0..1001)
        .map(|i| Point3::try_new(i as Real, 0., 0.).unwrap())
        .collect();
    let knots = std::iter::once(0.)
        .chain((0..=1000).map(|i| i as Real))
        .chain(std::iter::once(1000.))
        .collect();
    let many_controls = NurbsCurve::try_new(1, controls, knots).unwrap();
    for source in [high_degree, many_controls] {
        for solver in [None, Some(forbidden as crate::CurveEditPointSolver)] {
            assert!(matches!(
                fit_normal_locus(
                    &source,
                    1.,
                    circle.normal().unwrap(),
                    Tolerance::DEFAULT,
                    solver
                ),
                Err(GeometryError::NurbsOffsetFitLimit)
            ));
        }
    }
}

#[test]
fn malformed_backend_results_decline_without_a_portable_fallback() {
    fn truncated(_: &crate::CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, GeometryError> {
        Ok(Vec::new())
    }
    fn nonfinite(job: &crate::CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, GeometryError> {
        Ok(vec![Real::NAN; job.system.system.targets.len()])
    }
    fn failed(_: &crate::CurveEditPointInterpolation<'_>) -> Result<Vec<Real>, GeometryError> {
        Err(GeometryError::CurveEditPointInterpolationFailed)
    }
    let circle = ellipse(Point3::try_new(0., 0., 0.).unwrap(), 5., 5.);
    let source = circle.to_nurbs().unwrap();
    for solver in [truncated, nonfinite, failed] {
        assert!(
            fit_normal_locus(
                &source,
                1.,
                circle.normal().unwrap(),
                Tolerance::DEFAULT,
                Some(solver)
            )
            .is_err()
        );
    }
}
