use super::*;

fn face(corners: [Point3; 4]) -> Geometry {
    let [a, b, c, d] = corners;
    Geometry::NurbsSurface(NurbsSurface::try_bilinear([a, b, d, c]).unwrap())
}

fn scene(curved: bool) -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let source = doc
        .add_geometry(face([
            p(0., 0., 2.),
            p(2., 0., 2.),
            p(0., 3., 2.),
            p(2., 3., 2.),
        ]))
        .unwrap();
    let points = if curved {
        vec![p(8., -3., 0.), p(8.4, -2., 3.8), p(12., -1., 5.)]
    } else {
        vec![p(8., -3., 0.), p(12., -1., 5.)]
    };
    let n = points.len();
    let path = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(n - 1, points, [vec![0.; n], vec![1.; n]].concat()).unwrap(),
        ))
        .unwrap();
    (doc, source, path)
}

fn cutter(doc: &mut Document, [x, y, xx, yy]: [f64; 4], [a, b, c, d]: [f64; 4]) -> ObjectId {
    doc.add_geometry(face([p(x, y, a), p(xx, y, b), p(x, yy, c), p(xx, yy, d)]))
        .unwrap()
}

#[test]
fn surface_path_boundary_replays_flat_oblique_and_warped_rhino_cuts() {
    for (curved, bounds, heights, expected, volume) in [
        (false, [-10., -10., 15., 10.], [6.; 4], [5.2, 4.6, 6.], 24.),
        (
            false,
            [-10., -10., 15., 10.],
            [2.1, 5.6, 4.1, 7.6],
            [4.90566037735849, 4.452830188679246, 5.632075471698114],
            19.74056603773584,
        ),
        (
            true,
            [-10., -10., 15., 10.],
            [6.; 4],
            [4.0675527693020275, 4.376942212135248, 6.],
            24.,
        ),
        (
            true,
            [-10., -10., 15., 10.],
            [2.1, 5.6, 4.1, 7.6],
            [3.4010133006201926, 4.096761532631238, 5.385818015349949],
            18.25604047565413,
        ),
        (
            true,
            [-2., -2., 10., 7.],
            [4.2, 5.4, 4.8, 5.],
            [2.9559224212763207, 3.871339835462649, 4.817589853996418],
            16.331237480318908,
        ),
    ] {
        let (mut doc, source, path) = scene(curved);
        let target = cutter(&mut doc, bounds, heights);
        let source_before = doc.object(source).unwrap().clone();
        doc.clear_history().unwrap();
        let commands = CommandRegistry::with_builtins();
        commands.execute(&mut doc, &format!(
            "ExtrudeSrfAlongCrv FaceRefs={source}:0 Path={path} ToBoundary={target} Solid=No DeleteInput=Yes"
        )).unwrap_or_else(|error| panic!("{heights:?} curved={curved}: {error}"));
        let Geometry::Brep(result) = doc
            .objects()
            .find(|object| ![source, path, target].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected boundary extrusion");
        };
        assert!(result.is_solid());
        assert_eq!(result.faces().len(), 6);
        let tolerance = doc.tolerance();
        let native = viboceros_smlib::Shape::from_brep(result, tolerance).unwrap();
        let end = native.tight_bounds().unwrap().max();
        assert!((result.signed_volume(tolerance).unwrap().abs() - volume).abs() < 1e-4);
        assert!(
            end.distance_to(Point3::try_new(expected[0], expected[1], expected[2]).unwrap())
                .unwrap()
                < 1e-5,
            "{end:?}"
        );
        assert!(doc.object(source).is_none());
        assert!(doc.object(path).is_some() && doc.object(target).is_some());
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([target, path])
        );
        assert_eq!(doc.undo_label(), Some("ExtrudeSrfAlongCrv"));
        commands.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(source), Some(&source_before));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn surface_path_boundary_uses_the_prepared_interval_and_reversed_rail() {
    for (curved, z, argument, volume) in [
        (false, 5., "PathRange=0.2,0.9", 18.),
        (true, -1., "Reverse=Yes", 18.),
        (true, 2., "", 30.),
        (true, 7., "", 30.),
    ] {
        let (mut doc, source, path) = scene(curved);
        let target = cutter(&mut doc, [-10., -10., 15., 10.], [z; 4]);
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        let commands = CommandRegistry::with_builtins();
        commands
            .execute(
                &mut doc,
                &format!("ExtrudeSrfAlongCrv Path={path} ToBoundary={target} {argument}"),
            )
            .unwrap_or_else(|e| panic!("curved={curved}, z={z}, {argument}: {e}"));
        let Geometry::Brep(result) = doc
            .objects()
            .find(|object| ![source, path, target].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected boundary extrusion");
        };
        assert!(result.is_solid());
        assert_eq!(result.faces().len(), 6);
        assert!((result.signed_volume(doc.tolerance()).unwrap().abs() - volume).abs() < 1e-5);
        commands.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        commands.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
    }
}

