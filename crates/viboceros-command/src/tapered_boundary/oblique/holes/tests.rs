use super::*;
use viboceros_geometry::{Circle3, Polyline3, UnitVector3};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn circle(tolerance: Tolerance) -> NurbsCurve {
    Circle3::try_new(
        point(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

fn cutter_face(hole: [f64; 4], tolerance: Tolerance) -> Brep {
    let rectangle = |[x, y, xx, yy]: [f64; 4]| {
        Polyline3::try_new(
            vec![
                point(x, y, 5.),
                point(xx, y, 5.),
                point(xx, yy, 5.),
                point(x, yy, 5.),
                point(x, y, 5.),
            ],
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let outer = rectangle([-9., -9., 15., 13.]);
    Brep::try_planar_face_with_holes(&outer, &[rectangle(hole)], tolerance)
        .unwrap()
        .try_split_kinky_edges(&[0, 1], tolerance.angular(), tolerance)
        .unwrap()
        .unwrap()
}

const CROSSING: [f64; 4] = [4.45, -0.25, 5.1, 0.25];
const OUTSIDE: [f64; 4] = [7., -0.25, 8., 0.25];

#[test]
fn oblique_boundary_matches_rhinos_planar_cutter_holes() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let circle = circle(tolerance);
    let inside = [0.8, -0.25, 1.3, 0.25];
    let encompassing = [-6., -6., 7., 7.];
    for (surface, solid, hole, expected, volume) in [
        (false, true, inside, Some((7, 16)), Some(188.04637194715377)),
        (false, false, inside, Some((4, 12)), None),
        (false, true, OUTSIDE, Some((6, 12)), Some(188.0463701034683)),
        (true, false, OUTSIDE, Some((6, 12)), Some(188.046367568372)),
        (true, false, inside, None, None),
        (true, false, CROSSING, Some((5, 12)), None),
        (false, true, encompassing, None, None),
        (true, false, encompassing, None, None),
    ] {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let (command, input) = if surface {
            (
                "ExtrudeSrfTapered",
                Geometry::Brep(Brep::try_planar_face(&circle, tolerance).unwrap()),
            )
        } else {
            ("ExtrudeCrvTapered", Geometry::NurbsCurve(circle.clone()))
        };
        let source = doc.add_geometry(input).unwrap();
        let boundary = cutter_face(hole, tolerance);
        let chart = boundary.faces()[0].surface().clone();
        let cutter = doc.add_geometry(Geometry::Brep(boundary)).unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let result = registry.execute(
            &mut doc,
            &format!(
                "{command} ToBoundary={cutter} Direction=1,0,6 DraftAngle=-10 Solid={} DeleteInput=Yes",
                if solid { "Yes" } else { "No" }
            ),
        );
        let failed = (surface && hole != OUTSIDE) || hole == encompassing;
        if failed {
            assert!(matches!(
                result,
                Err(CommandError::SurfaceExtrusionBoundaryRejected)
            ));
        } else {
            result.unwrap();
        }
        assert_eq!(doc.object(source).is_some(), failed);
        assert_eq!(doc.is_selected(source), failed);
        assert!(doc.is_selected(cutter));
        let Some(topology) = expected else {
            assert_eq!(doc.objects().len(), 2);
            assert!(!doc.can_undo());
            continue;
        };
        let Geometry::Brep(brep) = doc
            .objects()
            .find(|obj| ![source, cutter].contains(&obj.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected a tapered extrusion");
        };
        assert_eq!((brep.faces().len(), brep.edges().len()), topology);
        assert_eq!(brep.is_solid(), volume.is_some());
        if let Some(volume) = volume {
            let actual = brep.signed_volume(tolerance).unwrap();
            assert!((actual - volume).abs() < 1e-4, "{actual} vs {volume}");
        }
        if topology == (7, 16) {
            let [ring, patch] = &brep.faces()[5..] else {
                panic!()
            };
            assert_eq!(ring.surface(), &chart);
            assert_eq!(patch.surface(), &chart);
            assert_eq!(ring.is_reversed(), patch.is_reversed());
            let mut ring_edges = ring.loops()[1]
                .trims()
                .iter()
                .map(|trim| trim.edge())
                .collect::<Vec<_>>();
            let mut patch_edges = patch.loops()[0]
                .trims()
                .iter()
                .map(|trim| trim.edge())
                .collect::<Vec<_>>();
            ring_edges.sort_unstable();
            patch_edges.sort_unstable();
            assert_eq!(ring_edges, patch_edges);
        }
        let bounds = Shape::from_brep(brep, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        let reference = if failed {
            point(14.144262518464235, 8.808094048519983, 33.44600781950188)
        } else {
            point(4.76689339995015, 3.89388644429519, 5.)
        };
        assert!(
            bounds.max().distance_to(reference).unwrap() < tolerance.absolute(),
            "{bounds:?} vs {reference:?}"
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 2);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(source).is_some(), failed);
        assert!(!doc.is_selected(cutter));
    }
}

#[test]
fn oblique_boundary_uses_rhinos_curve_preview_stock_for_planar_cuts() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let circle = circle(tolerance);
    for (pointer_x, solid, delete_input, hole, expected) in [
        (
            -9.017123287671236,
            true,
            false,
            CROSSING,
            Some([21.55689603161761, 12.67136203240619, 55.6927018500186]),
        ),
        (
            14.941780821917803,
            true,
            true,
            CROSSING,
            Some([33.74961541433635, 19.02588399065256, 92.28532403024347]),
        ),
        (
            1.078767123287671,
            true,
            false,
            CROSSING,
            Some([5.220061621708912, 4.157037235172032, 6.662818489462937]),
        ),
        (0.777397260273973, true, true, CROSSING, None),
        (0.400684931506849, true, true, OUTSIDE, None),
        (14.941780821917803, false, true, CROSSING, None),
        (
            14.941780821917803,
            true,
            true,
            OUTSIDE,
            Some([4.766893399950153, 3.8938864434609926, 5.]),
        ),
    ] {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let source = doc
            .add_geometry(Geometry::NurbsCurve(circle.clone()))
            .unwrap();
        let cutter = doc
            .add_geometry(Geometry::Brep(cutter_face(hole, tolerance)))
            .unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let options = TaperedExtrusionOptions::parse(&[
            "Direction=1,0,6",
            "DraftAngle=-10",
            if delete_input {
                "DeleteInput=Yes"
            } else {
                "DeleteInput=No"
            },
            if solid { "Solid=Yes" } else { "Solid=No" },
        ])
        .unwrap();
        let distance = pointer_x * 37_f64.sqrt();
        let result = registry.execute_curve_tapered_boundary(
            &mut doc,
            BoundaryRef::parse(&cutter.to_string()).unwrap(),
            options,
            CommandContext::default().construction_plane,
            Some(distance),
        );
        let succeeded = hole == OUTSIDE && expected.is_some();
        match expected {
            Some(_) => assert_eq!(
                result.unwrap(),
                if succeeded {
                    TaperedBoundaryOutcome::Cut(1)
                } else {
                    TaperedBoundaryOutcome::Fallback
                }
            ),
            None => assert!(
                matches!(result, Err(CommandError::SurfaceExtrusionBoundaryRejected)),
                "{pointer_x}: {result:?}",
            ),
        }
        let deleted = delete_input && expected.is_some();
        assert_eq!(doc.object(source).is_none(), deleted);
        assert_eq!(doc.is_selected(source), !deleted);
        assert!(doc.is_selected(cutter));
        let remembered = registry.tapered_extrusion_settings(false);
        assert_eq!(
            remembered.distance,
            if succeeded { distance.abs() } else { 1. }
        );
        let Some(expected) = expected else {
            assert_eq!(doc.objects().len(), 2);
            assert!(!doc.can_undo());
            continue;
        };
        let Geometry::Brep(brep) = doc
            .objects()
            .find(|obj| ![source, cutter].contains(&obj.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected a tapered extrusion")
        };
        assert_eq!(brep.is_solid(), succeeded);
        assert_eq!(
            (brep.faces().len(), brep.edges().len()),
            (if succeeded { 6 } else { 5 }, 12)
        );
        let bounds = Shape::from_brep(brep, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        assert!(
            bounds
                .max()
                .distance_to(Point3::try_from(expected).unwrap())
                .unwrap()
                < tolerance.absolute(),
            "{bounds:?} vs {expected:?}"
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 2);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(source).is_none(), deleted);
        assert!(!doc.is_selected(cutter));
    }
}
