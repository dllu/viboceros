use super::*;

fn face(document: &mut Document, [x, y, xx, yy]: [f64; 4], [a, b, c, d]: [f64; 4]) -> ObjectId {
    document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([p(x, y, a), p(xx, y, b), p(xx, yy, d), p(x, yy, c)])
                .unwrap(),
        ))
        .unwrap()
}

#[test]
fn surface_taper_boundary_matches_rhino_flat_reversed_oblique_and_warped_cuts() {
    for (bounds, cutter_z, top, volume) in [
        (
            [-20., -20., 20., 20.],
            [5.; 4],
            [6.528980942125395, 4.528980942125397, 5.],
            88.98871161228938,
        ),
        (
            [-20., -20., 20., 20.],
            [-1.; 4],
            [6.5289809421253955, 4.528980942125395, 2.],
            88.98871161228936,
        ),
        (
            [-5., -4., 12., 8.],
            [3., 5., 4., 6.],
            [6.541247014079039, 4.541247014079039, 5.069564350868827],
            69.96617325477561,
        ),
        (
            [-5., -4., 12., 8.],
            [3.4, 5.2, 4.8, 5.5],
            [6.544376045017059, 4.54437604501706, 5.087309967140641],
            78.19985580375516,
        ),
    ] {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
        let mut doc = Document::new(tolerance);
        let source = face(&mut doc, [0., 0., 6., 4.], [2.; 4]);
        let cutter = face(&mut doc, bounds, cutter_z);
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut doc,
                &format!("ExtrudeSrfTapered ToBoundary={cutter}:0 DraftAngle=-10 Solid=No"),
            )
            .unwrap_or_else(|error| panic!("cutter: {cutter_z:?}: {error}"));
        let Geometry::Brep(output) = doc
            .objects()
            .find(|object| ![source, cutter].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected clipped taper");
        };
        assert!(output.is_solid());
        assert_eq!(output.faces().len(), 6, "{cutter_z:?}");
        let actual_volume = output.signed_volume(tolerance).unwrap().abs();
        let actual = viboceros_smlib::Shape::from_brep(output, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        // Convert model-space tolerance to a volume tolerance for the captured body.
        let width = actual.min().distance_to(actual.max()).unwrap();
        assert!(
            (actual_volume - volume).abs() < width * width * tolerance.absolute(),
            "{cutter_z:?}: volume {actual_volume} != {volume}"
        );
        if cutter_z == [3., 5., 4., 6.] {
            // Exact polyhedral volume from the four draft rulings and the oblique plane.
            assert!((actual_volume - 69.96621080684903).abs() < 1e-7);
        }
        assert!(
            actual.max().distance_to(p(top[0], top[1], top[2])).unwrap() < 1e-5,
            "{actual:?}"
        );
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([source, cutter])
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), 2);
        assert_eq!(doc.selected_object_count(), 0);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().count(), 3);
        assert_eq!(doc.selected_object_count(), 0);
    }
}

#[test]
fn surface_taper_boundary_miss_retains_inputs_and_redo() {
    for (bounds, angle, height) in [
        ([-0.5, -0.5, 6.5, 4.5], -10., 5.),
        ([-0.25, -0.25, 6.25, 4.25], -10., 2.2),
        ([30., 30., 32., 32.], -10., 5.),
        ([-20., -20., 20., 20.], 5., 5.),
    ] {
        let mut doc = Document::default();
        let source = face(&mut doc, [0., 0., 6., 4.], [2.; 4]);
        let cutter = face(&mut doc, bounds, [height; 4]);
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        doc.add_geometry(Geometry::Point(p(50., 50., 50.))).unwrap();
        doc.undo().unwrap();
        let redo = doc.redo_label().unwrap().to_owned();
        let registry = CommandRegistry::with_builtins();
        let result = registry.execute(
            &mut doc,
            &format!("ExtrudeSrfTapered ToBoundary={cutter} DraftAngle={angle} DeleteInput=Yes"),
        );
        assert!(
            matches!(result, Err(CommandError::SurfaceExtrusionBoundaryRejected)),
            "{result:?}"
        );
        assert_eq!(doc.objects().count(), 2);
        assert!(doc.object(source).is_some());
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([source, cutter])
        );
        assert!(!doc.can_undo());
        assert_eq!(doc.redo_label(), Some(redo.as_str()));
    }
}

