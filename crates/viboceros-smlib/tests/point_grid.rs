#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{NurbsSurface, Point3};
use viboceros_smlib::interpolate_point_grid;

fn compare(a: &NurbsSurface, b: &NurbsSurface, epsilon: f64) {
    assert_eq!((a.degree_u(), a.degree_v()), (b.degree_u(), b.degree_v()));
    assert_eq!(a.knots_u(), b.knots_u());
    assert_eq!(a.knots_v(), b.knots_v());
    assert_eq!(a.control_points().len(), b.control_points().len());
    assert_eq!(
        (a.is_periodic_u(), a.is_periodic_v()),
        (b.is_periodic_u(), b.is_periodic_v())
    );
    assert_eq!(
        (a.is_closed_u().unwrap(), a.is_closed_v().unwrap()),
        (b.is_closed_u().unwrap(), b.is_closed_v().unwrap())
    );
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert_eq!(a.weight(), 1.);
        for (a, b) in a.point().to_array().into_iter().zip(b.point().to_array()) {
            assert!(
                (a - b).abs() <= epsilon + 1e-12 * a.abs().max(b.abs()),
                "{a} != {b}"
            );
        }
    }
    for j in 0..=8 {
        for i in 0..=8 {
            let u = a.parameter_at_u(i as f64 / 8.).unwrap();
            let v = a.parameter_at_v(j as f64 / 8.).unwrap();
            assert!(
                a.evaluate(u, v)
                    .unwrap()
                    .distance_to(b.evaluate(u, v).unwrap())
                    .unwrap()
                    <= epsilon * 4.
            );
        }
    }
}

