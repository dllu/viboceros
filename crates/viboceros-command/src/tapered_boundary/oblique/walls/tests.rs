use super::*;
use viboceros_geometry::Polyline3;

mod curved;
mod partial;

const RECTANGLE: [[f64; 2]; 5] = [[0., 0.], [6., 0.], [6., 4.], [0., 4.], [0., 0.]];
const REVERSED: [[f64; 2]; 5] = [[0., 0.], [0., 4.], [6., 4.], [6., 0.], [0., 0.]];
const ROTATED: [[f64; 2]; 5] = [[6., 0.], [6., 4.], [0., 4.], [0., 0.], [6., 0.]];
const WIDE: [[f64; 2]; 2] = [[-9., -9.], [15., 13.]];

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn cutter(corners: [[f64; 2]; 2], heights: [f64; 4]) -> NurbsSurface {
    let [[x, y], [xx, yy]] = corners;
    let [a, b, c, d] = heights;
    NurbsSurface::try_bilinear([
        point(x, y, a),
        point(xx, y, b),
        point(xx, yy, c),
        point(x, yy, d),
    ])
    .unwrap()
}

fn scene(
    xy: &[[f64; 2]],
    surface: NurbsSurface,
) -> (Document, CommandRegistry, ObjectId, BoundaryRef) {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    scene_with_geometry(
        Geometry::Polyline(polyline(xy, 2., tolerance)),
        Geometry::NurbsSurface(surface),
    )
}

fn polyline(xy: &[[f64; 2]], z: f64, tolerance: Tolerance) -> Polyline3 {
    Polyline3::try_new(xy.iter().map(|&[x, y]| point(x, y, z)).collect(), tolerance).unwrap()
}

fn scene_with_geometry(
    curve: Geometry,
    cutter: Geometry,
) -> (Document, CommandRegistry, ObjectId, BoundaryRef) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let source = doc.add_geometry(curve).unwrap();
    let boundary = doc.add_geometry(cutter).unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (
        doc,
        CommandRegistry::with_builtins(),
        source,
        BoundaryRef::parse(&boundary.to_string()).unwrap(),
    )
}

fn run(
    doc: &mut Document,
    registry: &CommandRegistry,
    boundary: BoundaryRef,
    options: &[&str],
    distance: Option<f64>,
) -> Result<TaperedBoundaryOutcome, CommandError> {
    registry.execute_curve_tapered_boundary(
        doc,
        boundary,
        TaperedExtrusionOptions::parse(options).unwrap(),
        CommandContext::default().construction_plane,
        distance,
    )
}

