use super::*;

const SINGLE: [[f64; 3]; 2] = [[0., -0.4847756250784715, 2.], [4.257412552744642, 0.5, 5.]];
const CONNECTED: [[f64; 3]; 2] = [[0., -1.0363056171015095, 2.], [8.749120807331991, 0.5, 5.]];
const OPTIONS: &[&str] = &["Direction=1,0,6", "DraftAngle=-10", "DeleteInput=Yes"];

fn profile(spans: usize) -> (Geometry, f64) {
    let controls = [
        [0., 0.],
        [2., 1.],
        [4., 0.],
        [6., -1.],
        [8., 0.],
        [8., 2.],
        [8., 4.],
    ];
    let knots = [0., 0., 0., 1., 1., 2., 2., 3., 3.];
    let mut knots = knots[..3 + 2 * spans].to_vec();
    knots.push(spans as f64);
    let distance = if spans == 1 {
        2.886986301369865 - 2.
    } else {
        4.845890410958901 - 4.
    };
    let curve = NurbsCurve::try_new(
        2,
        controls[..2 * spans + 1]
            .iter()
            .map(|&[x, y]| point(x, y, 2.))
            .collect(),
        knots,
    )
    .unwrap();
    (Geometry::NurbsCurve(curve), distance * 37_f64.sqrt())
}

fn closed_profile() -> (Geometry, f64) {
    let (Geometry::NurbsCurve(curve), distance) = profile(3) else {
        unreachable!()
    };
    let controls = curve
        .control_points()
        .iter()
        .map(|p| p.point())
        .chain([
            point(4., 4., 2.),
            point(0., 4., 2.),
            point(0., 2., 2.),
            point(0., 0., 2.),
        ])
        .collect();
    let knots = vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 5., 5., 5.];
    (
        Geometry::NurbsCurve(NurbsCurve::try_new(2, controls, knots).unwrap()),
        distance,
    )
}

