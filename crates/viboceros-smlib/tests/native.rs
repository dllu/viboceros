#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};
use viboceros_smlib::{BooleanOperation, KernelCurve, Solid, Tessellation};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn relative(actual: f64, expected: f64, epsilon: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs() * epsilon,
        "{actual} != {expected}"
    );
}

#[test]
fn boolean_operands_remain_owned_and_curved_outputs_become_closed_rust_meshes() {
    let a = Solid::box_solid(p(0., 0., 0.), [10.; 3]).unwrap();
    let b = Solid::box_solid(p(5., 2., 3.), [10.; 3]).unwrap();
    let original_a = a.properties(1e-8).unwrap();
    let original_b = b.properties(1e-8).unwrap();
    for (operation, volume) in [
        (BooleanOperation::Union, 1720.),
        (BooleanOperation::Intersection, 280.),
        (BooleanOperation::Difference, 720.),
    ] {
        let result = a.boolean(&b, operation).unwrap();
        let properties = result.properties(1e-8).unwrap();
        assert!(properties.manifold);
        relative(properties.volume, volume, 1e-9);
        let mesh = result
            .tessellate(Tessellation::default(), Tolerance::DEFAULT)
            .unwrap();
        let topology = mesh.topology();
        assert!(topology.is_closed());
        assert_eq!(topology.orientation_conflict_edge_count(), 0);
        assert_eq!(topology.non_manifold_edge_count(), 0);
        relative(mesh.signed_volume().unwrap(), volume, 1e-9);
        assert_eq!(a.properties(1e-8).unwrap(), original_a);
        assert_eq!(b.properties(1e-8).unwrap(), original_b);
    }
    let cylinder = Solid::cylinder(p(5., 5., -1.), 2., 12.).unwrap();
    let cut = a.boolean(&cylinder, BooleanOperation::Difference).unwrap();
    let expected = 1000. - 40. * std::f64::consts::PI;
    relative(cut.properties(1e-8).unwrap().volume, expected, 1e-8);
    let mesh = cut
        .tessellate(Tessellation::default(), Tolerance::DEFAULT)
        .unwrap();
    assert!(mesh.topology().is_closed());
    assert_eq!(mesh.topology().orientation_conflict_edge_count(), 0);
    relative(mesh.signed_volume().unwrap(), expected, 0.005);
    let sphere = Solid::sphere(p(5., 5., 10.), 2.).unwrap();
    let pocket = a.boolean(&sphere, BooleanOperation::Difference).unwrap();
    let expected = 1000. - 16. * std::f64::consts::PI / 3.;
    relative(pocket.properties(1e-8).unwrap().volume, expected, 1e-8);
    assert_eq!(a.properties(1e-8).unwrap(), original_a);
}

#[test]
fn invalid_queries_and_tessellation_preserve_solid_and_return_errors() {
    for dimensions in [
        [0., 1., 1.],
        [-1., 1., 1.],
        [1., f64::NAN, 1.],
        [f64::INFINITY; 3],
    ] {
        assert!(Solid::box_solid(p(0., 0., 0.), dimensions).is_err());
    }
    for radius in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(Solid::sphere(p(0., 0., 0.), radius).is_err());
        assert!(Solid::cylinder(p(0., 0., 0.), radius, 2.).is_err());
    }
    let a = Solid::box_solid(p(0., 0., 0.), [2.; 3]).unwrap();
    let before = a.properties(1e-8).unwrap();
    for accuracy in [0., 1e-10, f64::NAN, 1.] {
        assert!(a.properties(accuracy).is_err());
    }
    for chord_height in [0., -1., f64::NAN] {
        assert!(
            a.tessellate(
                Tessellation {
                    chord_height,
                    ..Tessellation::default()
                },
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
    assert_eq!(a.properties(1e-8).unwrap(), before);
    let result = a.boolean(&a, BooleanOperation::Union).unwrap();
    relative(result.properties(1e-8).unwrap().volume, 8., 1e-9);
    assert_eq!(a.properties(1e-8).unwrap(), before);
}

#[test]
fn rational_curve_transfer_retains_knots_control_points_weights_and_evaluation() {
    for knots in [
        vec![0., 0., 0., 1., 1., 1.],
        vec![100., 100., 100., 105., 105., 105.],
    ] {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(p(1., 0., 0.), 1.).unwrap(),
                WeightedPoint3::try_new(p(1., 1., 0.), std::f64::consts::FRAC_1_SQRT_2).unwrap(),
                WeightedPoint3::try_new(p(0., 1., 0.), 1.).unwrap(),
            ],
            knots,
        )
        .unwrap();
        let native = KernelCurve::from_nurbs(&curve).unwrap();
        let returned = native.to_nurbs().unwrap();
        assert_eq!(returned, curve);
        let domain = curve.domain();
        for i in 0..=128 {
            let t = domain.start() + (domain.end() - domain.start()) * i as f64 / 128.;
            assert!(
                native
                    .evaluate(t)
                    .unwrap()
                    .distance_to(curve.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
        assert!(native.evaluate(f64::NAN).is_err());
        assert!(native.evaluate(*domain.start() - 1.).is_err());
    }
    let piecewise = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., 0., 0.), 4.).unwrap(),
            WeightedPoint3::try_new(p(2., 3., 0.), 2.).unwrap(),
        ],
        vec![0., 0., 0.4, 1., 1.],
    )
    .unwrap();
    let native = KernelCurve::from_nurbs(&piecewise).unwrap();
    assert_eq!(native.to_nurbs().unwrap(), piecewise);
    for t in [0., 0.2, 0.4, 0.6, 1.] {
        assert!(
            native
                .evaluate(t)
                .unwrap()
                .distance_to(piecewise.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn polynomial_curve_transfer_handles_unit_weights_and_nonuniform_cubic_knots() {
    for (degree, points, knots) in [
        (1, vec![p(0., 0., 0.), p(2., 3., 4.)], vec![0., 0., 1., 1.]),
        (
            3,
            vec![
                p(0., 0., 0.),
                p(1., 2., 0.),
                p(3., -1., 1.),
                p(4., 3., 0.),
                p(6., 0., 2.),
            ],
            vec![0., 0., 0., 0., 0.3, 1., 1., 1., 1.],
        ),
    ] {
        let original = NurbsCurve::try_new_rational(
            degree,
            points
                .into_iter()
                .map(|point| WeightedPoint3::try_new(point, 1.).unwrap())
                .collect(),
            knots,
        )
        .unwrap();
        let native = KernelCurve::from_nurbs(&original).unwrap();
        assert_eq!(native.to_nurbs().unwrap(), original);
        for i in 0..=128 {
            let parameter = i as f64 / 128.;
            assert!(
                native
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(original.evaluate(parameter).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
}

#[test]
fn unsupported_negative_weights_and_unclamped_curves_fail_explicitly() {
    let negative = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), -1.).unwrap(),
            WeightedPoint3::try_new(p(2., 0., 0.), -2.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(KernelCurve::from_nurbs(&negative).is_err());
    let unclamped = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., 0., 0.), 1.).unwrap(),
        ],
        vec![-1., 0., 1., 2.],
    )
    .unwrap();
    assert!(KernelCurve::from_nurbs(&unclamped).is_err());
}

#[test]
fn independently_owned_worker_calls_are_serialized_and_handles_drop_cleanly() {
    let workers = (0..4)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..8 {
                    let solid = Solid::box_solid(p(0., 0., 0.), [2.; 3]).unwrap();
                    relative(solid.properties(1e-8).unwrap().volume, 8., 1e-9);
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
}
