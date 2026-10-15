#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_smlib::{ChamferRailType, Solid};
fn source() -> Brep {
    let t = Tolerance::DEFAULT;
    let plane = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        t,
    )
    .unwrap();
    Brep::try_box(plane, [[0., 10.], [0., 8.], [0., 6.]], t).unwrap()
}
#[test]
fn chamfer_every_original_box_edge_matches_width_and_rolling_ball_volumes() {
    let b = source();
    let original = b.clone();
    let t = Tolerance::DEFAULT;
    for rail in [
        ChamferRailType::DistanceBetweenRails,
        ChamferRailType::RollingBall,
    ] {
        for i in 0..b.edges().len() {
            let e = &b.edges()[i];
            let [a, c] = e.vertices().map(|v| b.vertices()[v].point());
            let length = a.distance_to(c).unwrap();
            let result = Solid::chamfer_brep_edges(&b, &[i, i], 1., rail, t)
                .unwrap()
                .to_brep(t)
                .unwrap();
            let expected = 480.
                - length
                    * if rail == ChamferRailType::DistanceBetweenRails {
                        0.25
                    } else {
                        0.5
                    };
            let volume = result.signed_volume(t).unwrap();
            assert!(result.is_solid());
            assert!(
                (volume - expected).abs() < 1e-6,
                "edge {i}, {rail:?}: {volume} != {expected}"
            );
            assert_eq!(b, original);
        }
    }
}
#[test]
fn chamfer_all_edges_and_invalid_sizes_preserve_source() {
    let b = source();
    let original = b.clone();
    let t = Tolerance::DEFAULT;
    let all = (0..b.edges().len()).collect::<Vec<_>>();
    let result = Solid::chamfer_brep_edges(&b, &all, 1., ChamferRailType::RollingBall, t)
        .unwrap()
        .to_brep(t)
        .unwrap();
    assert!(result.is_solid());
    assert!(result.faces().len() > 6);
    // Remove twelve triangular prisms; inclusion/exclusion at eight corners
    // adds 8*(3*(1/3)-1/4)=6 for the three pairwise chamfer halfspaces.
    let expected = 480. - 2. * (10. + 8. + 6.) + 6.;
    let intersection = Point3::try_new(0.5, 0.5, 0.5).unwrap();
    assert!(
        result
            .vertices()
            .iter()
            .any(|v| v.point().distance_to(intersection).unwrap() < 1e-7)
    );
    assert!(
        (result.signed_volume(t).unwrap() - expected).abs() < 1e-5,
        "all-edge volume: {}",
        result.signed_volume(t).unwrap()
    );
    for (edges, distance) in [
        (vec![], 1.),
        (vec![999], 1.),
        (vec![0], 0.),
        (vec![0], f64::NAN),
        (all, 100.),
    ] {
        assert!(
            Solid::chamfer_brep_edges(
                &b,
                &edges,
                distance,
                ChamferRailType::DistanceBetweenRails,
                t
            )
            .and_then(|s| s.to_brep(t))
            .is_err()
        );
        assert_eq!(b, original);
    }
}

