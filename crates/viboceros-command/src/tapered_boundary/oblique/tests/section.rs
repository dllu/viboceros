use super::*;

fn cutter(bounds: [f64; 4], warped: bool) -> NurbsSurface {
    let [left, bottom, right, top] = bounds;
    let corner = |x, y| {
        let u = (x + 9.) / 24.;
        let v = (y + 9.) / 22.;
        let z = if warped {
            4.3 + 1.2 * u + 0.9 * v + 0.4 * u * v
        } else {
            5.
        };
        point(x, y, z)
    };
    NurbsSurface::try_bilinear([
        corner(left, bottom),
        corner(right, bottom),
        corner(right, top),
        corner(left, top),
    ])
    .unwrap()
}

fn scene(tolerance: Tolerance, support: &NurbsSurface) -> (Document, ObjectId, BoundaryRef) {
    let mut doc = Document::new(tolerance);
    let source = doc
        .add_geometry(Geometry::NurbsCurve(circle(tolerance)))
        .unwrap();
    let object = doc
        .add_geometry(Geometry::NurbsSurface(support.clone()))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, source, BoundaryRef { object, face: None })
}

fn pick(
    doc: &mut Document,
    commands: &CommandRegistry,
    reference: BoundaryRef,
    support: &NurbsSurface,
    words: &[&str],
) -> Result<TaperedBoundaryOutcome, CommandError> {
    let options = TaperedExtrusionOptions::parse(words).unwrap();
    let hit = support.evaluate(0.5, 0.018).unwrap();
    commands.execute_curve_tapered_boundary_pick(
        doc,
        reference,
        options,
        CommandContext::default().construction_plane,
        TaperedBoundaryPreview {
            distance: Some(1.832191780821918 * 37_f64.sqrt()),
            pick_offset: Some(Vector3::try_from(hit.to_array()).unwrap()),
        },
    )
}

#[test]
fn oblique_boundary_caps_a_contracted_rim_even_when_the_cutter_misses_the_projected_base() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (warped, expected, volume) in [
        (false, point(3.1276394018095055, 3., 5.), 110.25562442847803),
        (
            true,
            point(3.136774381710049, 3., 5.3996199782888965),
            113.9527032624614,
        ),
    ] {
        let support = cutter([-2., -2.7, 3.7, 2.7], warped);
        let (mut doc, source, reference) = scene(tolerance, &support);
        let commands = CommandRegistry::with_builtins();
        assert_eq!(
            pick(
                &mut doc,
                &commands,
                reference,
                &support,
                &[
                    "Direction=1,0,6",
                    "DraftAngle=8",
                    "Solid=Yes",
                    "DeleteInput=Yes",
                ]
            )
            .unwrap(),
            TaperedBoundaryOutcome::Cut(1),
            "warped={warped}",
        );
        assert!(doc.object(source).is_none());
        let Geometry::Brep(result) = doc
            .objects()
            .find(|object| object.id() != reference.object)
            .unwrap()
            .geometry()
        else {
            panic!("expected a cut B-rep");
        };
        assert!(result.is_solid());
        assert_eq!((result.faces().len(), result.edges().len()), (6, 12));
        let cap = result.faces().last().unwrap().surface();
        assert_eq!(cap.domain_u(), support.domain_u());
        assert_eq!(cap.domain_v(), support.domain_v());
        assert!(
            cap.control_points()
                .iter()
                .zip(support.control_points())
                .all(|(a, b)| a.point().is_near(b.point(), tolerance))
        );
        let result_volume = result.signed_volume(tolerance).unwrap();
        assert!(
            (result_volume - volume).abs() < 1e-4,
            "{result_volume} vs {volume}"
        );
        let bounds = Shape::from_brep(result, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        assert!(
            bounds.max().is_near(expected, tolerance),
            "{bounds:?} vs {expected:?}"
        );
    }
}

#[test]
fn oblique_boundary_finishes_cutter_picks_that_miss_the_rim_without_geometry() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (warped, solid, bounds) in [
        (false, "Yes", [0.3, -1.2, 1.8, 1.2]),
        (true, "No", [0.3, -1.2, 1.8, 1.2]),
        (false, "Yes", [7., -6., 12., 7.]),
        (true, "No", [7., -6., 12., 7.]),
    ] {
        let support = cutter(bounds, warped);
        let (mut doc, source, reference) = scene(tolerance, &support);
        let commands = CommandRegistry::with_builtins();
        let outcome = pick(
            &mut doc,
            &commands,
            reference,
            &support,
            &[
                "Direction=1,0,6",
                "DraftAngle=-10",
                &format!("Solid={solid}"),
                "DeleteInput=Yes",
            ],
        );
        assert_eq!(
            outcome.unwrap(),
            TaperedBoundaryOutcome::Fallback,
            "warped={warped}, {bounds:?}"
        );
        assert!(doc.object(source).is_none());
        assert_eq!(doc.objects().len(), 1);
        commands.execute(&mut doc, "Undo").unwrap();
        assert!(doc.is_selected(source));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn oblique_boundary_applies_delete_input_when_a_cutter_id_misses_the_rim() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (warped, bounds) in [(false, [0.3, -1.2, 1.8, 1.2]), (true, [7., -6., 12., 7.])] {
        let (mut doc, source, reference) = scene(tolerance, &cutter(bounds, warped));
        let commands = CommandRegistry::with_builtins();
        assert!(matches!(
            commands.execute(&mut doc, &format!(
                "ExtrudeCrvTapered ToBoundary={reference} Direction=1,0,6 DraftAngle=-10 Solid=Yes DeleteInput=Yes"
            )),
            Err(CommandError::SurfaceExtrusionBoundaryRejected),
        ));
        assert!(doc.object(source).is_none());
        assert_eq!(doc.objects().len(), 1);
        commands.execute(&mut doc, "Undo").unwrap();
        assert!(doc.is_selected(source));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}
