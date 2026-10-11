#![cfg(feature = "native")]
use viboceros_geometry::{Circle3, NurbsCurve, Point3, Tolerance, UnitVector3, WeightedPoint3};
use viboceros_smlib::{KernelCurve, Shape};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_new(p[0], p[1], p[2]).unwrap()
}
fn quadratic(controls: [[f64; 3]; 3], weight: f64, domain: [f64; 2]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        controls
            .into_iter()
            .zip([1., weight, 1.])
            .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
            .collect(),
        vec![
            domain[0], domain[0], domain[0], domain[1], domain[1], domain[1],
        ],
    )
    .unwrap()
}
fn circle() -> NurbsCurve {
    Circle3::try_new(
        point([0., 0., 0.]),
        2.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn rational_sweep_retains_parameters_and_matches_translation_without_mutating_curves() {
    let profile = quadratic([[4., 0., 0.], [4., 1., 0.], [3., 1., 0.]], 0.75, [3., 8.]);
    let path = quadratic([[10., 0., 0.], [11., 2., 1.], [12., 3., 4.]], 0.5, [2., 7.]);
    let source = KernelCurve::from_nurbs(&profile).unwrap();
    let rail = KernelCurve::from_nurbs(&path).unwrap();
    let surface = source.translational_sweep(&rail).unwrap();
    assert_eq!(surface.knots_u(), profile.knots());
    assert_eq!(surface.knots_v(), path.knots());
    let start = path.evaluate(*path.domain().start()).unwrap();
    for i in 0..=16 {
        let u = profile.parameter_at(i as f64 / 16.).unwrap();
        let p = profile.evaluate(u).unwrap();
        assert!(source.evaluate(u).unwrap().distance_to(p).unwrap() < 1e-12);
        for j in 0..=16 {
            let v = path.parameter_at(j as f64 / 16.).unwrap();
            let q = path.evaluate(v).unwrap();
            let expected = p.translated(start.vector_to(q).unwrap()).unwrap();
            assert!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
            assert!(rail.evaluate(v).unwrap().distance_to(q).unwrap() < 1e-12);
        }
    }
}

#[test]
fn curved_capped_sweep_has_analytic_volume_for_both_path_directions() {
    let t = Tolerance::DEFAULT;
    let profile = KernelCurve::from_nurbs(&circle()).unwrap();
    for direction in [1., -1.] {
        let path = quadratic(
            [
                [30., 0., 0.],
                [31., 4., 2. * direction],
                [32., 3., 5. * direction],
            ],
            0.5,
            [2., 7.],
        );
        let rail = KernelCurve::from_nurbs(&path).unwrap();
        let before = rail.evaluate(4.5).unwrap();
        let shape = Shape::translational_sweep(&profile, &rail, t).unwrap();
        let brep = shape.to_brep(t).unwrap();
        assert!(brep.is_solid());
        assert!((brep.signed_volume(t).unwrap() - 20. * std::f64::consts::PI).abs() < 1e-8);
        assert_eq!(rail.evaluate(4.5).unwrap(), before);
    }
}

#[test]
fn modeling_preparation_preserves_unclamped_and_negative_gauge_sweep_geometry() {
    let make = |controls: [[f64; 3]; 3], weights: [f64; 3]| {
        NurbsCurve::try_new_rational(
            2,
            controls
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
                .collect(),
            vec![-2., -1., 0., 1., 2., 3.],
        )
        .unwrap()
    };
    for gauge in [1., -1.] {
        let profile = make(
            [[0., 0., 0.], [1., 2., 0.], [3., 1., 1.]],
            [gauge, 0.75 * gauge, gauge],
        );
        let path = make(
            [[20., 0., 0.], [21., 2., 2.], [23., 1., 5.]],
            [gauge, 0.5 * gauge, gauge],
        );
        let native = KernelCurve::from_nurbs_for_modeling(&profile).unwrap();
        let rail = KernelCurve::from_nurbs_for_modeling(&path).unwrap();
        let output = native.translational_sweep(&rail).unwrap();
        assert_eq!(output.domain_u(), profile.domain());
        assert_eq!(output.domain_v(), path.domain());
        let start = path.evaluate(*path.domain().start()).unwrap();
        for i in 0..=16 {
            let u = profile.parameter_at(i as f64 / 16.).unwrap();
            for j in 0..=16 {
                let v = path.parameter_at(j as f64 / 16.).unwrap();
                let expected = profile
                    .evaluate(u)
                    .unwrap()
                    .translated(start.vector_to(path.evaluate(v).unwrap()).unwrap())
                    .unwrap();
                assert!(
                    output
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
}

#[test]
fn constant_paths_and_coplanar_caps_fail_without_mutating_native_inputs() {
    let t = Tolerance::DEFAULT;
    let source = circle();
    let profile = KernelCurve::from_nurbs(&source).unwrap();
    let before = profile
        .evaluate(source.parameter_at(0.375).unwrap())
        .unwrap();
    let path = quadratic(
        [[10., 0., 0.], [10., 0., 0.], [10., 0., 0.]],
        0.5,
        [-4., 6.],
    );
    let rail = KernelCurve::from_nurbs(&path).unwrap();
    assert!(profile.translational_sweep(&rail).is_err());
    let planar = quadratic(
        [[10., 0., 0.], [11., 2., 0.], [12., 3., 0.]],
        0.5,
        [-4., 6.],
    );
    let rail = KernelCurve::from_nurbs(&planar).unwrap();
    assert!(Shape::translational_sweep(&profile, &rail, t).is_err());
    assert_eq!(
        profile
            .evaluate(source.parameter_at(0.375).unwrap())
            .unwrap(),
        before
    );
    assert_eq!(rail.evaluate(-4.).unwrap(), point([10., 0., 0.]));
}