#[test]
fn chamfer_acute_and_obtuse_prism_edges_match_independent_cross_sections() {
    let t = Tolerance::DEFAULT;
    for shear in [-0.5, 0.5] {
        let transform = viboceros_geometry::AffineTransform3::try_new(
            [[1., shear, 0.], [0., 1., 0.], [0., 0., 1.]],
            Vector3::try_new(0., 0., 0.).unwrap(),
        )
        .unwrap();
        let b = source().transformed(transform, t).unwrap();
        let edge = b
            .edges()
            .iter()
            .position(|e| {
                e.vertices()
                    .map(|v| b.vertices()[v].point())
                    .iter()
                    .all(|p| p.x().abs() < 1e-8 && p.y().abs() < 1e-8)
            })
            .unwrap();
        let angle = 8_f64.atan2(shear * 8.);
        let cot = 1. / (angle / 2.).tan();
        for rail in [
            ChamferRailType::DistanceBetweenRails,
            ChamferRailType::RollingBall,
        ] {
            let setback = if rail == ChamferRailType::DistanceBetweenRails {
                1. / (2. * (angle / 2.).sin())
            } else {
                cot
            };
            let expected = 480. - 0.5 * setback.powi(2) * angle.sin() * 6.;
            let result = Solid::chamfer_brep_edges(&b, &[edge], 1., rail, t)
                .unwrap()
                .to_brep(t)
                .unwrap();
            assert!(
                (result.signed_volume(t).unwrap() - expected).abs() < 1e-6,
                "shear={shear}, rail={rail:?}"
            );
        }
    }
}
#[test]
fn chamfer_cylinder_rim_matches_conical_revolution_volume() {
    let t = Tolerance::DEFAULT;
    let plane = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        t,
    )
    .unwrap();
    let b = Brep::try_cylinder(plane, 5., 0., 8., t).unwrap();
    let index = b
        .edges()
        .iter()
        .enumerate()
        .filter(|(i, _)| b.edges_shared_by_distinct_faces()[*i])
        .find(|(_, e)| {
            let d = e.curve().domain();
            e.curve().evaluate((d.start() + d.end()) / 2.).unwrap().z() > 7.999
        })
        .unwrap()
        .0;
    for rail in [
        ChamferRailType::DistanceBetweenRails,
        ChamferRailType::RollingBall,
    ] {
        let s = if rail == ChamferRailType::DistanceBetweenRails {
            1. / 2_f64.sqrt()
        } else {
            1.
        };
        let expected =
            200. * std::f64::consts::PI - std::f64::consts::PI * (5. * s * s - s * s * s / 3.);
        let result = Solid::chamfer_brep_edges(&b, &[index], 1., rail, t)
            .unwrap()
            .to_brep(t)
            .unwrap();
        assert!(result.is_solid());
        assert!(
            (result.signed_volume(t).unwrap() - expected).abs() < 1e-5,
            "rail={rail:?}"
        );
        for face in result.faces() {
            for u in [0.2, 0.4, 0.6, 0.8] {
                for v in [0.2, 0.4, 0.6, 0.8] {
                    let a = face.surface().parameter_at_u(u).unwrap();
                    let b = face.surface().parameter_at_v(v).unwrap();
                    if !face.contains_parameters(a, b, t).unwrap() {
                        continue;
                    }
                    let p = face.surface().evaluate(a, b).unwrap();
                    let r = p.x().hypot(p.y());
                    let z = p.z();
                    let e = 1e-7;
                    let bottom = z.abs() < e && r <= 5. + e;
                    let top = (z - 8.).abs() < e && r <= 5. - s + e;
                    let wall = (r - 5.).abs() < e && z >= -e && z <= 8. - s + e;
                    let bevel = (r + z - (13. - s)).abs() < e && z >= 8. - s - e && z <= 8. + e;
                    assert!(
                        bottom || top || wall || bevel,
                        "point {p:?} leaves the independent chamfer boundary"
                    );
                }
            }
        }
    }
}

#[test]
fn chamfer_cylinder_tolerance_and_seam_frames() {
    for abs in [1e-9, 1e-6] {
        for normal_frame in [false, true] {
            let t = Tolerance::try_new(abs, 1e-12, 1e-10).unwrap();
            let origin = Point3::try_new(0., 0., 0.).unwrap();
            let z = Vector3::try_new(0., 0., 1.).unwrap();
            let frame = if normal_frame {
                Frame3::try_from_normal(origin, z, t).unwrap()
            } else {
                Frame3::try_from_directions(
                    origin,
                    Vector3::try_new(1., 0., 0.).unwrap(),
                    Vector3::try_new(0., 1., 0.).unwrap(),
                    t,
                )
                .unwrap()
            };
            let b = Brep::try_cylinder(frame, 5., 0., 8., t).unwrap();
            let shared = b.edges_shared_by_distinct_faces();
            let e = b
                .edges()
                .iter()
                .enumerate()
                .find(|(i, e)| {
                    let d = e.curve().domain();
                    shared[*i]
                        && e.curve().evaluate((d.start() + d.end()) / 2.).unwrap().z() > 7.999
                })
                .unwrap()
                .0;
            let r =
                Solid::chamfer_brep_edges(&b, &[e], 1., ChamferRailType::DistanceBetweenRails, t)
                    .and_then(|s| s.to_brep(t));
            println!(
                "abs={abs}, normal={normal_frame}, result={:?}",
                r.as_ref().map(|b| (b.faces().len(), b.signed_volume(t)))
            );
            assert!(r.is_ok(), "abs={abs}, normal={normal_frame}");
        }
    }
}

#[test]
fn chamfer_concave_notch_adds_the_expected_wedge_without_changing_source() {
    use viboceros_smlib::BooleanOperation;
    let t = Tolerance::DEFAULT;
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let outer = Solid::box_solid(p(0., 0., 0.), [10., 10., 10.]).unwrap();
    let cutter = Solid::box_solid(p(5., 5., -1.), [6., 6., 12.]).unwrap();
    let b = outer
        .boolean(&cutter, BooleanOperation::Difference)
        .unwrap()
        .to_brep(t)
        .unwrap();
    let original = b.clone();
    let edge = b
        .edges()
        .iter()
        .position(|e| {
            e.vertices()
                .map(|v| b.vertices()[v].point())
                .iter()
                .all(|p| (p.x() - 5.).abs() < 1e-8 && (p.y() - 5.).abs() < 1e-8)
        })
        .unwrap();
    for (rail, extra) in [
        (ChamferRailType::DistanceBetweenRails, 2.5),
        (ChamferRailType::RollingBall, 5.),
    ] {
        let result = Solid::chamfer_brep_edges(&b, &[edge], 1., rail, t)
            .unwrap()
            .to_brep(t)
            .unwrap();
        assert!(result.is_solid());
        assert!((result.signed_volume(t).unwrap() - (750. + extra)).abs() < 1e-5);
        assert_eq!(b, original);
    }
}
