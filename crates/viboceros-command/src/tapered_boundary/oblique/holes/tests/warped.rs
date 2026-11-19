use super::*;

const INSIDE: [f64; 4] = [0.8, -0.25, 1.3, 0.25];
const RIM: [f64; 4] = [4.6, -0.25, 5.3, 0.25];
const COVER: [f64; 4] = [-7., -7., 9., 7.];
const LONG: f64 = 14.94178082191781;
const SHORT: f64 = 0.4006849315068499;

fn cutter(hole: [f64; 4], tolerance: Tolerance) -> Brep {
    let surface = NurbsSurface::try_bilinear([
        point(-9., -9., 4.3),
        point(15., -9., 5.5),
        point(15., 13., 6.8),
        point(-9., 13., 5.2),
    ])
    .unwrap();
    let [x, y, xx, yy] = hole;
    let points = [[x, y], [xx, y], [xx, yy], [x, yy], [x, y]]
        .map(|[x, y]| surface.evaluate((x + 9.) / 24., (y + 9.) / 22.).unwrap());
    let wire = Polyline3::try_new(points.to_vec(), tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let [trimmed, _] = Brep::try_split_rectangular_surface_face_with_closed_curve(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        wire,
        false,
        tolerance,
    )
    .unwrap();
    trimmed
}

fn scene(hole: [f64; 4], surface: bool) -> (Document, CommandRegistry, ObjectId, ObjectId) {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let mut doc = Document::new(tolerance);
    let profile = circle(tolerance);
    let source = doc
        .add_geometry(if surface {
            Geometry::Brep(Brep::try_planar_face(&profile, tolerance).unwrap())
        } else {
            Geometry::NurbsCurve(profile)
        })
        .unwrap();
    let cutter = doc
        .add_geometry(Geometry::Brep(cutter(hole, tolerance)))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, CommandRegistry::with_builtins(), source, cutter)
}

fn cut(
    doc: &mut Document,
    commands: &CommandRegistry,
    cutter: ObjectId,
    solid: bool,
    pointer_x: f64,
) -> Result<TaperedBoundaryOutcome, CommandError> {
    let mut options =
        TaperedExtrusionOptions::parse(&["Direction=1,0,6", "DraftAngle=-10", "DeleteInput=Yes"])
            .unwrap();
    options.solid = solid;
    commands.execute_curve_tapered_boundary_pick(
        doc,
        BoundaryRef {
            object: cutter,
            face: None,
        },
        options,
        CommandContext::default().construction_plane,
        TaperedBoundaryPreview {
            distance: Some(pointer_x * 37_f64.sqrt()),
            pick_offset: Some(Vector3::try_new(-9., -2., 4.586).unwrap()),
        },
    )
}

fn output(doc: &Document, source: ObjectId, cutter: ObjectId) -> &Brep {
    let Geometry::Brep(piece) = doc
        .objects()
        .find(|obj| ![source, cutter].contains(&obj.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected drafted geometry");
    };
    piece
}

fn assert_height(piece: &Brep, expected: f64, tolerance: Tolerance) {
    let height = Shape::from_brep(piece, tolerance)
        .unwrap()
        .tight_bounds()
        .unwrap()
        .max()
        .z();
    assert!(
        (height - expected).abs() < tolerance.absolute(),
        "{height} vs {expected}"
    );
}

#[test]
fn oblique_boundary_warped_cutter_keeps_enclosed_holes_open_in_the_cap() {
    for (hole, solid, topology, closed) in [
        (INSIDE, true, (6, 16), false),
        (OUTSIDE, true, (6, 12), true),
        (INSIDE, false, (4, 12), false),
    ] {
        let (mut doc, commands, source, cutter) = scene(hole, false);
        let Geometry::Brep(cutting_face) = doc.object(cutter).unwrap().geometry() else {
            panic!("expected a trimmed cutter");
        };
        let surface = cutting_face.faces()[0].surface().clone();
        assert_eq!(
            cut(&mut doc, &commands, cutter, solid, LONG).unwrap(),
            TaperedBoundaryOutcome::Cut(1)
        );
        let piece = output(&doc, source, cutter);
        assert_eq!((piece.faces().len(), piece.edges().len()), topology);
        assert_eq!(piece.is_solid(), closed);
        assert_height(piece, 5.538213309250741, doc.tolerance());
        if solid && hole == INSIDE {
            let cap = piece.faces().last().unwrap();
            assert_eq!(cap.surface(), &surface);
            assert_eq!(cap.loops().len(), 2);
            assert_eq!(cap.loops()[1].trims().len(), 4);
            assert!(
                cap.loops()[1]
                    .trims()
                    .iter()
                    .all(|trim| trim.trim_type() == BrepTrimType::Boundary)
            );
            assert_eq!(
                piece
                    .edge_use_counts()
                    .into_iter()
                    .filter(|&uses| uses == 1)
                    .count(),
                4
            );
        }
        assert!(doc.object(source).is_none());
        commands.execute(&mut doc, "Undo").unwrap();
        assert!(doc.is_selected(source));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn oblique_boundary_warped_holes_replay_the_captured_stock_or_empty_cut() {
    for (hole, solid, pointer, stock_height) in [
        (RIM, true, LONG, Some(92.28532403024347)),
        (RIM, true, 1.455479452054795, Some(8.989517009592825)),
        (RIM, false, LONG, None),
        (COVER, true, LONG, None),
        (INSIDE, true, SHORT, None),
    ] {
        let (mut doc, commands, source, cutter) = scene(hole, false);
        assert_eq!(
            cut(&mut doc, &commands, cutter, solid, pointer).unwrap(),
            TaperedBoundaryOutcome::Fallback
        );
        assert!(doc.object(source).is_none());
        assert!(doc.is_selected(cutter));
        if let Some(height) = stock_height {
            let piece = output(&doc, source, cutter);
            assert_eq!((piece.faces().len(), piece.edges().len()), (5, 12));
            assert!(!piece.is_solid());
            assert_height(piece, height, doc.tolerance());
        } else {
            assert_eq!(doc.objects().len(), 1);
        }
        commands.execute(&mut doc, "Undo").unwrap();
        assert!(doc.is_selected(source));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
        assert_eq!(doc.selected_object_count(), 0);
    }
}

#[test]
fn oblique_surface_boundary_warped_hole_failures_keep_the_source() {
    for (hole, fallback) in [(INSIDE, false), (RIM, true)] {
        let (mut doc, commands, source, cutter) = scene(hole, true);
        let result = commands.execute(
            &mut doc,
            &format!("ExtrudeSrfTapered ToBoundary={cutter} Direction=1,0,6 DraftAngle=-10 DeleteInput=Yes"),
        );
        assert!(matches!(
            result,
            Err(CommandError::SurfaceExtrusionBoundaryRejected)
        ));
        assert!(doc.object(source).is_some());
        assert!(doc.is_selected(source));
        if fallback {
            let piece = output(&doc, source, cutter);
            assert_eq!(piece.faces().len(), 5);
            assert_height(piece, 33.771791300688776, doc.tolerance());
        } else {
            assert_eq!(doc.objects().len(), 2);
            assert!(!doc.can_undo());
        }
    }
}
