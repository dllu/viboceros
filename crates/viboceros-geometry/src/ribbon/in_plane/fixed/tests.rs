use super::*;
use crate::WeightedPoint3;

fn normal() -> UnitVector3 {
    Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}
fn cubic(points: [[f64; 3]; 4]) -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        points
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn spatial_line_projection_retains_rational_clock_and_height() {
    let source = NurbsCurve::try_new_rational(
        3,
        [[0., 0., 0.], [0.2, 0., 1.], [4., 0., -1.], [5., 0., 2.]]
            .into_iter()
            .zip([2., 0.7, 1.3, 3.])
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
    .try_reparameterized(-4. ..=8.)
    .unwrap();
    let original = source.clone();
    let brep = try_fixed_plane_ribbon(&source, 0.15, false, normal(), Tolerance::DEFAULT).unwrap();
    let surface = brep.faces()[0].surface();
    assert_eq!(surface.domain_v(), source.domain());
    let offset = surface.isocurve_v(*surface.domain_u().end()).unwrap();
    let translated = source
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(0., 0.15, 0.).unwrap(),
        ))
        .unwrap();
    assert!(
        offset
            .parameter_curve_deviation_bound(&translated, 1e-12)
            .unwrap()
            .is_some()
    );
    assert_eq!(source, original);
}

#[test]
fn coarse_both_sides_preserve_cubic_height_throughout_the_ruled_image() {
    let source = cubic([[0., 0., 0.], [1., 2., 2.], [4., -1., -1.], [5., 1., 1.]]);
    let original = source.clone();
    let model = Tolerance::DEFAULT;
    let fit = Tolerance::try_new(0.1, model.relative(), model.angular()).unwrap();
    let brep = try_fixed_plane_ribbon_with_fit_tolerance(
        &source,
        0.15,
        true,
        normal(),
        model,
        fit,
        &mut NurbsSurface::try_ruled_from_curves,
    )
    .unwrap();
    assert_eq!(brep.faces().len(), 2);
    for face in brep.faces() {
        let s = face.surface();
        for v in [0., 0.13, 0.42, 0.69, 1.] {
            let z = source.evaluate(v).unwrap().z();
            for u in [0., 0.04, 0.11, 0.15] {
                assert!((s.evaluate(u, v).unwrap().z() - z).abs() < 1e-12);
            }
        }
    }
    assert_eq!(source, original);
}

#[test]
fn unqualified_plane_switches_fail_before_the_ruled_backend() {
    for source in [
        cubic([
            [0., 0., 0.],
            [1., 2., 0.002],
            [4., -1., -0.001],
            [5., 1., 0.001],
        ]),
        cubic([[0., 0., 0.], [1., 0.001, 1.], [4., 0., -1.], [5., 0., 2.]]),
    ] {
        let result = try_fixed_plane_ribbon_with_ruled(
            &source,
            0.15,
            false,
            normal(),
            Tolerance::DEFAULT,
            &mut |_, _| -> Result<NurbsSurface, GeometryError> {
                panic!("unsupported mode reached backend")
            },
        );
        assert!(matches!(result, Err(GeometryError::UnsupportedCurveOffset)));
    }
}