#[test]
fn oblique_boundary_keeps_the_entire_base_of_regular_curved_walls() {
    for (spans, footprint, expected) in [
        (1, WIDE, SINGLE),
        (1, [[0.19, -4.], [5., 3.]], SINGLE),
        (2, WIDE, CONNECTED),
        (3, WIDE, CONNECTED),
        (3, [[0.19, -2.], [9., 0.1]], CONNECTED),
    ] {
        let (curve, distance) = profile(spans);
        let (mut doc, registry, source, boundary) =
            scene_with_geometry(curve, Geometry::NurbsSurface(cutter(footprint, [5.; 4])));
        assert_eq!(
            run(&mut doc, &registry, boundary, OPTIONS, Some(distance)).unwrap(),
            TaperedBoundaryOutcome::Cut(1),
        );
        assert!(doc.object(source).is_none());
        let piece = result(&doc, source, boundary);
        let faces = spans.min(2);
        assert_eq!(
            (
                piece.faces().len(),
                piece.edges().len(),
                piece.vertices().len()
            ),
            (faces, 1 + 3 * faces, 2 + 2 * faces)
        );
        assert_eq!(piece.edge_connected_face_components().len(), 1);
        assert_bounds(piece, expected, doc.tolerance());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn oblique_boundary_rejects_incomplete_intersections_of_connected_curved_walls() {
    let rectangle = |[[x, y], [xx, yy]]: [[f64; 2]; 2], tolerance| {
        polyline(&[[x, y], [xx, y], [xx, yy], [x, yy], [x, y]], 5., tolerance)
            .to_nurbs()
            .unwrap()
    };
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let outer = rectangle([[-3., -4.], [11., 3.]], tolerance);
    let hole = rectangle([[1.5, -2.], [2.8, 2.]], tolerance);
    let interrupted = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
    for (spans, boundary_geometry) in [
        (
            1,
            Geometry::NurbsSurface(cutter([[-3., -4.], [2.4, 3.]], [5.; 4])),
        ),
        (
            2,
            Geometry::NurbsSurface(cutter([[-3., -4.], [6., 3.]], [5.; 4])),
        ),
        (
            2,
            Geometry::NurbsSurface(cutter([[2., -4.], [11., 3.]], [5.; 4])),
        ),
        (2, Geometry::Brep(interrupted)),
    ] {
        let (curve, distance) = profile(spans);
        let (mut doc, registry, source, boundary) = scene_with_geometry(curve, boundary_geometry);
        assert!(matches!(
            run(&mut doc, &registry, boundary, OPTIONS, Some(distance)),
            Err(CommandError::SurfaceExtrusionBoundaryRejected)
        ));
        assert!(doc.object(source).is_some());
        assert_eq!(doc.objects().count(), 2);
        assert!(!doc.can_undo());
    }
}

#[test]
fn oblique_boundary_preserves_shared_rulings_in_partially_cut_curved_strips() {
    let shared = point(4.257412552744642, -0.4720620595239417, 5.);
    let start = point(0.7491208073319924, -0.48477562507847205, 5.);
    let end = point(8.749120807331995, -0.4847756250784715, 5.);
    for (closed, footprint, outer_hit, face_order) in [
        (
            false,
            [[2., -4.], [6., 0.1]],
            None,
            &[[1., 2.], [0., 2.]][..],
        ),
        (
            false,
            [[2., -2.], [9., 0.1]],
            Some(end),
            &[[1., 5.], [0., 2.], [1., 2.]],
        ),
        (
            false,
            [[0.19, -4.], [6., 0.1]],
            Some(start),
            &[[1., 2.], [0., 5.], [0., 2.]],
        ),
        (
            true,
            [[2., -2.], [9., 0.1]],
            Some(end),
            &[[1., 5.], [0., 2.], [1., 2.]],
        ),
        (
            true,
            [[0.19, -4.], [6., 0.1]],
            Some(start),
            &[[1., 2.], [0., 5.], [0., 2.]],
        ),
    ] {
        let (curve, distance) = if closed { closed_profile() } else { profile(3) };
        let (mut doc, registry, source, boundary) =
            scene_with_geometry(curve, Geometry::NurbsSurface(cutter(footprint, [5.; 4])));
        assert_eq!(
            run(&mut doc, &registry, boundary, OPTIONS, Some(distance)).unwrap(),
            TaperedBoundaryOutcome::Cut(1)
        );
        let piece = result(&doc, source, boundary);
        let tolerance = doc.tolerance();
        let divided = usize::from(outer_hit.is_some());
        assert_eq!(piece.faces().len(), 2 + divided);
        assert_eq!(piece.edges().len(), 8 + 2 * divided);
        assert_eq!(piece.vertices().len(), 7 + divided);
        assert_eq!(piece.edge_connected_face_components().len(), 1);
        let uses = piece.edge_use_counts();
        assert_eq!(
            uses.iter().filter(|&&count| count == 2).count(),
            2 + divided
        );
        let mut shared_ruling_pieces = 0;
        let mut section_edges = 0;
        for (edge, count) in piece.edges().iter().zip(uses) {
            if count != 2 {
                continue;
            }
            let ends = edge.vertices().map(|i| piece.vertices()[i].point());
            if ends.iter().any(|&end| end.is_near(shared, tolerance)) {
                if outer_hit.is_some_and(|hit| ends.iter().any(|&end| end.is_near(hit, tolerance)))
                {
                    section_edges += 1;
                } else {
                    shared_ruling_pieces += 1;
                }
            }
        }
        assert_eq!(shared_ruling_pieces, 2);
        assert_eq!(section_edges, divided);
        for (face, &[v, expected_z]) in piece.faces().iter().zip(face_order) {
            assert_eq!(*face.surface().domain_v().start(), v);
            let low = face.loops()[0]
                .trims()
                .iter()
                .map(|t| piece.vertices()[t.vertices()[0]].point().z())
                .min_by(f64::total_cmp)
                .unwrap();
            assert!((low - expected_z).abs() < tolerance.absolute());
        }
        assert_bounds(
            piece,
            [
                [0., -1.4072641177380512, 2.],
                [9.25050887564777, 0.5, 7.142778876534832],
            ],
            tolerance,
        );
        assert!(doc.object(source).is_none());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn oblique_boundary_selects_the_closing_or_leading_strip_by_cutter_orientation() {
    for reversed in [false, true] {
        let (curve, distance) = closed_profile();
        let plane = cutter([[0.19, -2.], [9., 0.1]], [5.; 4]);
        let surface = if reversed {
            plane.try_reversed_u().unwrap()
        } else {
            plane
        };
        let (mut doc, registry, source, boundary) =
            scene_with_geometry(curve, Geometry::NurbsSurface(surface));
        assert_eq!(
            run(&mut doc, &registry, boundary, OPTIONS, Some(distance)).unwrap(),
            TaperedBoundaryOutcome::Cut(1)
        );
        let piece = result(&doc, source, boundary);
        let (faces, edges, bounds) = if reversed {
            (2, 7, CONNECTED)
        } else {
            (
                1,
                4,
                [[-0.049029402088851426, 0., 2.], [0., 4., 7.224495767927976]],
            )
        };
        assert_eq!((piece.faces().len(), piece.edges().len()), (faces, edges));
        assert_bounds(piece, bounds, doc.tolerance());
    }
}
