#![cfg(feature = "native")]
use viboceros_geometry::{
    Circle3, NurbsCurve, Point3, Tolerance, UnitVector3, Vector3, WeightedPoint3,
};
use viboceros_smlib::{KernelCurve, Shape};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn native_surface_extrusion_preserves_profile_and_distance_parameters() {
    let source = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(point(1., 2., 0.), 0.5).unwrap(),
            WeightedPoint3::try_new(point(3., 0., 0.), 1.).unwrap(),
        ],
        vec![2., 2., 2., 7., 7., 7.],
    )
    .unwrap();
    let native = KernelCurve::from_nurbs(&source).unwrap();
    let before = (0..=16)
        .map(|i| {
            native
                .evaluate(source.parameter_at(i as f64 / 16.).unwrap())
                .unwrap()
        })
        .collect::<Vec<_>>();
    for displacement in [
        Vector3::try_new(0., 3., 4.).unwrap(),
        Vector3::try_new(1., -2., -2.).unwrap(),
    ] {
        let surface = native.extruded_surface(displacement).unwrap();
        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.domain_u(), source.domain());
        assert_eq!(surface.domain_v(), 0. ..=displacement.length().unwrap());
        for (i, before) in before.iter().enumerate() {
            let u = source.parameter_at(i as f64 / 16.).unwrap();
            assert_eq!(&native.evaluate(u).unwrap(), before);
            for j in 0..=8 {
                let f = j as f64 / 8.;
                let expected = source
                    .evaluate(u)
                    .unwrap()
                    .translated(displacement.scaled(f).unwrap())
                    .unwrap();
                let actual = surface
                    .evaluate(u, surface.parameter_at_v(f).unwrap())
                    .unwrap();
                assert!(actual.distance_to(expected).unwrap() < 1e-12);
            }
        }
    }
    assert!(
        native
            .extruded_surface(Vector3::try_new(0., 0., 0.).unwrap())
            .is_err()
    );
}

#[test]
fn native_capped_circular_sweep_preserves_curve_and_matches_analytic_volume() {
    let tolerance = Tolerance::DEFAULT;
    let source = Circle3::try_new(
        point(0., 0., 0.),
        2.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let native = KernelCurve::from_nurbs(&source).unwrap();
    let before = native
        .evaluate(source.parameter_at(0.375).unwrap())
        .unwrap();
    for height in [5., -5.] {
        let shape = Shape::linear_sweep(
            &[&native],
            Vector3::try_new(0., 0., height).unwrap(),
            true,
            tolerance,
        )
        .unwrap();
        let output = shape.to_brep(tolerance).unwrap();
        assert!(output.is_solid());
        let volume = output.signed_volume(tolerance).unwrap();
        assert!(
            (volume - 20. * std::f64::consts::PI).abs() < 1e-8,
            "{height}: {volume}"
        );
        assert_eq!(
            native
                .evaluate(source.parameter_at(0.375).unwrap())
                .unwrap(),
            before
        );
    }
}

#[test]
fn native_sweep_builds_nested_profile_regions_and_preserves_sources_on_failure() {
    let tolerance = Tolerance::DEFAULT;
    let curve = |radius| {
        Circle3::try_new(
            point(0., 0., 0.),
            radius,
            UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let outer = KernelCurve::from_nurbs(&curve(2.)).unwrap();
    let inner = KernelCurve::from_nurbs(&curve(1.)).unwrap();
    let output = Shape::linear_sweep(
        &[&outer, &inner],
        Vector3::try_new(0., 0., 5.).unwrap(),
        true,
        tolerance,
    )
    .unwrap()
    .to_brep(tolerance)
    .unwrap();
    assert!(output.is_solid());
    assert!((output.signed_volume(tolerance).unwrap() - 15. * std::f64::consts::PI).abs() < 1e-8);
    assert!(
        output
            .faces()
            .iter()
            .filter(|f| f.loops().len() == 2)
            .count()
            >= 2
    );
    let before = outer
        .evaluate(outer.to_nurbs().unwrap().parameter_at(0.5).unwrap())
        .unwrap();
    assert!(
        Shape::linear_sweep(
            &[&outer],
            Vector3::try_new(0., 0., 0.).unwrap(),
            true,
            tolerance
        )
        .is_err()
    );
    assert_eq!(
        outer
            .evaluate(outer.to_nurbs().unwrap().parameter_at(0.5).unwrap())
            .unwrap(),
        before
    );
    assert!(
        Shape::linear_sweep(
            &[&outer],
            Vector3::try_new(3., 0., 0.).unwrap(),
            true,
            tolerance
        )
        .is_err()
    );
    assert_eq!(
        outer
            .evaluate(outer.to_nurbs().unwrap().parameter_at(0.5).unwrap())
            .unwrap(),
        before
    );
}
