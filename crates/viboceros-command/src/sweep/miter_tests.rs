use super::*;
use viboceros_geometry::{Circle3, Vector3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn add_circle(document: &mut Document, center: Point3, a: Real, b: Real, radius: Real) -> ObjectId {
    let unit = |x, y| {
        Vector3::try_new(x, y, 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
    };
    let circle = Circle3::try_from_frame(
        center,
        radius,
        unit(-b, a),
        unit(a, b),
        document.tolerance(),
    )
    .unwrap();
    document.add_geometry(Geometry::Circle(circle)).unwrap()
}

#[test]
fn sweep_miters_place_a_closed_profile_around_the_rail_corner() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    registry
        .execute(&mut document, "Polyline 0,0,0 5,0,0 5,5,0")
        .unwrap();
    let mut ids = vec![document.objects().last().unwrap().id()];
    for (center, a, b, radius) in [
        (point(0., 0., 0.), 1., 0., 1.),
        (point(5., 0., 0.), 1., 1., 1.2),
        (point(5., 5., 0.), 0., 1., 1.5),
    ] {
        ids.push(add_circle(&mut document, center, a, b, radius));
    }
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    for command in ["Sweep1", "Sweep1 UntrimmedMiters=Yes RefitRail=Yes"] {
        registry.execute(&mut document, command).unwrap();
        let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
            panic!("expected a Sweep1 BRep");
        };
        // Rhino 8.32 with automatic profile placement and the natural seams.
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (2, 5, 3)
        );
        assert!((brep.area(tolerance).unwrap() - 71.893_014_28).abs() < 2e-4);
        let first = brep.faces()[0].surface();
        let middle = first.isocurve_v(*first.domain_u().end()).unwrap();
        let q = 1.2 * std::f64::consts::FRAC_1_SQRT_2;
        let seam = middle.evaluate(*middle.domain().start()).unwrap();
        assert!(seam.distance_to(point(5. - q, q, 0.)).unwrap() < 1e-8);
        document.undo().unwrap();
    }
}

#[test]
fn sweep_miters_join_point_ends_to_a_shared_corner_profile() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in ["Polyline 0,0,0 5,0,0 5,5,0", "Line 5,0,0 4,1,1"] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "Sweep1 StartPoint=0,0,0 EndPoint=5,5,0 UntrimmedMiters=Yes",
        )
        .unwrap();
    let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
        panic!("expected a Sweep1 BRep");
    };
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (2, 5, 4)
    );
    for (face, pole, u) in [
        (&brep.faces()[0], point(0., 0., 0.), 0.),
        (&brep.faces()[1], point(5., 5., 0.), 10.),
    ] {
        let c = face.surface().isocurve_v(u).unwrap();
        assert!(c.control_points().iter().all(|p| p.point() == pole));
    }
    assert!((brep.area(document.tolerance()).unwrap() - 5. * 2_f64.sqrt()).abs() < 1e-7);
}

#[test]
fn sweep_miters_transport_the_return_section_around_a_spatial_loop() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Polyline 0,0,0 5,0,0 5,4,2 0,4,0 0,0,0",
        "Line 0,0,0 1,1,0.5",
        "Line 5,0,0 4,1,2",
        "Line 5,4,2 4,3,2",
        "Line 0,4,0 1,3,1",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    for (options, topology, tip, area) in [
        (
            "Closed=Yes UntrimmedMiters=Yes",
            (4, 13, 9),
            point(1.071523002947292, -1., 0.3191213784045467),
            21.13494938,
        ),
        (
            "Closed=Yes UntrimmedMiters=No",
            (6, 19, 14),
            point(1.071523002947291, -3.3233944016987693, 0.3191213784045462),
            26.33021720,
        ),
    ] {
        registry
            .execute(&mut document, &format!("Sweep1 {options}"))
            .unwrap();
        let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
            panic!("expected a Sweep1 BRep");
        };
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            topology
        );
        let last = brep.faces().last().unwrap().surface();
        let actual = last
            .evaluate(*last.domain_u().end(), *last.domain_v().end())
            .unwrap();
        assert!(actual.distance_to(tip).unwrap() < 1e-8, "{actual:?}");
        let measured = brep.area(document.tolerance()).unwrap();
        assert!((measured - area).abs() < 5e-5, "{measured} != {area}");
        document.undo().unwrap();
    }
}

