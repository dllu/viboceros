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