#[test]
fn surface_taper_boundary_uses_temporary_stock_for_inward_and_coincident_cuts() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (size, height, draft, expected_faces, max_z, volume) in [
        (
            [-0.25, -0.25, 6.25, 4.25],
            2.2,
            0.,
            6,
            2.2,
            4.765086180096268,
        ),
        (
            [-0.25, -0.25, 6.25, 4.25],
            2.2,
            0.1,
            6,
            2.2,
            4.799301900082926,
        ),
        (
            [-0.25, -0.25, 6.25, 4.25],
            2.2,
            10.,
            6,
            2.2,
            4.7298008472273025,
        ),
        (
            [-9., -9., 15., 13.],
            2.,
            -10.,
            6,
            34.55764119219941,
            4081.1049097886953,
        ),
    ] {
        let mut doc = Document::new(tolerance);
        let source = face(&mut doc, [0., 0., 6., 4.], [2.; 4]);
        let cutter = face(&mut doc, size, [height; 4]);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfTapered FaceRefs={source}:0 ToBoundary={cutter} DraftAngle={draft}"
                ),
            )
            .unwrap_or_else(|error| panic!("z={height}, draft={draft}: {error}"));
        let Geometry::Brep(brep) = doc
            .objects()
            .find(|object| ![source, cutter].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected tapered stock or cut");
        };
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), expected_faces);
        let actual_volume = brep.signed_volume(tolerance).unwrap().abs();
        assert!(
            (actual_volume - volume).abs() < 1e-5,
            "z={height}, draft={draft}: {actual_volume} != {volume}"
        );
        let top = brep
            .vertices()
            .iter()
            .map(|vertex| vertex.point().z())
            .reduce(f64::max)
            .unwrap();
        assert!((top - max_z).abs() <= tolerance.absolute(), "{top}");
    }
}

#[test]
fn surface_taper_boundary_uses_the_set_axis_and_stages_a_batch_atomically() {
    let mut doc = Document::default();
    let first = face(&mut doc, [0., 0., 6., 4.], [2.; 4]);
    let second = face(&mut doc, [40., 0., 46., 4.], [2.; 4]);
    let cutter = face(&mut doc, [-9., -9., 15., 13.], [5.; 4]);
    doc.select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    let result = registry.execute(
        &mut doc,
        &format!(
            "ExtrudeSrfTapered ToBoundary={cutter} Direction=0,0,-1 DraftAngle=10 DeleteInput=Yes"
        ),
    );
    assert!(
        matches!(result, Err(CommandError::SurfaceExtrusionBoundaryRejected)),
        "{result:?}"
    );
    assert!(
        doc.object(first).is_some() && doc.object(second).is_some() && doc.object(cutter).is_some()
    );
    assert_eq!(doc.objects().count(), 3);
    assert!(!doc.can_undo());

    doc.select_objects_direct([first], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, &format!("ExtrudeSrfTapered ToBoundary={cutter} Direction=0,0,-1 DraftAngle=10 DeleteInput=Yes")).unwrap();
    assert!(doc.object(first).is_none());
    assert!(doc.object(second).is_some() && doc.object(cutter).is_some());
    let Geometry::Brep(result) = doc
        .objects()
        .find(|object| ![second, cutter].contains(&object.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected outward taper")
    };
    assert!(
        (result.signed_volume(doc.tolerance()).unwrap().abs() - 88.98871161228945).abs() < 1e-5
    );
}

fn holed_cutter(doc: &mut Document, [x, end]: [f64; 2]) -> ObjectId {
    let tol = doc.tolerance();
    let rectangle = |[x, y, xx, yy]: [f64; 4]| {
        Polyline3::try_new(
            vec![
                p(x, y, 5.),
                p(xx, y, 5.),
                p(xx, yy, 5.),
                p(x, yy, 5.),
                p(x, y, 5.),
            ],
            tol,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let brep = Brep::try_planar_face_with_holes(
        &rectangle([-9., -9., 15., 13.]),
        &[rectangle([x, 1.5, end, 2.])],
        tol,
    )
    .unwrap();
    doc.add_geometry(Geometry::Brep(brep)).unwrap()
}

#[test]
fn surface_taper_boundary_copies_an_enclosed_cutter_opening_and_replays_a_rim_miss() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (hole, clipped) in [([2., 2.5], true), ([-0.7, 0.5], false)] {
        let mut doc = Document::new(tolerance);
        let source = face(&mut doc, [0., 0., 6., 4.], [2.; 4]);
        let cutter = holed_cutter(&mut doc, hole);
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let registry = CommandRegistry::with_builtins();
        let result = registry.execute(
            &mut doc,
            &format!("ExtrudeSrfTapered ToBoundary={cutter} DraftAngle=-10"),
        );
        if clipped {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(
                matches!(result, Err(CommandError::SurfaceExtrusionBoundaryRejected)),
                "{result:?}"
            );
        }
        let Geometry::Brep(brep) = doc
            .objects()
            .find(|object| ![source, cutter].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected cut or untrimmed taper")
        };
        assert_eq!(brep.is_solid(), !clipped);
        assert_eq!(brep.faces().len(), 6);
        let top = brep
            .vertices()
            .iter()
            .map(|vertex| vertex.point().z())
            .reduce(f64::max)
            .unwrap();
        assert!(
            (top - if clipped { 5. } else { 34.69556544854363 }).abs() <= tolerance.absolute(),
            "{top}"
        );
        if !clipped {
            assert!(
                (brep.signed_volume(tolerance).unwrap().abs() - 4118.543754304289).abs() < 1e-5
            );
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().count(), 2);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().count(), 3);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        }
    }
}

