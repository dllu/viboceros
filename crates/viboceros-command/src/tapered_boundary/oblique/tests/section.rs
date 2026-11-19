use super::*;

mod slivers;

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

fn scene(
    tolerance: Tolerance,
    support: &NurbsSurface,
    surface: bool,
) -> (Document, ObjectId, BoundaryRef) {
    let mut doc = Document::new(tolerance);
    let curve = circle(tolerance);
    let profile = if surface {
        Geometry::Brep(Brep::try_planar_face(&curve, tolerance).unwrap())
    } else {
        Geometry::NurbsCurve(curve)
    };
    let source = doc.add_geometry(profile).unwrap();
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
    for (warped, rectangle, expected, volume) in [
        (
            false,
            [-2., -2.7, 3.7, 2.7],
            point(3.1276394018095055, 3., 5.),
            Some(110.25562442847803),
        ),
        (
            true,
            [-2., -2.7, 3.7, 2.7],
            point(3.136774381710049, 3., 5.3996199782888965),
            Some(113.9527032624614),
        ),
        (
            false,
            [-2., -2.2876659672906228, 3.7, 2.7],
            point(3.1276394018095064, 3., 5.),
            Some(110.25562442776386),
        ),
        (
            true,
            [-2., -2.7, 3.136774381710049, 2.7],
            point(3.136774381710049, 3., 5.399619978288897),
            Some(113.9527032637002),
        ),
        (
            false,
            [-2., -2.2876659672906228, 3.7, 2.7],
            point(3.1276394018095064, 3., 5.),
            None,
        ),
        (
            true,
            [-2., -2.7, 3.136774381710049, 2.7],
            point(3.136774381710049, 3., 5.399619978288898),
            None,
        ),
    ] {
        let support = cutter(rectangle, warped);
        let (mut doc, source, reference) = scene(tolerance, &support, false);
        let commands = CommandRegistry::with_builtins();
        let solid = if volume.is_some() { "Yes" } else { "No" };
        assert_eq!(
            pick(
                &mut doc,
                &commands,
                reference,
                &support,
                &[
                    "Direction=1,0,6",
                    "DraftAngle=8",
                    &format!("Solid={solid}"),
                    "DeleteInput=Yes",
                ]
            )
            .unwrap_or_else(|error| panic!("{warped}, {rectangle:?}: {error}")),
            TaperedBoundaryOutcome::Cut(1),
            "warped={warped}, solid={solid}, {rectangle:?}",
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
        assert_eq!(result.is_solid(), volume.is_some());
        assert_eq!(result.edges().len(), 12);
        if let Some(volume) = volume {
            assert_eq!(result.faces().len(), 6);
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
        } else {
            assert_eq!(result.faces().len(), 4);
        }
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
fn oblique_boundary_keeps_preview_stock_when_the_cutter_just_crosses_the_rim() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (warped, bounds) in [
        (false, [-2., -2.2875228972462147, 3.7, 2.7]),
        (true, [-2., -2.7, 3.1366943817100488, 2.7]),
    ] {
        let support = cutter(bounds, warped);
        let (mut doc, source, reference) = scene(tolerance, &support, false);
        let commands = CommandRegistry::with_builtins();
        let outcome = pick(
            &mut doc,
            &commands,
            reference,
            &support,
            &[
                "Direction=1,0,6",
                "DraftAngle=8",
                "Solid=Yes",
                "DeleteInput=Yes",
            ],
        );
        assert_eq!(
            outcome.unwrap(),
            TaperedBoundaryOutcome::Fallback,
            "{bounds:?}"
        );
        assert!(doc.object(source).is_none());
        let Geometry::Brep(result) = doc
            .objects()
            .find(|object| object.id() != reference.object)
            .unwrap()
            .geometry()
        else {
            panic!("expected the open preview stock");
        };
        assert_eq!((result.faces().len(), result.edges().len()), (5, 12));
        assert!(!result.is_solid());
        let bounds = Shape::from_brep(result, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        assert!(
            bounds
                .max()
                .is_near(point(3.2872052075524794, 3., 11.250648447143103), tolerance),
            "{bounds:?}"
        );
    }
}

#[test]
fn oblique_boundary_finishes_cutter_picks_that_miss_the_rim_without_geometry() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (warped, solid, draft, bounds) in [
        (false, "Yes", -10, [0.3, -1.2, 1.8, 1.2]),
        (true, "No", -10, [0.3, -1.2, 1.8, 1.2]),
        (false, "Yes", -10, [7., -6., 12., 7.]),
        (true, "No", -10, [7., -6., 12., 7.]),
        (false, "Yes", 8, [-2., -5., 3.7, -2.2876659672906228]),
        (true, "Yes", 8, [3.136774381710049, -2.7, 6.5, 2.7]),
    ] {
        let support = cutter(bounds, warped);
        let (mut doc, source, reference) = scene(tolerance, &support, false);
        let commands = CommandRegistry::with_builtins();
        let outcome = pick(
            &mut doc,
            &commands,
            reference,
            &support,
            &[
                "Direction=1,0,6",
                &format!("DraftAngle={draft}"),
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
        let (mut doc, source, reference) = scene(tolerance, &cutter(bounds, warped), false);
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
