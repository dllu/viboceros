use super::*;

fn p(values: [f64; 3]) -> Point3 {
    Point3::try_from(values).unwrap()
}

#[test]
fn remap_cplane_preserves_local_coordinates_across_model_and_direction_units() {
    let registry = CommandRegistry::with_builtins();
    let mut cases = 0;
    for exponent in [-600, -200, -20, 0, 20, 40, 200, 600] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let source = Frame3::try_from_unit_directions(
            p([5., -7., 11.].map(|v| v * scale)),
            Vector3::try_new(0., 1., 0.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tolerance,
        )
        .unwrap();
        for direction_exponent in [-600, 0, 600] {
            let direction_scale = 2_f64.powi(direction_exponent);
            for copy in [false, true] {
                let mut doc = Document::default();
                doc.set_tolerance(tolerance);
                let id = doc
                    .add_geometry(Geometry::Point(p([9., -5., 14.].map(|v| v * scale))))
                    .unwrap();
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
                doc.clear_history().unwrap();
                let before = doc.object(id).unwrap().clone();
                let command = format!(
                    "RemapCPlane TargetFrame={},{},{};0,0,{direction_scale};{direction_scale},0,0 Copy={}",
                    -3. * scale,
                    7. * scale,
                    11. * scale,
                    if copy { "Yes" } else { "No" }
                );
                registry
                    .execute_in_context(
                        &mut doc,
                        &command,
                        CommandContext {
                            construction_plane: source,
                        },
                    )
                    .unwrap();
                assert_eq!(doc.objects().len(), if copy { 2 } else { 1 });
                let output = if copy {
                    doc.objects().find(|o| o.id() != id).unwrap()
                } else {
                    doc.object(id).unwrap()
                };
                let Geometry::Point(actual) = output.geometry() else {
                    panic!("remapped point");
                };
                assert!(
                    actual
                        .distance_to(p([0., 11., 13.].map(|v| v * scale)))
                        .unwrap()
                        / scale
                        < 2e-12,
                    "model 2^{exponent}, direction 2^{direction_exponent}, copy {copy}"
                );
                let after = doc.objects().cloned().collect::<Vec<_>>();
                if copy {
                    assert_eq!(doc.object(id).unwrap(), &before);
                }
                assert_eq!(doc.undo_label(), Some("RemapCPlane"));
                doc.undo().unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), vec![before]);
                doc.redo().unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 48);
    eprintln!("REMAP_CPLANE_UNITS cases={cases}");
}

#[test]
fn remap_cplane_target_names_and_copy_options_have_separate_tokens() {
    for (input, expected, copy) in [
        ("CPlane=Top", Target::CPlane("Top".into()), false),
        (
            "_CPlane _Front _Copy=_Yes",
            Target::CPlane("_Front".into()),
            true,
        ),
        (
            "CPlane \"Copy\" Copy=Yes",
            Target::CPlane("Copy".into()),
            true,
        ),
        ("CPlane Copy", Target::CPlane("Copy".into()), false),
        (
            "CPlane=\"Bay = Copy\" Copy No",
            Target::CPlane("Bay = Copy".into()),
            false,
        ),
        (
            "CPlane \"Bay = Copy\" Copy Yes",
            Target::CPlane("Bay = Copy".into()),
            true,
        ),
        (
            "View \"工作视图 A\"",
            Target::View("工作视图 A".into()),
            false,
        ),
        ("CPlane\u{2003}\"Bay\"", Target::CPlane("Bay".into()), false),
    ] {
        let args = arguments(input).unwrap();
        let (args, remembered) = copy_options::arguments(&args, false);
        assert_eq!(remembered, Some(copy), "{input}");
        assert_eq!(
            options(&args, false).unwrap(),
            Options {
                target: Some(expected),
                copy
            },
            "{input}"
        );
    }
}

#[test]
fn remap_cplane_invalid_targets_preserve_objects_selection_redo_and_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Point(p([1., 2., 3.]))).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.add_geometry(Geometry::Point(p([4., 5., 6.]))).unwrap();
    doc.undo().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selection = doc.selected_object_ids().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    for args in [
        "",
        "CPlane=Top",
        "CPlane=Top Copy=Yes",
        "CPlane=missing",
        "View=missing",
        "CPlane=Front CPlane=Right",
        "CPlane=Front View=Top",
        "Copy=Yes Copy=No CPlane=Front",
        "CPlane=Front Copy=maybe",
        "CPlane=\"unfinished",
        "TargetFrame=0,0,0;0,0,0;1,0,0",
        "TargetFrame=0,0,0;1,0,0;2,0,0",
        "TargetFrame=NaN,0,0;1,0,0;0,1,0",
        "TargetFrame=0,0,0;1,0,0",
        "CPlane=Front garbage",
        "CPlane \"Bay\"tail",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("RemapCPlane {args}"))
                .is_err(),
            "{args}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before, "{args}");
        assert_eq!(
            doc.selected_object_ids().collect::<Vec<_>>(),
            selection,
            "{args}"
        );
        assert_eq!(doc.undo_label(), undo.as_deref());
        assert_eq!(doc.redo_label(), redo.as_deref());
        assert_eq!(registry.copy_default("RemapCPlane"), Some(false));
    }
}
