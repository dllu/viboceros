use viboceros_geometry::{LineSegment, NurbsCurve, Point3, Tolerance, WeightedPoint3};

fn circle(z: f64, dent: bool, gauge: f64) -> NurbsCurve {
    let mut points = [
        [2., 0.],
        [2., 2.],
        [0., 2.],
        [-2., 2.],
        [-2., 0.],
        [-2., -2.],
        [0., -2.],
        [2., -2.],
        [2., 0.],
    ];
    if dent {
        points[1] = [0., 0.];
    }
    NurbsCurve::try_new_rational(
        2,
        points
            .into_iter()
            .enumerate()
            .map(|(i, [x, y])| {
                WeightedPoint3::try_new(
                    Point3::try_new(x, y, z).unwrap(),
                    gauge * if i % 2 == 0 { 1. } else { 0.5_f64.sqrt() },
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
    )
    .unwrap()
}

fn rail(start: f64, end: f64) -> NurbsCurve {
    LineSegment::try_new(
        Point3::try_new(0., 0., start).unwrap(),
        Point3::try_new(0., 0., end).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn radial_minima_certify_full_circles_at_end_and_interior_stations() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-12).unwrap();
    for z in [0., 2.5, 5.] {
        for gauge in [1., -1e-100, 1e100] {
            let section = circle(z, false, gauge);
            for rail in [rail(0., 5.), rail(5., 0.)] {
                for (a, b, index) in [(&rail, &section, 0), (&section, &rail, 1)] {
                    let before = [a.clone(), b.clone()];
                    let m = a
                        .minimum_distance_with_curve(b, tolerance)
                        .unwrap()
                        .expect("complete radial minimum");
                    assert!((m.upper_distance - 2.).abs() <= 1e-9, "{m:?}");
                    assert!((m.points[index].z() - z).abs() <= 1e-8, "{m:?}");
                    assert_eq!(m.maximum_error, 1e-9);
                    assert_eq!([a.clone(), b.clone()], before);
                }
            }
        }
    }
}

#[test]
fn a_dented_rational_section_cannot_be_certified_as_a_circle() {
    let section = circle(0., true, 1.);
    let axis = rail(-1., 1.);
    let expected = 2_f64.sqrt() / (1. + 0.5_f64.sqrt());
    for (a, b, seed) in [(&axis, &section, [0., 0.]), (&section, &axis, [0., 0.])] {
        let m = a
            .minimum_distance_with_curve_seeded(
                b,
                Tolerance::try_new(1e-9, 1e-12, 1e-12).unwrap(),
                seed,
            )
            .unwrap()
            .expect("global minimum must improve an inaccurate seed");
        assert!((m.upper_distance - expected).abs() <= 1e-9, "{m:?}");
    }
}
