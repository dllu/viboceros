//! Whole-surface separation must account for every target row and stored weight.
#[path = "support/profile_rows.rs"]
mod profile_rows;
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, Tolerance, WeightedPoint3};

fn strip(offset: f64, rounded: bool) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        (0..2)
            .flat_map(|v| {
                (0..2).map(move |u| {
                    let rounding = if rounded && u == 1 { -1e-13 } else { 0. };
                    WeightedPoint3::try_new(
                        Point3::try_new(f64::from(v), offset + rounding, 5. * f64::from(u))
                            .unwrap(),
                        if rounded && u == 1 && v == 1 {
                            1. + 1e-15
                        } else {
                            1.
                        },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn exact_global_minimum_proves_parallel_strips_separated() {
    let source = strip(0., false);
    let target = strip(1e-4, false);
    let lower = profile_rows::complete_image_separation(&source, &target, 1e-7).unwrap();
    assert!(lower > 9.9e-5 && lower <= 1e-4, "{lower}");
    assert!(profile_rows::complete_bound(&source, &target, 1e-7).is_none());
}

#[test]
fn projected_row_rounding_and_nonseparable_weights_are_charged() {
    let source = strip(0., false);
    let exact = strip(1e-4, false);
    let rounded = strip(1e-4, true);
    let exact_lower = profile_rows::complete_image_separation(&source, &exact, 1e-7).unwrap();
    let rounded_lower = profile_rows::complete_image_separation(&source, &rounded, 1e-7).unwrap();
    assert!(rounded_lower > 9.9e-5 && rounded_lower < exact_lower);
    // The proposed projected reference is shifted, but the actual surface
    // includes points closer than epsilon. Its projection tube must prevent a
    // false proof; a floating-point comparison of first rows would miss this.
    let nearby = strip(1.0000005e-7, true);
    assert!(profile_rows::complete_image_separation(&source, &nearby, 1e-7).is_none());
}

#[test]
fn different_first_rows_do_not_separate_reversed_equal_images() {
    let source = strip(0., false);
    let reversed = source.try_reversed_u().unwrap();
    assert_ne!(source.control_points()[0], reversed.control_points()[0]);
    assert!(profile_rows::complete_image_separation(&source, &reversed, 1e-7).is_none());
    assert!(profile_rows::complete_bound(&source, &reversed, 1e-7).is_some());
}

#[test]
fn a_bad_local_proposal_cannot_certify_a_global_surface_gap() {
    let points = [[0., 0., 0.], [2., 2., 0.], [-2., 2., 0.], [0., 0., 0.]]
        .map(|p| Point3::try_from(p).unwrap());
    let knots = vec![0., 0., 0., 0., 1., 1., 1., 1.];
    let curve = NurbsCurve::try_new(3, points.to_vec(), knots.clone()).unwrap();
    let origin = points[0];
    let seed = curve
        .point_getter_parameter_from_seed(
            origin,
            0.5,
            Tolerance::try_new(1e-11, 1e-12, 1e-10).unwrap(),
        )
        .unwrap();
    assert!(curve.evaluate(seed).unwrap().distance_to(origin).unwrap() > 1.);
    // The local solver stops at a stationary point far from the origin, even
    // though the target contains the origin at both ends of its V domain.
    let target = NurbsSurface::try_new_rational(
        1,
        3,
        2,
        4,
        points
            .into_iter()
            .flat_map(|p| {
                [0., 5.].map(|z| {
                    WeightedPoint3::try_new(Point3::try_new(p.x(), p.y(), z).unwrap(), 1.).unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1.],
        knots,
    )
    .unwrap();
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [0., 5., 0., 5.]
            .map(|z| WeightedPoint3::try_new(Point3::try_new(0., 0., z).unwrap(), 1.).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(profile_rows::complete_image_separation(&source, &target, 1e-7).is_none());
}

#[test]
fn inactive_source_control_row_cannot_supply_an_image_witness() {
    let source = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    WeightedPoint3::try_new(
                        Point3::try_new(f64::from(v), f64::from(u), 0.).unwrap(),
                        1.,
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(profile_rows::complete_image_separation(&source, &strip(0., false), 1e-7).is_none());
}

#[test]
fn span_midpoints_reveal_gaps_hidden_at_every_uniform_station() {
    let mut points = Vec::new();
    for span in 0..64 {
        for control in usize::from(span != 0)..4 {
            let x = (f64::from(span) + control as f64 / 3.) / 64.;
            let y = if control == 1 || control == 2 {
                4e-6 / 3.
            } else {
                0.
            };
            for z in [0., 5.] {
                points
                    .push(WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), 1.).unwrap());
            }
        }
    }
    let mut knots = vec![0.; 4];
    for span in 1..64 {
        knots.extend([f64::from(span) / 64.; 3]);
    }
    knots.extend([1.; 4]);
    let source =
        NurbsSurface::try_new_rational(1, 3, 2, 193, points, vec![0., 0., 1., 1.], knots).unwrap();
    // Every i/64 node lies on the target. Each intervening cubic has a known
    // y peak of 1e-6; sampling those nodes cannot certify absence of a gap.
    for index in 0..=64 {
        assert_eq!(source.evaluate(0., f64::from(index) / 64.).unwrap().y(), 0.);
    }
    let lower = profile_rows::complete_image_separation(&source, &strip(0., false), 1e-7).unwrap();
    assert!(lower > 9.9e-7 && lower <= 1e-6, "{lower}");
}