#[test]
fn sweep_miters_place_round_profiles_at_polygonal_loop_corners() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Polyline 0,0,0 5,0,0 5,5,0 0,5,0 0,0,0")
        .unwrap();
    let mut ids = vec![document.objects().last().unwrap().id()];
    for (center, a, b, radius) in [
        (point(0., 0., 0.), 1., -1., 0.8),
        (point(5., 0., 0.), 1., 1., 1.0),
        (point(5., 5., 0.), -1., 1., 1.2),
        (point(0., 5., 0.), -1., -1., 1.4),
    ] {
        ids.push(add_circle(&mut document, center, a, b, radius));
    }
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Sweep1 Closed=Yes UntrimmedMiters=Yes")
        .unwrap();
    let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
        panic!("expected a Sweep1 BRep");
    };
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (4, 9, 5)
    );
    assert!((brep.area(document.tolerance()).unwrap() - 119.18477541).abs() < 3e-4);
    let last = brep.faces().last().unwrap().surface();
    let seam = last
        .evaluate(*last.domain_u().end(), *last.domain_v().start())
        .unwrap();
    let q = 0.8 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(seam.distance_to(point(q, -q, 0.)).unwrap() < 1e-8);
}

#[test]
fn sweep_miters_sew_an_untrimmed_polygonal_loop_at_a_smooth_seam() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Polyline 0,0,0 5,0,0 5,5,0 0,5,0 0,0,0",
        "Line 2.5,0,0 2.5,1,1",
        "Line 5,0,0 4,1,1",
        "Line 5,5,0 4,4,1",
        "Line 0,5,0 1,4,1",
        "Line 0,0,0 1,1,1",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Sweep1 Closed=Yes UntrimmedMiters=Yes")
        .unwrap();
    let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
        panic!("expected a Sweep1 BRep");
    };
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (5, 15, 10)
    );
    assert_eq!(
        brep.edge_use_counts()
            .iter()
            .filter(|&&uses| uses == 2)
            .count(),
        5
    );
    assert!((brep.area(document.tolerance()).unwrap() - 22.62741700).abs() < 5e-5);
}

#[test]
fn sweep_miters_infer_sections_for_spatial_and_partly_authored_rails() {
    for (input, faces, hinges, expected_area) in [
        (
            &[
                "Polyline 0,0,0 4,0,0 4,3,0 4,3,5 10,3,5",
                "Line 0,0,0 0,1.5,1",
                "Line 10,3,5 10,6,7",
            ][..],
            4,
            &[
                point(2.436899862825789, 1.563100137174211, 0.49519890260631),
                point(2.33196159122085, 3.344307270233196, -0.344307270233196),
                point(2.12962962962963, 4.962962962962963, 6.87037037037037),
            ][..],
            46.2940862,
        ),
        (
            &[
                "Polyline 0,0,0 4,0,0 4,3,0 10,3,0",
                "Line 0,0,0 0,1.5,1",
                "Line 4,3,0 3,4,1.5",
                "Line 10,3,0 10,6,2",
            ][..],
            3,
            &[point(3.2320057, 1.6255917, 1.3032070), point(3., 4., 1.5)][..],
            29.0098133,
        ),
        (
            &[
                "Polyline 0,0,0 5,0,0 5,5,0",
                "Line 0,0,0 0,1.5,1",
                "Line 2,0,0 2,1.25,1.5",
                "Line 5,2,0 3,2,1.5",
                "Line 5,5,0 2,5,2",
            ][..],
            2,
            &[point(3.264, 1.736, 1.5)][..],
            20.2827423,
        ),
    ] {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for command in input {
            registry.execute(&mut document, command).unwrap();
        }
        let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
        document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "Sweep1 UntrimmedMiters=Yes")
            .unwrap();
        let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
            panic!("expected a Sweep1 BRep");
        };
        assert_eq!(brep.faces().len(), faces);
        assert_eq!(
            brep.edge_use_counts()
                .iter()
                .filter(|&&count| count == 2)
                .count(),
            hinges.len()
        );
        for (face, expected) in brep.faces().iter().zip(hinges) {
            let surface = face.surface();
            let actual = surface
                .evaluate(*surface.domain_u().end(), *surface.domain_v().end())
                .unwrap();
            assert!(
                actual.distance_to(*expected).unwrap() < 2e-6,
                "{actual:?} != {expected:?}"
            );
        }
        let area = brep.area(document.tolerance()).unwrap();
        assert!(
            (area - expected_area).abs() < 2e-4,
            "{area} != {expected_area} for {input:?}"
        );
    }
}

#[test]
fn sweep_miters_trim_matching_profiles_across_an_unauthored_corner() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Polyline 0,0,0 5,0,0 5,5,0",
        "Line 0,0,0 0,1.5,1",
        "Line 5,5,0 3.5,5,1",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Sweep1 UntrimmedMiters=No")
        .unwrap();
    let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
        panic!("expected a Sweep1 BRep");
    };
    assert_eq!((brep.faces().len(), brep.edges().len()), (2, 7));
    assert!((brep.area(document.tolerance()).unwrap() - 15.3235929).abs() < 1e-7);
}