#[test]
fn surface_taper_boundary_keeps_curved_and_trimmed_source_faces() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let circle = |radius| {
        Circle3::try_new(
            p(0., 0., 0.),
            radius,
            UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let ring = Geometry::Brep(
        Brep::try_planar_face_with_holes(&circle(3.), &[circle(1.)], tolerance).unwrap(),
    );
    for (geometry, draft, faces, volume) in [
        (patch(true), -10., 14, 234.0799615668335),
        (ring.clone(), -1., 4, 131.14737742302438),
        (ring.clone(), -2., 4, 149.7593095584865),
        (ring, -10., 4, 186.98756507888106),
    ] {
        let mut doc = Document::new(tolerance);
        let source = doc.add_geometry(geometry.clone()).unwrap();
        let cutter = face(&mut doc, [-9., -9., 15., 13.], [5.; 4]);
        let commands = CommandRegistry::with_builtins();
        commands
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfTapered FaceRefs={source}:0 ToBoundary={cutter} DraftAngle={draft}"
                ),
            )
            .unwrap_or_else(|error| panic!("draft={draft}, faces={faces}: {error}"));
        let Geometry::Brep(output) = doc
            .objects()
            .find(|object| ![source, cutter].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected curved or trimmed tapered cut");
        };
        assert!(output.is_solid());
        assert_eq!(output.faces().len(), faces);
        let actual = output.signed_volume(tolerance).unwrap().abs();
        assert!(
            (actual - volume).abs() < 1e-4,
            "draft={draft}, faces={faces}: {actual} != {volume}"
        );
        assert_eq!(doc.object(source).unwrap().geometry(), &geometry);
    }
}

#[test]
fn curve_taper_boundary_matches_rhino_caps_and_open_line() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (open, solid, z, faces, low, high) in [
        (
            false,
            true,
            [5.; 4],
            6,
            [-0.528980942125395, -0.528980942125395, 2.],
            [6.528980942125395, 4.528980942125395, 5.],
        ),
        (
            false,
            false,
            [-1.; 4],
            4,
            [-0.528980942125395, -0.528980942125395, -1.],
            [6.528980942125395, 4.528980942125395, 2.],
        ),
        (
            false,
            false,
            [3., 5., 4., 6.],
            4,
            [-0.3952088687249683, -0.4661106709348568, 2.],
            [6.541247014079039, 4.54124701407904, 5.069564350868827],
        ),
        (
            true,
            true,
            [5.; 4],
            1,
            [0., -0.528980942125395, 2.],
            [6., 0., 5.],
        ),
    ] {
        let mut doc = Document::new(tolerance);
        let mut points = vec![p(0., 0., 2.), p(6., 0., 2.)];
        if !open {
            points.extend([p(6., 4., 2.), p(0., 4., 2.), p(0., 0., 2.)]);
        }
        let profile = Geometry::Polyline(Polyline3::try_new(points, tolerance).unwrap());
        let source = doc.add_geometry(profile).unwrap();
        let cutter = face(&mut doc, [-5., -4., 12., 8.], z);
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        let solid_option = if solid { "Yes" } else { "No" };
        CommandRegistry::with_builtins()
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeCrvTapered ToBoundary={cutter} DraftAngle=-10 Solid={solid_option}"
                ),
            )
            .unwrap_or_else(|error| panic!("open={open}, solid={solid}, z={z:?}: {error}"));
        let Geometry::Brep(output) = doc
            .objects()
            .find(|object| ![source, cutter].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected a tapered cut");
        };
        assert_eq!(output.is_solid(), !open && solid);
        assert_eq!(output.faces().len(), faces);
        let actual = viboceros_smlib::Shape::from_brep(output, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        assert!(
            actual
                .min()
                .distance_to(Point3::try_from(low).unwrap())
                .unwrap()
                < 1e-5,
            "{actual:?}"
        );
        assert!(
            actual
                .max()
                .distance_to(Point3::try_from(high).unwrap())
                .unwrap()
                < 1e-5,
            "{actual:?}"
        );
    }
}
