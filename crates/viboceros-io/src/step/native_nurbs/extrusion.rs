//! Shared linear-extrusion patches and certified non-isoparametric edge images.
use super::*;
use monstertruck::step::load::step_geometry::StepExtrusionSurface;

pub(super) fn patch(
    directrix: &NurbsCurve,
    vector: [f64; 3],
    range: [f64; 2],
    id: u64,
) -> Result<NurbsSurface, StepError> {
    let [v0, v1] = range;
    if !v0.is_finite() || !v1.is_finite() || v0 >= v1 {
        return Err(StepError::UnsupportedNativeShell {
            shell: id,
            reason: "extrusion axial trim range is degenerate",
        });
    }
    let controls = range
        .into_iter()
        .flat_map(|v| {
            directrix.control_points().iter().map(move |c| {
                let p = c.point().to_array();
                WeightedPoint3::try_new(
                    Point3::try_new(
                        p[0] + v * vector[0],
                        p[1] + v * vector[1],
                        p[2] + v * vector[2],
                    )?,
                    c.weight(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NurbsSurface::try_new_rational(
        directrix.degree(),
        1,
        directrix.control_points().len(),
        2,
        controls,
        directrix.knots().to_vec(),
        vec![v0, v0, v1, v1],
    )?)
}

fn preserves_parameter(curve: &Curve3D) -> bool {
    match curve {
        Curve3D::Line(_)
        | Curve3D::Polyline(_)
        | Curve3D::BsplineCurve(_)
        | Curve3D::NurbsCurve(_) => true,
        Curve3D::SurfaceCurve(c) => preserves_parameter(c.leader()),
        Curve3D::IntersectionCurve(c) => preserves_parameter(c.leader()),
        // Converted conics keep their loci but have a different interior parameter speed.
        Curve3D::Conic(_) => false,
        Curve3D::ParameterCurve(pcurve) => {
            matches!(
                pcurve.curve().as_ref(),
                Curve2D::Line(_)
                    | Curve2D::Polyline(_)
                    | Curve2D::BsplineCurve(_)
                    | Curve2D::NurbsCurve(_)
            ) && match pcurve.surface().as_ref() {
                Surface::ElementarySurface(ElementarySurface::Plane(_))
                | Surface::BsplineSurface(_)
                | Surface::NurbsSurface(_) => true,
                Surface::SweepSurface(SweepSurface::ExtrusionSurface(e)) => {
                    preserves_parameter(e.entity_curve())
                }
                _ => false,
            }
        }
    }
}

pub(super) fn image(
    extrusion: &StepExtrusionSurface,
    uv: &NurbsCurve2,
    id: u64,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve>, StepError> {
    if !preserves_parameter(extrusion.entity_curve()) {
        return Ok(None);
    }
    let directrix = sweep_directrix_with_tolerance(extrusion.entity_curve(), id, tolerance)?;
    let sign = uv.control_points()[0].weight().is_sign_positive();
    let mut range = [f64::INFINITY, f64::NEG_INFINITY];
    for c in uv.control_points() {
        if c.weight().is_sign_positive() != sign || !directrix.domain().contains(&c.point().x()) {
            return Err(StepError::UnsupportedNativeShell {
                shell: id,
                reason: "extrusion p-curve leaves its coherent source chart",
            });
        }
        range[0] = range[0].min(c.point().y());
        range[1] = range[1].max(c.point().y());
    }
    let vector = extrusion.extruding_vector();
    if range[0] == range[1] {
        range = [range[0].min(0.), range[1].max(0.)];
        if range[0] == range[1] {
            range = [0., 1.];
        }
    }
    let surface = patch(&directrix, [vector.x, vector.y, vector.z], range, id)?;
    let image = surface.try_pushup_curve_certified(uv, tolerance)?;
    Ok(Some(image))
}

#[cfg(test)]
mod tests {
    use super::*;
    use monstertruck::meshing::prelude::ParametricSurface;
    use monstertruck::modeling::{
        BsplineCurve, KnotVector, NurbsCurve as TruckNurbsCurve, Point3 as P, Vector3, Vector4,
    };
    fn q(u: f64, v: f64) -> Point2 {
        Point2::try_new(u, v).unwrap()
    }
    fn uv(points: [[f64; 2]; 3], weights: [f64; 3]) -> NurbsCurve2 {
        NurbsCurve2::try_new_rational(
            2,
            points
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint2::try_new(q(p[0], p[1]), w).unwrap())
                .collect(),
            vec![-3., -3., -3., 7., 7., 7.],
        )
        .unwrap()
    }
    #[test]
    fn curved_and_reversed_rational_paths_on_spline_extrusions_keep_domains_and_loci() {
        let knots = || KnotVector::from(vec![2., 2., 2., 3., 4., 4., 4.]);
        let polynomial = Curve3D::BsplineCurve(BsplineCurve::new(
            knots(),
            vec![
                P::new(0., 0., 0.),
                P::new(1., 0., 1.),
                P::new(2., 0., -0.5),
                P::new(3., 0., 0.),
            ],
        ));
        let rational = Curve3D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
            knots(),
            vec![
                Vector4::new(0., 0., 0., 1.),
                Vector4::new(0.5, 0., 0.5, 0.5),
                Vector4::new(4., 0., -1., 2.),
                Vector4::new(3., 0., 0., 1.),
            ],
        )));
        let parameterized = Curve3D::ParameterCurve(StepParameterCurve::new(
            Box::new(Curve2D::BsplineCurve(BsplineCurve::new(
                knots(),
                vec![
                    monstertruck::modeling::Point2::new(0., 0.),
                    monstertruck::modeling::Point2::new(1., 1.),
                    monstertruck::modeling::Point2::new(2., -0.5),
                    monstertruck::modeling::Point2::new(3., 0.),
                ],
            ))),
            Box::new(Surface::ElementarySurface(ElementarySurface::Plane(
                monstertruck::modeling::Plane::new(
                    P::new(0., 0., 0.),
                    P::new(1., 0., 0.),
                    P::new(0., 0., 1.),
                ),
            ))),
        ));
        for directrix in [polynomial, rational, parameterized] {
            let source =
                StepExtrusionSurface::by_extrusion(directrix, Vector3::new(0.25, 3., -0.5));
            for path in [
                uv([[2.1, -0.3], [3.7, 1.8], [3.9, 0.7]], [1., 0.7, 2.]),
                uv([[3.9, 0.7], [3.7, 1.8], [2.1, -0.3]], [-2., -0.7, -1.]),
                uv([[2.1, 0.4], [3.3, 0.4], [3.9, 0.4]], [1., 0.7, 2.]),
            ] {
                let original = path.clone();
                let result = image(&source, &path, 27, Tolerance::DEFAULT)
                    .unwrap()
                    .unwrap();
                assert_eq!(result.domain(), path.domain());
                assert_eq!(path, original);
                for i in 0..=64 {
                    let t = -3. + 10. * i as f64 / 64.;
                    let uv = path.evaluate(t).unwrap();
                    let expected = source.evaluate(uv.x(), uv.y());
                    let actual = result.evaluate(t).unwrap();
                    assert!((actual.x() - expected.x).abs() < 1e-6);
                    assert!((actual.y() - expected.y).abs() < 1e-6);
                    assert!((actual.z() - expected.z).abs() < 1e-6);
                }
            }
        }
    }
    #[test]
    fn extrusion_curve_images_reject_outside_or_mixed_weight_charts() {
        let source = StepExtrusionSurface::by_extrusion(
            Curve3D::BsplineCurve(BsplineCurve::new(
                KnotVector::from(vec![2., 2., 2., 4., 4., 4.]),
                vec![P::new(0., 0., 0.), P::new(1., 0., 1.), P::new(2., 0., 0.)],
            )),
            Vector3::new(0., 3., 0.),
        );
        for path in [
            uv([[1.9, 0.], [3., 1.], [3.9, 0.7]], [1.; 3]),
            uv([[2.1, 0.], [3., 1.], [3.9, 0.7]], [1., -0.1, 1.]),
        ] {
            assert!(matches!(
                image(&source, &path, 28, Tolerance::DEFAULT),
                Err(StepError::UnsupportedNativeShell { shell: 28, .. })
            ));
        }
    }
}
