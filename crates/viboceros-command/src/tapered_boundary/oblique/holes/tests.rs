use super::*;
use viboceros_geometry::{Circle3, Polyline3, UnitVector3};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn oblique_boundary_matches_rhinos_planar_cutter_holes() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let circle = Circle3::try_new(
        point(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
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
    let inside = [0.8, -0.25, 1.3, 0.25];
    let outside = [7., -0.25, 8., 0.25];
    let crossing = [4.45, -0.25, 5.1, 0.25];
    let encompassing = [-6., -6., 7., 7.];
    for (surface, solid, hole, expected, volume) in [
        (false, true, inside, Some((7, 16)), Some(188.04637194715377)),
        (false, false, inside, Some((4, 12)), None),
        (false, true, outside, Some((6, 12)), Some(188.0463701034683)),
        (true, false, outside, Some((6, 12)), Some(188.046367568372)),
        (true, false, inside, None, None),
        (true, false, crossing, Some((5, 12)), None),
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
        let boundary =
            Brep::try_planar_face_with_holes(&outer, &[rectangle(hole)], tolerance).unwrap();
        let boundary = boundary
            .try_split_kinky_edges(&[0, 1], tolerance.angular(), tolerance)
            .unwrap()
            .unwrap();
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
        let failed = (surface && hole != outside) || hole == encompassing;
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
