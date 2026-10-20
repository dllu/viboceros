use viboceros_command::CommandRegistry;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[allow(dead_code)]
#[path = "support/splop.rs"]
mod support;
#[test]
fn native_splop_geometry_groups_selection_and_history() {
    let mut failures = Vec::new();
    let rows = support::rows();
    for (op, value) in &rows {
        let result = std::panic::catch_unwind(|| {
            let (mut doc, sources) = support::setup(op, value);
            sources.check(&doc, op, value, "before");
            sources.execute(&mut doc, op, &CommandRegistry::with_builtins());
            let fit = sources.check_fit(&doc, op);
            println!(
                "SPLOP_FIT_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":fit})
            );
            let mut gap = sources.check(&doc, op, value, "after_script");
            assert!(doc.undo().unwrap().is_some());
            gap = gap.max(sources.check(&doc, op, value, "undo"));
            assert!(doc.redo().unwrap().is_some());
            gap = gap.max(sources.check(&doc, op, value, "redo"));
            println!(
                "SPLOP_API_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn complete_point_directions_and_partial_control_grips_keep_basis_and_undo() {
    use viboceros_document::{ControlPointId, Geometry, SelectionMode};
    use viboceros_geometry::PointMorph;
    for copy in [false, true] {
        for rigid in [false, true] {
            let (op, value) = support::rows()
                .into_iter()
                .find(|(op, _)| op["id"] == "splop_12")
                .unwrap();
            let (mut doc, sources) = support::setup(&op, &value);
            let owner = sources.ids[0];
            let before = doc.object(owner).unwrap().geometry().clone();
            let Geometry::NurbsCurve(curve) = &before else {
                panic!()
            };
            doc.enable_control_points([owner]).unwrap();
            doc.clear_selection();
            let grips = [0, 2].map(|index| ControlPointId {
                object: owner,
                index,
            });
            doc.select_control_points(grips, SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let placement = viboceros_command::splop::Placement::try_new(
                viboceros_command::splop::ReferenceSphere::try_new(
                    viboceros_geometry::Point3::try_from([0.; 3]).unwrap(),
                    2.,
                    sources.plane,
                )
                .unwrap(),
                viboceros_command::orient_surface::Target::capture(&doc, sources.guide, None)
                    .unwrap(),
                sources.target,
            )
            .unwrap();
            let options = viboceros_command::splop::Options {
                copy,
                rigid,
                flip: false,
            };
            let morph = placement.morph(4., 0., options, doc.tolerance()).unwrap();
            let input = format!(
                "Splop 0,0,0 2 {} {} Surface={} {}",
                support::token(sources.target),
                support::token(sources.direction(4., 0.)),
                sources.guide,
                options.arguments()
            );
            CommandRegistry::with_builtins()
                .execute(&mut doc, &input)
                .unwrap();
            let edited = doc
                .objects()
                .filter(|o| o.id() != sources.guide)
                .last()
                .unwrap()
                .geometry()
                .clone();
            let Geometry::NurbsCurve(edited_curve) = &edited else {
                panic!()
            };
            assert_eq!(curve.degree(), edited_curve.degree());
            assert_eq!(curve.knots(), edited_curve.knots());
            for (i, (a, b)) in curve
                .control_points()
                .iter()
                .zip(edited_curve.control_points())
                .enumerate()
            {
                assert_eq!(a.weight(), b.weight());
                let expected = if [0, 2].contains(&i) {
                    morph.morph_point(a.point()).unwrap()
                } else {
                    a.point()
                };
                assert!(expected.distance_to(b.point()).unwrap() < 1e-11);
            }
            if copy {
                assert_eq!(doc.object(owner).unwrap().geometry(), &before);
            }
            assert!(doc.undo().unwrap().is_some());
            assert_eq!(doc.object(owner).unwrap().geometry(), &before);
            assert_eq!(doc.objects().len(), 2);
            assert!(doc.redo().unwrap().is_some());
            assert_eq!(
                doc.objects()
                    .filter(|o| o.id() != sources.guide)
                    .last()
                    .unwrap()
                    .geometry(),
                &edited
            );
        }
    }
}

#[test]
fn invalid_spheres_and_unavailable_targets_leave_geometry_preferences_and_redo() {
    use viboceros_document::Geometry;
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "splop_0")
        .unwrap();
    let (mut doc, sources) = support::setup(&op, &value);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = doc
        .objects()
        .map(|o| (o.id(), o.geometry().clone(), doc.is_selected(o.id())))
        .collect::<Vec<_>>();
    let redo = doc.redo_label().map(str::to_owned);
    let options = registry.begin_copy_options("Splop");
    for args in [
        "0,0,0 0 0,0,5 4",
        "0,0,0 -1 0,0,5 4",
        "0,0,0 2 0,0,5 0",
        "0,0,0 NaN 0,0,5 4",
        "0,0,0 2 0,0,5 4 Angle=1e309",
        "0,0,0 2 0,0,5 4 Copy=Maybe",
        "0,0,0 2 0,0,5 4 Face=8",
        "0,0,0 2 0,0,5 4 Copy=Yes Copy=No",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("Splop {args} Surface={}", sources.guide))
                .is_err(),
            "{args}"
        );
        assert_eq!(
            doc.objects()
                .map(|o| (o.id(), o.geometry().clone(), doc.is_selected(o.id())))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(doc.redo_label(), redo.as_deref());
        assert_eq!(registry.begin_copy_options("Splop"), options);
    }
    assert!(matches!(
        doc.object(sources.ids[0]).unwrap().geometry(),
        Geometry::PointCloud(_)
    ));
}