#[test]
fn sweep_miters_infer_a_rational_miter_between_circular_profiles() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Polyline 0,0,0 5,0,0 5,5,0")
        .unwrap();
    let rail = document.objects().last().unwrap().id();
    let first = add_circle(&mut document, point(0., 0., 0.), 1., 0., 1.);
    let last = add_circle(&mut document, point(5., 5., 0.), 0., 1., 1.5);
    document
        .select_objects_direct([rail, first, last], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Sweep1 UntrimmedMiters=Yes")
        .unwrap();
    let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
        panic!("expected a Sweep1 BRep");
    };
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (2, 5, 3)
    );
    let surface = brep.faces()[0].surface();
    let miter = surface.isocurve_v(*surface.domain_u().end()).unwrap();
    assert_eq!(miter.degree(), 2);
    assert!(miter.is_closed().unwrap());
    for (v, expected) in [
        (0., point(3.75, 1.25, 0.)),
        (0.25, point(5., 0., 1.25)),
        (0.50, point(6.25, -1.25, 0.)),
    ] {
        assert!(miter.evaluate(v).unwrap().distance_to(expected).unwrap() < 1e-8);
    }
    assert!((brep.area(document.tolerance()).unwrap() - 78.6891927).abs() < 3e-4);
}

#[test]
fn sweep_miters_trim_a_varying_width_and_keep_extended_components_when_the_cut_is_partial() {
    for (tip, areas, topology, seam_tip) in [
        (
            "2,5,1",
            &[19.39848929][..],
            (2, 7, 6),
            Some(point(2.37938393, 1.71435433, 1.)),
        ),
        ("8,5,1", &[19.82641659][..], (4, 11, 8), None),
        ("3.5,5,2", &[16.93826831, 15.45398343][..], (2, 7, 6), None),
    ] {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for command in [
            "Polyline 0,0,0 5,0,0 5,5,0".to_owned(),
            "Line 0,0,0 0,1.5,1".to_owned(),
            format!("Line 5,5,0 {tip}"),
        ] {
            registry.execute(&mut document, &command).unwrap();
        }
        let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
        document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        document.clear_history().unwrap();
        let sources = document.objects().cloned().collect::<Vec<_>>();
        let message = registry
            .execute(&mut document, "Sweep1 UntrimmedMiters=No")
            .unwrap();
        let parts = document.objects().skip(sources.len()).collect::<Vec<_>>();
        assert_eq!(parts.len(), areas.len(), "{tip} {message}");
        for (object, &expected) in parts.iter().zip(areas) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("expected a Sweep1 BRep");
            };
            assert_eq!(
                (
                    brep.faces().len(),
                    brep.edges().len(),
                    brep.vertices().len()
                ),
                topology,
                "{tip}"
            );
            if let Some(seam_tip) = seam_tip {
                assert!(
                    brep.edges()
                        .iter()
                        .zip(brep.edge_use_counts())
                        .any(|(edge, uses)| {
                            if uses != 2 {
                                return false;
                            }
                            let curve = edge.curve();
                            let domain = curve.domain();
                            [*domain.start(), *domain.end()].into_iter().any(|t| {
                                curve.evaluate(t).unwrap().distance_to(seam_tip).unwrap() < 3e-5
                            })
                        })
                );
            }
            assert_eq!(brep.edge_connected_face_components().len(), 1);
            let actual = brep.area(document.tolerance()).unwrap();
            assert!(
                (actual - expected).abs() < 2e-4,
                "{tip} {actual} != {expected}"
            );
        }
        assert_eq!(document.undo_label(), Some("Sweep1"));
        document.undo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), sources);
    }
}

#[test]
fn sweep_miters_trim_an_inferred_corner_and_sew_the_polygonal_seam() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Polyline 0,0,0 5,0,0 5,5,0 0,5,0 0,0,0",
        "Line 0,0,0 0,1,1",
        "Line 5,0,0 4,1,1",
        "Line 0,5,0 1,4,1",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let message = registry
        .execute(&mut document, "Sweep1 Closed=Yes UntrimmedMiters=No")
        .unwrap();
    let outputs = document
        .objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Brep(brep) => Some(brep),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 1, "{message}");
    let brep = outputs[0];
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (4, 12, 8)
    );
    assert!((brep.area(document.tolerance()).unwrap() - 22.6274191).abs() < 2e-4);
}