#[test]
fn surface_path_boundary_rejects_short_unreached_or_enclosed_cutters_atomically() {
    let wide = [-10., -10., 15., 10.];
    for (bounds, z, argument) in [
        (wide, 8., ""),
        (wide, 6., "PathRange=0.,0.5"),
        (wide, 6., "Reverse=Yes"),
        ([2.1, 1.1, 2.4, 1.4], 4., ""),
        ([0.2, 0.2, 0.8, 0.8], 2., ""),
        ([4.2, 2.2, 4.8, 2.8], 7., ""),
    ] {
        let (mut doc, source, path) = scene(false);
        let target = cutter(&mut doc, bounds, [z; 4]);
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        doc.add_geometry(Geometry::Point(p(-20., 0., 0.))).unwrap();
        doc.undo().unwrap();
        let redo = doc.redo_label().unwrap().to_owned();
        let commands = CommandRegistry::with_builtins();
        let output = commands.execute(
            &mut doc,
            &format!(
                "ExtrudeSrfAlongCrv Path={path} ToBoundary={target} DeleteInput=Yes {argument}"
            ),
        );
        assert!(
            matches!(output, Err(CommandError::SurfaceExtrusionBoundaryRejected)),
            "{z}, {bounds:?}, {argument}: {output:?}"
        );
        assert_eq!(doc.objects().count(), 3);
        assert!(doc.object(source).is_some());
        assert!(!doc.can_undo());
        assert_eq!(doc.redo_label(), Some(redo.as_str()));
        assert_eq!(doc.selected_object_count(), 3);
    }
}

#[test]
fn surface_path_boundary_batch_deletes_only_sources_that_reach_the_cutter() {
    let (mut doc, source, path) = scene(false);
    let missed = cutter(&mut doc, [40., 0., 42., 3.], [2.; 4]);
    let target = cutter(&mut doc, [-10., -10., 15., 10.], [6.; 4]);
    doc.select_objects_direct([source, missed], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let commands = CommandRegistry::with_builtins();
    commands
        .execute(
            &mut doc,
            &format!("ExtrudeSrfAlongCrv Path={path} ToBoundary={target} DeleteInput=Yes"),
        )
        .unwrap();
    assert!(doc.object(source).is_none());
    assert!(doc.object(missed).is_some());
    assert_eq!(doc.objects().count(), 4);
    assert_eq!(doc.selected_object_count(), 3);
    commands.execute(&mut doc, "Undo").unwrap();
    assert_eq!(
        doc.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([source, missed])
    );
    commands.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [missed]);
}

