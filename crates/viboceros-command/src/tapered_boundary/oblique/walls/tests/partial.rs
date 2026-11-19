use super::*;

const STOCK_FIRST: [[f64; 3]; 2] = [
    [0., -12.889027486408285, 2.],
    [18.017123287671154, 0., 74.10273972602694],
];
const STOCK_LAST: [[f64; 3]; 2] = [[-0.6965351090274599, 0., 2.], [0., 4., 76.22168279214344]];
const CUT_FIRST: [[f64; 3]; 2] = [[0., -0.5362775756670339, 2.], [6.5, 0., 5.]];

const FIRST_LEFT: [f64; 3] = [0.5, -0.5362775756670339, 5.];
const FIRST_RIGHT: [f64; 3] = [6.5, -0.5362775756670339, 5.];
const LAST_END: [f64; 3] = [-0.02815356979892647, 0., 5.];
const FIRST_SECTION: [[f64; 3]; 2] = [FIRST_LEFT, FIRST_RIGHT];

#[test]
fn oblique_boundary_selects_and_imprints_walls_for_partial_cutter_coverage() {
    let front_only = [[0.25, -1.], [6.75, -0.2]];
    let front_and_side = [[-1., -1.], [6.75, 4.2]];
    let partial_last = [[-1., -1.], [8., 1.]];
    let partial_first_left = [[0.25, -1.], [6., 5.]];
    for (profile, footprint, flip, expected) in [
        (
            &RECTANGLE[..],
            front_only,
            false,
            Some((STOCK_LAST, &[][..])),
        ),
        (
            &RECTANGLE[..],
            front_and_side,
            false,
            Some((CUT_FIRST, &FIRST_SECTION[..])),
        ),
        (
            &RECTANGLE[..],
            partial_last,
            false,
            Some((STOCK_LAST, &[LAST_END][..])),
        ),
        (
            &RECTANGLE[..],
            partial_first_left,
            false,
            Some((STOCK_FIRST, &[FIRST_LEFT][..])),
        ),
        (
            &RECTANGLE[..],
            [[2., -1.], [8., 5.]],
            false,
            Some((STOCK_FIRST, &[FIRST_RIGHT][..])),
        ),
        (
            &RECTANGLE[..],
            front_only,
            true,
            Some((CUT_FIRST, &FIRST_SECTION[..])),
        ),
        (
            &RECTANGLE[..4],
            front_only,
            false,
            Some((CUT_FIRST, &FIRST_SECTION[..])),
        ),
        (
            &RECTANGLE[..4],
            [[0.25, -1.], [3., -0.2]],
            false,
            Some((STOCK_FIRST, &[FIRST_LEFT][..])),
        ),
        (&RECTANGLE[..2], [[0.25, -1.], [3., -0.2]], false, None),
    ] {
        let face = cutter(footprint, [5.; 4]);
        let face = if flip {
            face.try_reversed_u().unwrap()
        } else {
            face
        };
        let (mut doc, registry, source, boundary) = scene(profile, face);
        let executed = run(
            &mut doc,
            &registry,
            boundary,
            &[
                "Direction=1,0,6",
                "DraftAngle=-10",
                "Solid=No",
                "DeleteInput=Yes",
            ],
            Some(12.017123287671236 * 37_f64.sqrt()),
        );
        let Some((bounds, hits)) = expected else {
            assert!(matches!(
                executed,
                Err(CommandError::SurfaceExtrusionBoundaryRejected)
            ));
            assert!(doc.object(source).is_some());
            assert!(!doc.can_undo());
            continue;
        };
        assert_eq!(
            executed.unwrap_or_else(|error| panic!("{profile:?} {footprint:?} {flip}: {error}")),
            TaperedBoundaryOutcome::Cut(1),
            "{profile:?} {footprint:?} {flip}"
        );
        let wall = result(&doc, source, boundary);
        assert_bounds(wall, bounds, doc.tolerance());
        let cut = bounds == CUT_FIRST;
        let corners = 4 + if cut { 0 } else { hits.len() };
        assert_eq!(
            (
                wall.faces().len(),
                wall.edges().len(),
                wall.vertices().len()
            ),
            (1, corners, corners)
        );
        for &hit in hits {
            let hit = Point3::try_from(hit).unwrap();
            assert!(
                wall.vertices()
                    .iter()
                    .any(|v| v.point().is_near(hit, doc.tolerance()))
            );
        }
        assert!(doc.object(source).is_none());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn oblique_boundary_keeps_both_edge_imprints_when_a_cutter_hole_interrupts_the_trace() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let rectangle = |[[x, y], [xx, yy]]: [[f64; 2]; 2]| {
        polyline(&[[x, y], [xx, y], [xx, yy], [x, yy], [x, y]], 5., tolerance)
            .to_nurbs()
            .unwrap()
    };
    let outer = rectangle(WIDE);
    for (hole, bounds, hits) in [
        ([[2., -0.8], [3., -0.3]], STOCK_FIRST, FIRST_SECTION),
        (
            [[-0.25, 1.], [0.25, 2.]],
            STOCK_LAST,
            [LAST_END, [LAST_END[0], 4., 5.]],
        ),
    ] {
        let cutter =
            Brep::try_planar_face_with_holes(&outer, &[rectangle(hole)], tolerance).unwrap();
        let (mut doc, registry, source, boundary) = scene_with_geometry(
            Geometry::Polyline(polyline(&RECTANGLE, 2., tolerance)),
            Geometry::Brep(cutter),
        );
        let outcome = run(
            &mut doc,
            &registry,
            boundary,
            &["Direction=1,0,6", "DraftAngle=-10"],
            Some(12.017123287671236 * 37_f64.sqrt()),
        )
        .unwrap();
        assert_eq!(outcome, TaperedBoundaryOutcome::Cut(1));
        let wall = result(&doc, source, boundary);
        assert_bounds(wall, bounds, tolerance);
        assert_eq!(
            (
                wall.faces().len(),
                wall.edges().len(),
                wall.vertices().len()
            ),
            (1, 6, 6)
        );
        for hit in hits {
            let hit = Point3::try_from(hit).unwrap();
            assert!(
                wall.vertices()
                    .iter()
                    .any(|v| v.point().is_near(hit, tolerance))
            );
        }
    }
}
