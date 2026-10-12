#![cfg(feature = "native")]
use viboceros_geometry::{
    Curve3, LineSegment, NurbsCurve, Point3, Sweep1, SweepBlend, SweepFrameStyle, SweepSection,
    Tolerance, Vector3,
};
use viboceros_smlib::sweep_model;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn native_common_section_matching_preserves_local_global_and_roadlike_sweep_models() {
    let rail = Curve3::Line(
        LineSegment::try_new(p(0., 0., 0.), p(0., 0., 10.), Tolerance::DEFAULT).unwrap(),
    );
    let sections = vec![
        SweepSection {
            parameter: 0.,
            curve: NurbsCurve::try_new(
                1,
                vec![p(-1., 0., 0.), p(1., 0., 0.)],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        },
        SweepSection {
            parameter: 5.,
            curve: NurbsCurve::try_new(
                2,
                vec![
                    p(-2., 0., 5.),
                    p(-1., 2., 5.),
                    p(1., -1., 5.),
                    p(2., 0., 5.),
                ],
                vec![-2., -1., 0., 0.8, 2., 3., 4.],
            )
            .unwrap(),
        },
        SweepSection {
            parameter: 10.,
            curve: NurbsCurve::try_new(
                3,
                vec![
                    p(-1., 0., 10.),
                    p(0., -2., 10.),
                    p(1., 2., 10.),
                    p(2., 0., 10.),
                ],
                vec![0., 0., 0., 0., 1., 1., 1., 1.],
            )
            .unwrap(),
        },
    ];
    let before = sections.clone();
    for style in [
        SweepFrameStyle::Freeform,
        SweepFrameStyle::Roadlike(
            Vector3::try_new(1., 0., 0.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
        ),
    ] {
        for blend in [SweepBlend::Local, SweepBlend::Global] {
            let native =
                sweep_model(rail.as_ref(), &sections, style, blend, Tolerance::DEFAULT).unwrap();
            let portable =
                Sweep1::try_new(rail.as_ref(), &sections, style, blend, Tolerance::DEFAULT)
                    .unwrap();
            assert_eq!(native.domain(), portable.domain());
            let stations = [0., 2.5, 5., 7.5, 10.];
            let actual = native.sections_at(&stations).unwrap();
            let expected = portable.sections_at(&stations).unwrap();
            for (a, e) in actual.iter().zip(expected.iter()) {
                assert_eq!(a.degree(), e.degree());
                assert_eq!(a.knots(), e.knots());
                for i in 0..=32 {
                    let t = a.parameter_at(i as f64 / 32.).unwrap();
                    assert!(
                        a.evaluate(t)
                            .unwrap()
                            .distance_to(e.evaluate(t).unwrap())
                            .unwrap()
                            < 1e-9
                    );
                }
            }
            let actual = native.to_surface().unwrap();
            let expected = portable.to_surface().unwrap();
            assert_eq!(actual.degree_u(), expected.degree_u());
            assert_eq!(actual.degree_v(), expected.degree_v());
            for i in 0..=8 {
                for j in 0..=8 {
                    let a = actual
                        .evaluate(
                            actual.parameter_at_u(i as f64 / 8.).unwrap(),
                            actual.parameter_at_v(j as f64 / 8.).unwrap(),
                        )
                        .unwrap();
                    let e = expected
                        .evaluate(
                            expected.parameter_at_u(i as f64 / 8.).unwrap(),
                            expected.parameter_at_v(j as f64 / 8.).unwrap(),
                        )
                        .unwrap();
                    assert!(a.distance_to(e).unwrap() < 1e-6);
                }
            }
        }
    }
    assert_eq!(sections, before);
}
