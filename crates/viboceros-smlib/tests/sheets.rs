#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3};
use viboceros_smlib::{Shape, Solid};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 3., 0.),
            point(0., 3., 0.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn sheet_round_trip_preserves_naked_edges_and_face_orientation() {
    for reversed in [false, true] {
        let mut input = plane();
        if reversed {
            input.reverse_orientation();
        }
        let before = input.clone();
        assert!(Solid::from_brep(&input, Tolerance::DEFAULT).is_err());
        let shape = Shape::from_brep(&input, Tolerance::DEFAULT).unwrap();
        let output = shape.to_brep(Tolerance::DEFAULT).unwrap();
        assert!(!output.is_solid());
        assert_eq!(output.edge_use_counts(), vec![1; 4]);
        let surface = output.faces()[0].surface();
        let normal = surface
            .normal_at(
                surface.parameter_at_u(0.5).unwrap(),
                surface.parameter_at_v(0.5).unwrap(),
            )
            .unwrap();
        assert_eq!(normal.as_vector().z() < 0., reversed);
        assert!((output.area(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-10);
        assert_eq!(input, before);
    }
}

#[test]
fn native_planar_offsets_and_walls_preserve_source_geometry() {
    let input = plane();
    let before = input.clone();
    let shape = Shape::from_brep(&input, Tolerance::DEFAULT).unwrap();
    for distance in [0.5, -0.5] {
        for solid in [false, true] {
            let output = shape
                .offset(distance, solid)
                .unwrap()
                .to_brep_approximated(Tolerance::DEFAULT, 1e-8)
                .unwrap();
            assert_eq!(output.is_solid(), solid);
            if solid {
                assert!((output.signed_volume(Tolerance::DEFAULT).unwrap() - 6.).abs() < 1e-8);
                assert!(output.vertices().iter().all(|v| {
                    (distance.min(0.) - 1e-8..=distance.max(0.) + 1e-8).contains(&v.point().z())
                }));
            } else {
                assert!(
                    output
                        .vertices()
                        .iter()
                        .all(|v| (v.point().z() - distance).abs() < 1e-8)
                );
                assert!((output.area(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-8);
            }
        }
    }
    assert_eq!(
        shape
            .to_brep(Tolerance::DEFAULT)
            .unwrap()
            .area(Tolerance::DEFAULT)
            .unwrap(),
        12.
    );
    assert_eq!(input, before);
    for distance in [0., f64::NAN, f64::INFINITY] {
        assert!(shape.offset(distance, false).is_err());
    }
}

#[test]
fn periodic_sheet_round_trip_retains_seams_and_boundary_curves() {
    let frame = Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let input = Brep::try_surface_face(
        NurbsSurface::try_cylinder(frame, 2., 0., 3.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let output = Shape::from_brep(&input, Tolerance::DEFAULT)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(output.faces().len(), 1);
    assert!(!output.is_solid());
    assert!(output.edge_use_counts().contains(&2));
    assert!((output.area(Tolerance::DEFAULT).unwrap() - 12. * std::f64::consts::PI).abs() < 1e-8);
}

#[test]
fn single_support_export_checks_budget_face_count_and_source_immutability() {
    for reversed in [false, true] {
        let input = if reversed {
            plane().reversed()
        } else {
            plane()
        };
        let shape = Shape::from_brep(&input, Tolerance::DEFAULT).unwrap();
        let before = shape.to_brep(Tolerance::DEFAULT).unwrap();
        for distance in [-0.5, 0.5] {
            let offset = shape.offset(distance, false).unwrap();
            let surface = offset.to_single_surface_approximated(1e-5).unwrap();
            for u in [0., 0.3, 1.] {
                for v in [0., 0.7, 1.] {
                    let p = surface
                        .evaluate(
                            surface.parameter_at_u(u).unwrap(),
                            surface.parameter_at_v(v).unwrap(),
                        )
                        .unwrap();
                    assert!((p.z() - distance * if reversed { -1. } else { 1. }).abs() < 1e-10);
                }
            }
        }
        for budget in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(shape.to_single_surface_approximated(budget).is_err());
        }
        assert!(
            shape
                .offset(0.5, true)
                .unwrap()
                .to_single_surface_approximated(1e-5)
                .is_err()
        );
        assert_eq!(shape.to_brep(Tolerance::DEFAULT).unwrap(), before);
        assert_eq!(
            input,
            if reversed {
                plane().reversed()
            } else {
                plane()
            }
        );
    }
}

#[test]
fn curved_support_export_respects_authored_normal_offset_stations() {
    let heights = [
        [0., 0.2, -0.1, 0.3],
        [0.1, 0.8, 0.7, -0.2],
        [0., 0.6, 1.1, 0.1],
        [0.2, 0.1, 0., -0.2],
    ];
    let controls = heights
        .into_iter()
        .enumerate()
        .flat_map(|(j, row)| {
            row.into_iter()
                .enumerate()
                .map(move |(i, z)| point(i as f64 * 1.7, j as f64 * 1.2, z))
        })
        .collect();
    let knots = vec![0., 0., 0., 0., 1., 1., 1., 1.];
    let source = NurbsSurface::try_new(3, 3, 4, 4, controls, knots.clone(), knots).unwrap();
    let brep = Brep::try_surface_face(source.clone(), Tolerance::DEFAULT).unwrap();
    let shape = Shape::from_brep(&brep, Tolerance::DEFAULT).unwrap();
    let before = shape.to_brep(Tolerance::DEFAULT).unwrap();
    for distance in [-0.3, 0.5] {
        let started = std::time::Instant::now();
        let fitted = shape
            .offset_with_accuracy(distance, false, 2.5e-5)
            .unwrap()
            .to_single_surface_approximated(2.5e-5)
            .unwrap();
        for u in [0., 0.25, 0.5, 0.75, 1.] {
            for v in [0., 0.25, 0.5, 0.75, 1.] {
                let p = source
                    .evaluate(u, v)
                    .unwrap()
                    .translated(
                        source
                            .normal_at(u, v)
                            .unwrap()
                            .as_vector()
                            .scaled(distance)
                            .unwrap(),
                    )
                    .unwrap();
                assert!(p.distance_to(fitted.evaluate(u, v).unwrap()).unwrap() <= 1e-4);
            }
        }
        println!(
            "OFFSET_SUPPORT {distance} {}x{} {:?}",
            fitted.control_point_count_u(),
            fitted.control_point_count_v(),
            started.elapsed()
        );
    }
    assert_eq!(shape.to_brep(Tolerance::DEFAULT).unwrap(), before);
}