#[test]
fn surface_path_boundary_imprints_a_partial_coincident_cutter_on_the_start_rim() {
    let (mut doc, source, path) = scene(true);
    let target = cutter(&mut doc, [-1., -1., 0.5, 0.5], [2.; 4]);
    let commands = CommandRegistry::with_builtins();
    commands
        .execute(
            &mut doc,
            &format!("ExtrudeSrfAlongCrv FaceRefs={source}:0 Path={path} ToBoundary={target}"),
        )
        .unwrap();
    let Geometry::Brep(result) = doc
        .objects()
        .find(|object| ![source, path, target].contains(&object.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected boundary extrusion");
    };
    assert!(result.is_solid());
    assert_eq!(result.faces().len(), 7);
    assert!((result.signed_volume(doc.tolerance()).unwrap().abs() - 30.).abs() < 1e-5);
}

#[test]
fn surface_path_boundary_preserves_cutter_holes_and_coincident_imprints() {
    for (curved, z, hole, solid, faces, edges, top) in [
        (false, 4., [2.1, 1.6, 2.6, 2.1], false, 6, 16, 4.),
        (true, 4., [1.5, 1.2, 1.75, 1.5], false, 6, 16, 4.),
        (true, 4., [8., 1.2, 9., 1.5], true, 6, 12, 4.),
        (false, 2., [0.5, 1., 1.5, 2.], true, 7, 16, 7.),
    ] {
        let (mut doc, source, path) = scene(curved);
        let tolerance = doc.tolerance();
        let target = cutter_with_hole(&mut doc, hole, z);
        let commands = CommandRegistry::with_builtins();
        commands
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfAlongCrv FaceRefs={source}:0 Path={path} ToBoundary={target}:0"
                ),
            )
            .unwrap_or_else(|error| panic!("{hole:?}, z={z}, curved={curved}: {error}"));
        let Geometry::Brep(result) = doc
            .objects()
            .find(|object| ![source, path, target].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected boundary extrusion");
        };
        assert_eq!(result.is_solid(), solid, "{hole:?}");
        assert_eq!(result.faces().len(), faces, "{hole:?}");
        assert_eq!(result.edges().len(), edges, "{hole:?}");
        let max = result
            .vertices()
            .iter()
            .map(|v| v.point().z())
            .reduce(f64::max)
            .unwrap();
        assert!((max - top).abs() <= tolerance.absolute(), "{hole:?}: {max}");
    }
}

fn cutter_with_hole(doc: &mut Document, hole: [f64; 4], z: f64) -> ObjectId {
    let tolerance = doc.tolerance();
    let rectangle = |[x, y, xx, yy]: [f64; 4]| {
        Polyline3::try_new(
            vec![
                p(x, y, z),
                p(xx, y, z),
                p(xx, yy, z),
                p(x, yy, z),
                p(x, y, z),
            ],
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let boundary = Brep::try_planar_face_with_holes(
        &rectangle([-10., -10., 15., 10.]),
        &[rectangle(hole)],
        tolerance,
    )
    .unwrap();
    doc.add_geometry(Geometry::Brep(boundary)).unwrap()
}

#[test]
fn surface_path_boundary_hole_across_rim_leaves_the_untrimmed_solid_and_source() {
    let (mut doc, source, path) = scene(false);
    let target = cutter_with_hole(&mut doc, [1.4, 1.6, 2.4, 2.1], 4.);
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let commands = CommandRegistry::with_builtins();
    let result = commands.execute(
        &mut doc,
        &format!("ExtrudeSrfAlongCrv Path={path} ToBoundary={target}:0 DeleteInput=Yes"),
    );
    assert!(
        matches!(result, Err(CommandError::SurfaceExtrusionBoundaryRejected)),
        "{result:?}"
    );
    assert!(doc.object(source).is_some());
    let Geometry::Brep(result) = doc
        .objects()
        .find(|object| ![source, path, target].contains(&object.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected the untrimmed extrusion");
    };
    assert!(result.is_solid());
    assert_eq!(result.faces().len(), 6);
    assert!((result.signed_volume(doc.tolerance()).unwrap() - 30.).abs() < 1e-5);
    assert_eq!(doc.selected_object_count(), 3);
    commands.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().count(), 3);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
    commands.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 4);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
}