#[test]
fn native_point_grids_match_portable_complete_nets_for_permanent_api_fixtures() {
    let mut cases = 0;
    for (text, epsilon) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/point_grid.json"),
            2e-12,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/point_grid_high_degree.json"),
            1e-8,
        ),
    ] {
        let request: Value = serde_json::from_str(text).unwrap();
        for f in request["operations"].as_array().unwrap() {
            if f["control"] == true || f["command"] == true {
                continue;
            }
            let count: [usize; 2] =
                std::array::from_fn(|a| f["count"][a].as_u64().unwrap() as usize);
            let degree = std::array::from_fn(|a| f["degree"][a].as_u64().unwrap() as usize);
            let closed = std::array::from_fn(|a| f["closed"][a].as_bool().unwrap_or(false));
            let points = (0..count[1])
                .flat_map(|v| (0..count[0]).map(move |u| (u, v)))
                .map(|(u, v)| {
                    Point3::try_from(std::array::from_fn(|a| {
                        f["points"][u * count[1] + v][a].as_f64().unwrap()
                    }))
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let before = points.clone();
            let a = interpolate_point_grid(&points, count, degree, closed)
                .unwrap_or_else(|e| panic!("{}: {e}", f["id"]));
            let b = NurbsSurface::try_through_point_grid(&points, count, degree, closed).unwrap();
            compare(&a, &b, epsilon);
            assert_eq!(points, before);
            cases += 1;
        }
    }
    assert_eq!(cases, 47);
}

#[test]
fn native_closed_grids_keep_repeated_controls_and_extreme_coordinate_scales() {
    let points = (0..5)
        .flat_map(|v| {
            (0..6)
                .map(move |u| Point3::try_new(u as f64, v as f64, (u * v) as f64 * 0.125).unwrap())
        })
        .collect::<Vec<_>>();
    for closed in [[false, true], [true, false], [true, true]] {
        let base = interpolate_point_grid(&points, [6, 5], [3; 2], closed).unwrap();
        compare(
            &base,
            &NurbsSurface::try_through_point_grid(&points, [6, 5], [3; 2], closed).unwrap(),
            1e-11,
        );
        let n = base.control_point_count_u();
        if closed[0] {
            for j in 0..base.control_point_count_v() {
                for i in 0..3 {
                    assert_eq!(
                        base.control_points()[j * n + i],
                        base.control_points()[j * n + 6 + i]
                    );
                }
            }
        }
        if closed[1] {
            for j in 0..3 {
                assert_eq!(
                    &base.control_points()[j * n..(j + 1) * n],
                    &base.control_points()[(5 + j) * n..(6 + j) * n]
                );
            }
        }
        for scale in [2.0_f64.powi(-360), 2.0_f64.powi(360)] {
            let points = points
                .iter()
                .map(|p| Point3::try_from(p.to_array().map(|x| x * scale)).unwrap())
                .collect::<Vec<_>>();
            let actual = interpolate_point_grid(&points, [6, 5], [3; 2], closed).unwrap();
            for (a, b) in actual.control_points().iter().zip(base.control_points()) {
                for (a, b) in a.point().to_array().into_iter().zip(b.point().to_array()) {
                    assert!((a / scale - b).abs() < 1e-11);
                }
            }
        }
    }
}

#[test]
fn native_minimum_closed_grids_clamp_degrees_and_reject_unsolvable_inputs() {
    let points = (0..3)
        .flat_map(|v| {
            (0..3)
                .map(move |u| Point3::try_new(u as f64, v as f64, (u * v) as f64 * 0.125).unwrap())
        })
        .collect::<Vec<_>>();
    for degree in [0, 1, 2, 13] {
        let a = interpolate_point_grid(&points, [3; 2], [degree; 2], [true; 2]).unwrap();
        let b =
            NurbsSurface::try_through_point_grid(&points, [3; 2], [degree; 2], [true; 2]).unwrap();
        compare(&a, &b, 1e-11);
    }
    for count in [[0, 0], [2, 3], [3, 4], [usize::MAX, 3]] {
        assert!(interpolate_point_grid(&points, count, [3; 2], [true; 2]).is_err());
    }
    assert!(interpolate_point_grid(&[points[0]; 9], [3; 2], [3; 2], [true; 2]).is_err());
}

#[test]
fn native_closed_scalar_solves_retain_nonconstant_tiny_channels() {
    for count in [[6, 5], [64, 65]] {
        let grid = |factor| {
            (0..count[1])
                .flat_map(|v| {
                    (0..count[0]).map(move |u| {
                        Point3::try_new(u as f64, v as f64, (u * v) as f64 * factor).unwrap()
                    })
                })
                .collect::<Vec<_>>()
        };
        // Z does not affect any rounded mean chord at either scale. Both runs
        // therefore solve the same basis with exactly scaled nonconstant RHS.
        let reference = interpolate_point_grid(&grid(1e-100), count, [3; 2], [true; 2]).unwrap();
        let tiny = interpolate_point_grid(&grid(1e-280), count, [3; 2], [true; 2]).unwrap();
        assert_eq!(reference.knots_u(), tiny.knots_u());
        assert_eq!(reference.knots_v(), tiny.knots_v());
        for (a, b) in tiny.control_points().iter().zip(reference.control_points()) {
            assert!((a.point().z() / 1e-280 - b.point().z() / 1e-100).abs() < 1e-10);
        }
    }
}

#[test]
fn native_large_closed_grids_match_portable_nets_across_all_degrees_and_both_axes() {
    for degree in 1..=11 {
        let n = 64;
        let points = (0..7)
            .flat_map(|v| {
                (0..n).map(move |u| {
                    let x = u as f64 * (1. + 0.001 * u as f64);
                    Point3::try_new(x, v as f64, (x * 0.21).sin() * (1. + 0.13 * v as f64)).unwrap()
                })
            })
            .collect::<Vec<_>>();
        let a = interpolate_point_grid(&points, [n, 7], [degree, 3], [true, false]).unwrap();
        let b = NurbsSurface::try_through_point_grid(&points, [n, 7], [degree, 3], [true, false])
            .unwrap();
        compare(&a, &b, 1e-8);
    }
    let points = (0..65)
        .flat_map(|v| {
            (0..64).map(move |u| {
                Point3::try_new(
                    u as f64,
                    v as f64,
                    (u as f64 * 0.17).sin() * (v as f64 * 0.23).cos(),
                )
                .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let a = interpolate_point_grid(&points, [64, 65], [3, 4], [true, true]).unwrap();
    let b = NurbsSurface::try_through_point_grid(&points, [64, 65], [3, 4], [true, true]).unwrap();
    compare(&a, &b, 1e-10);
    assert_eq!(a.control_points()[0], a.control_points()[64]);
    assert_eq!(a.control_points()[0], a.control_points()[65 * 67]);
}

#[test]
fn native_closed_bands_construct_the_full_point_grid_budget() {
    let points = (0..256)
        .flat_map(|v| {
            (0..256).map(move |u| {
                Point3::try_new(
                    u as f64,
                    v as f64,
                    (u as f64 * 0.07).sin() * (v as f64 * 0.11).cos(),
                )
                .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let a = interpolate_point_grid(&points, [256; 2], [3; 2], [true; 2]).unwrap();
    assert_eq!(a.control_point_count_u(), 259);
    assert_eq!(a.control_point_count_v(), 259);
    assert!(a.is_periodic_u() && a.is_periodic_v());
    assert!(a.is_closed_u().unwrap() && a.is_closed_v().unwrap());
    assert!(a.control_points().iter().all(|p| p.weight() == 1.));
    assert_eq!(a.control_points()[0], a.control_points()[256]);
    assert_eq!(a.control_points()[0], a.control_points()[256 * 259]);
    for j in [0, 17, 128, 255] {
        for i in [0, 17, 128, 255] {
            let u = a.knots_u()[i + 1..=i + 3].iter().sum::<f64>() / 3.;
            let v = a.knots_v()[j + 1..=j + 3].iter().sum::<f64>() / 3.;
            assert!(
                a.evaluate_extended(u, v)
                    .unwrap()
                    .distance_to(points[j * 256 + i])
                    .unwrap()
                    < 1e-9
            );
        }
    }
}
