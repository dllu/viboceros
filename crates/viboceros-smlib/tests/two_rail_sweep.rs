#![cfg(feature = "native")]
use viboceros_geometry::{
    AffineTransform3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::two_rail_sweep_surface;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn curve(points: Vec<Point3>) -> NurbsCurve {
    let degree = points.len() - 1;
    NurbsCurve::try_new(
        degree,
        points,
        std::iter::repeat_n(0., degree + 1)
            .chain(std::iter::repeat_n(1., degree + 1))
            .collect(),
    )
    .unwrap()
}
fn inputs(bend: f64) -> ([NurbsCurve; 2], Vec<NurbsCurve>) {
    let rails = [0., 2.].map(|x| {
        if bend == 0. {
            curve(vec![p(x, 0., 0.), p(x, 0., 5.)])
        } else {
            curve(vec![p(x, 0., 0.), p(x, bend, 2.5), p(x, 0., 5.)])
        }
    });
    let mut sections = vec![curve(vec![p(0., 0., 0.), p(1., 1., 0.), p(2., 0., 0.)])];
    if bend != 0. {
        sections.push(curve(vec![
            p(0., bend / 2., 2.5),
            p(1., bend / 2. + 1., 2.5),
            p(2., bend / 2., 2.5),
        ]));
    }
    sections.push(curve(vec![p(0., 0., 5.), p(1., 1., 5.), p(2., 0., 5.)]));
    (rails, sections)
}
fn check(surface: &NurbsSurface, bend: f64, shift: Vector3) {
    for i in 0..=12 {
        for j in 0..=12 {
            let (u, v) = (i as f64 / 12., j as f64 / 12.);
            let expected = p(2. * v, 2. * bend * u * (1. - u) + 2. * v * (1. - v), 5. * u)
                .translated(shift)
                .unwrap();
            let actual = surface.evaluate(u, v).unwrap();
            assert!(
                actual.distance_to(expected).unwrap() < 2e-6,
                "{u} {v}: {actual:?} != {expected:?}"
            );
        }
    }
}
#[test]
fn native_two_rail_sweep_interpolates_rails_sections_and_preserves_inputs() {
    for bend in [0., 1.] {
        let (rails, sections) = inputs(bend);
        let before = format!("{rails:?}{sections:?}");
        let s =
            two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
        check(&s, bend, Vector3::try_new(0., 0., 0.).unwrap());
        assert_eq!(format!("{rails:?}{sections:?}"), before);
    }
}
#[test]
fn native_two_rail_sweep_orients_and_orders_independent_curve_domains() {
    let (mut rails, mut sections) = inputs(1.);
    rails[0] = rails[0].try_reparameterized(-7. ..=13.).unwrap();
    rails[1] = rails[1]
        .reversed()
        .unwrap()
        .try_reparameterized(42. ..=45.)
        .unwrap();
    sections.reverse();
    sections[0] = sections[0]
        .reversed()
        .unwrap()
        .try_reparameterized(10. ..=12.)
        .unwrap();
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    check(&s, 1., Vector3::try_new(0., 0., 0.).unwrap());
}
#[test]
fn native_two_rail_sweep_declines_unmatched_missing_duplicate_and_disconnected_sections() {
    let (rails, sections) = inputs(0.);
    for invalid in [
        vec![],
        vec![sections[0].clone()],
        vec![sections[0].clone(), sections[0].clone()],
        vec![
            sections[0].clone(),
            curve(vec![p(0., 0., 3.), p(2., 0., 4.)]),
        ],
        vec![
            curve(vec![p(0., 0., 1.), p(2., 0., 1.)]),
            sections[1].clone(),
        ],
    ] {
        assert!(
            two_rail_sweep_surface(&rails[0], &rails[1], &invalid, Tolerance::DEFAULT).is_err()
        );
    }
}
#[test]
fn native_two_rail_sweep_certifies_remote_geometry() {
    let (rails, sections) = inputs(1.);
    let shift = Vector3::try_new(1e9, -2e9, 3e9).unwrap();
    let tr = AffineTransform3::from_translation(shift);
    let rails = rails.map(|r| r.transformed(tr).unwrap());
    let sections = sections
        .iter()
        .map(|r| r.transformed(tr).unwrap())
        .collect::<Vec<_>>();
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    check(&s, 1., shift);
}

#[test]
fn native_two_rail_sweep_weight_gauges_and_resource_failures_are_explicit() {
    let (mut rails, mut sections) = inputs(0.);
    for (i, c) in rails.iter_mut().chain(&mut sections).enumerate() {
        *c = NurbsCurve::try_new_rational(
            c.degree(),
            c.control_points()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(p.point(), if i % 2 == 0 { -3. } else { 7. }).unwrap()
                })
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap();
    }
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    check(&s, 0., Vector3::try_new(0., 0., 0.).unwrap());
    let mut controls = sections[0].control_points().to_vec();
    controls[1] = WeightedPoint3::try_new(controls[1].point(), 2.).unwrap();
    sections[0] = NurbsCurve::try_new_rational(2, controls, sections[0].knots().to_vec()).unwrap();
    let before = format!("{rails:?}{sections:?}");
    assert!(
        two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT)
            .unwrap_err()
            .to_string()
            .contains("polynomial")
    );
    assert!(
        two_rail_sweep_surface(
            &rails[0],
            &rails[1],
            &vec![sections[1].clone(); 33],
            Tolerance::DEFAULT
        )
        .is_err()
    );
    assert_eq!(format!("{rails:?}{sections:?}"), before);
    let (rails, mut sections) = inputs(1.);
    sections.remove(1);
    assert!(
        two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT)
            .unwrap_err()
            .to_string()
            .contains("rail degree plus one")
    );
}

#[test]
fn native_two_rail_sweep_blends_different_cross_section_shapes() {
    let (rails, mut sections) = inputs(0.);
    sections[1] = curve(vec![p(0., 0., 5.), p(1., 2., 5.), p(2., 0., 5.)]);
    let s = two_rail_sweep_surface(&rails[0], &rails[1], &sections, Tolerance::DEFAULT).unwrap();
    for i in 0..=16 {
        for j in 0..=12 {
            let (u, v) = (i as f64 / 16., j as f64 / 12.);
            let expected = p(2. * v, 2. * (1. + u) * v * (1. - v), 5. * u);
            assert!(s.evaluate(u, v).unwrap().distance_to(expected).unwrap() < 1e-9);
        }
    }
}