fn result(doc: &Document, source: ObjectId, cutter: BoundaryRef) -> &Brep {
    let Geometry::Brep(result) = doc
        .objects()
        .find(|obj| ![source, cutter.object].contains(&obj.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected a drafted wall")
    };
    result
}

fn assert_bounds(brep: &Brep, expected: [[f64; 3]; 2], tolerance: Tolerance) {
    let bounds = Shape::from_brep(brep, tolerance)
        .unwrap()
        .tight_bounds()
        .unwrap();
    for (actual, expected) in [bounds.min(), bounds.max()].into_iter().zip(expected) {
        assert!(
            actual
                .distance_to(Point3::try_from(expected).unwrap())
                .unwrap()
                < tolerance.absolute(),
            "{actual:?} vs {expected:?}"
        );
    }
}

#[test]
fn oblique_boundary_keeps_the_leading_directed_wall_of_a_gapped_curve() {
    let tall_stock = (-9.017123287671236_f64 - 3.).abs() * 37_f64.sqrt();
    for (profile, corners, heights, solid, expected) in [
        (
            &RECTANGLE[..],
            WIDE,
            [5.; 4],
            "No",
            [[0., -0.5362775756670342, 2.], [6.5, 0., 5.]],
        ),
        (
            &ROTATED[..],
            WIDE,
            [5.; 4],
            "No",
            [[6., 0., 2.], [7.060136039970089, 4., 5.]],
        ),
        (
            &REVERSED[..],
            WIDE,
            [5.; 4],
            "No",
            [[0., 0., 2.], [1.0601360399700892, 4., 5.]],
        ),
        (
            &RECTANGLE[..4],
            WIDE,
            [5.; 4],
            "Yes",
            [[0., -0.5362775756670342, 2.], [6.5, 0., 5.]],
        ),
        (
            &REVERSED[..2],
            WIDE,
            [-1.; 4],
            "Yes",
            [[0., 0., -1.], [0.028153569798926324, 4., 2.]],
        ),
        (
            &RECTANGLE[..],
            WIDE,
            [4.3, 5.5, 6.8, 5.2],
            "No",
            [
                [0., -0.6293425619277813, 2.],
                [6.586769418006144, 0., 5.520616508036859],
            ],
        ),
        (
            &RECTANGLE[..],
            [[5.6, 1.], [8., 5.]],
            [5.; 4],
            "No",
            [
                [0., -12.889027486408285, 2.],
                [18.017123287671154, 0., 74.10273972602694],
            ],
        ),
    ] {
        let (mut doc, registry, source, boundary) = scene(profile, cutter(corners, heights));
        let request = format!("Solid={solid}");
        assert_eq!(
            run(
                &mut doc,
                &registry,
                boundary,
                &[
                    "Direction=1,0,6",
                    "DraftAngle=-10",
                    &request,
                    "DeleteInput=Yes"
                ],
                Some(tall_stock)
            )
            .unwrap(),
            TaperedBoundaryOutcome::Cut(1),
            "{profile:?} {heights:?}",
        );
        let piece = result(&doc, source, boundary);
        assert_eq!((piece.faces().len(), piece.edges().len()), (1, 4));
        assert!(!piece.is_solid());
        assert_bounds(piece, expected, doc.tolerance());
        assert!(doc.object(source).is_none());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(!doc.is_selected(boundary.object));
    }
}

#[test]
fn oblique_boundary_replays_rhinos_base_capped_stock_for_gapped_solid_curves() {
    let distance = 12.017123287671236 * 37_f64.sqrt();
    let concave = [
        [0., 0.],
        [6., 0.],
        [6., 4.],
        [4., 4.],
        [4., 2.],
        [2., 2.],
        [2., 4.],
        [0., 4.],
        [0., 0.],
    ];
    for (profile, heights, edge_count, vertex_count, expected) in [
        (
            &RECTANGLE[..],
            [5.; 4],
            19,
            14,
            [
                [-0.6965351090274599, -12.889027486408294, 2.],
                [30.730781684369788, 16.889027486408295, 76.22168279214344],
            ],
        ),
        (
            &ROTATED[..],
            [5.; 4],
            19,
            14,
            [
                [-0.6965351090274599, -12.889027486408294, 2.],
                [30.730781684369788, 16.889027486408295, 76.22168279214344],
            ],
        ),
        (
            &REVERSED[..],
            [5.; 4],
            16,
            12,
            [
                [0., -8.889027486408294, 2.],
                [24.73078168436979, 12.889027486408294, 76.22168279214344],
            ],
        ),
        (
            &RECTANGLE[..],
            [-1.; 4],
            16,
            12,
            [
                [-24.730781684369788, -12.889027486408292, -72.22168279214343],
                [6.69653510902746, 16.889027486408292, 2.],
            ],
        ),
        (
            &ROTATED[..],
            [4.3, 5.5, 6.8, 5.2],
            19,
            14,
            [
                [-0.6965351090274723, -12.889027486408494, 2.],
                [30.73078168437017, 16.889027486408494, 76.22168279214459],
            ],
        ),
        (
            &concave[..],
            [5.; 4],
            39,
            30,
            [
                [-0.6965351090274599, -12.88902748640829, 2.],
                [30.73078168436978, 16.88902748640829, 76.22168279214341],
            ],
        ),
    ] {
        let (mut doc, registry, source, boundary) = scene(profile, cutter(WIDE, heights));
        let options = [
            "Direction=1,0,6",
            "DraftAngle=-10",
            "Solid=Yes",
            "DeleteInput=Yes",
        ];
        let short = run(&mut doc, &registry, boundary, &options, Some(1.));
        assert!(matches!(
            short,
            Err(CommandError::SurfaceExtrusionBoundaryRejected)
        ));
        assert_eq!(doc.objects().len(), 2);
        assert!(!doc.can_undo());
        assert_eq!(
            run(&mut doc, &registry, boundary, &options, Some(distance)).unwrap(),
            TaperedBoundaryOutcome::Fallback
        );
        let stock = result(&doc, source, boundary);
        assert_eq!(
            (
                stock.faces().len(),
                stock.edges().len(),
                stock.vertices().len()
            ),
            (profile.len(), edge_count, vertex_count)
        );
        assert!(!stock.is_solid());
        assert_bounds(stock, expected, doc.tolerance());
        if edge_count > 4 * (profile.len() - 1) {
            let [edge] = stock
                .edges()
                .iter()
                .zip(stock.edge_use_counts())
                .filter_map(|(edge, uses)| (uses == 2).then_some(edge))
                .collect::<Vec<_>>()[..]
            else {
                panic!("expected the base to join only the second wall")
            };
            for &[x, y] in &profile[1..3] {
                assert!(edge.vertices().iter().any(|&i| {
                    stock.vertices()[i]
                        .point()
                        .is_near(point(x, y, 2.), doc.tolerance())
                }));
            }
        }
        assert!(doc.object(source).is_none());
        assert!(doc.is_selected(boundary.object));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [source]);
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
        assert_eq!(doc.selected_object_count(), 0);
    }
}
