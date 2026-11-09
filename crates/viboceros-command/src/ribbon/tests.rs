use super::*;

#[test]
fn options_reject_duplicate_values_and_resolve_model_units() {
    use viboceros_geometry::LengthUnitSystem;
    let input = parse(
        &["Distance=2cm", "w1,2,3", "Corner=Smooth", "BothSides"],
        Options::default(),
        &LengthUnitSystem::Millimeters,
        CommandContext::default(),
    )
    .unwrap();
    assert_eq!(input.options.distance, 20.);
    assert!(input.options.both_sides);
    assert!(!input.options.remembered().both_sides);
    assert_eq!(input.side.unwrap(), Point3::try_new(1., 2., 3.).unwrap());
    for args in [
        vec!["0"],
        vec!["-1"],
        vec!["nan"],
        vec!["Distance=1", "2"],
        vec!["Loose=Yes", "Loose=No"],
        vec!["OutputLayer=Target"],
        vec!["w0,0,0", "w1,2,3"],
    ] {
        assert!(
            parse(
                &args,
                Options::default(),
                &LengthUnitSystem::Millimeters,
                CommandContext::default()
            )
            .is_err(),
            "{args:?}"
        );
    }
}

#[test]
fn command_keeps_preselection_and_creates_unselected_output_in_one_undo_step() {
    let mut doc = Document::new(Tolerance::DEFAULT);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Line 0,0,0 5,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let source = doc.selected_objects().next().unwrap().clone();
    registry.execute(&mut doc, "Ribbon .5 w0,2,0").unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(
        doc.selected_object_ids().collect::<Vec<_>>(),
        vec![source.id()]
    );
    let result = doc
        .objects()
        .find(|o| o.id() != source.id())
        .unwrap()
        .clone();
    assert!(matches!(result.geometry(), Geometry::Brep(_)));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(
        doc.object(source.id()).unwrap().geometry(),
        source.geometry()
    );
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(
        doc.object(result.id()).unwrap().geometry(),
        result.geometry()
    );
}

#[test]
fn remembered_options_are_registry_local_and_failures_preserve_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Line 0,0,0 5,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(
            &mut doc,
            "Ribbon 2 w0,5,0 BothSides=Yes OutputLayer=Input Corner=Round",
        )
        .unwrap();
    let accepted = registry.ribbon_options();
    assert_eq!(accepted.distance, 2.);
    assert!(accepted.input_layer);
    assert!(!accepted.both_sides);
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    for text in [
        "Ribbon 0 w0,5,0",
        "Ribbon nan w0,5,0",
        "Ribbon Distance=1 Distance=2 w0,5,0",
    ] {
        assert!(registry.execute(&mut doc, text).is_err());
        assert_eq!(format!("{doc:?}"), before);
        assert_eq!(registry.ribbon_options(), accepted);
    }
    assert!(doc.can_redo());
    assert_eq!(
        CommandRegistry::with_builtins().ribbon_options(),
        Options::default()
    );
}

#[test]
fn coarse_fit_tolerance_does_not_collapse_or_sew_distinct_boundaries() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Circle 0,0,0 3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let mut coarse = doc.clone();
    registry
        .execute(&mut doc, "Ribbon 1 w5,0,0 BothSides=Yes")
        .unwrap();
    registry
        .execute(&mut coarse, "Ribbon 1 w5,0,0 BothSides=Yes Tolerance=10")
        .unwrap();
    let result = |d: &Document| {
        d.objects()
            .find_map(|o| {
                if let Geometry::Brep(b) = o.geometry() {
                    Some(b.clone())
                } else {
                    None
                }
            })
            .unwrap()
    };
    let (a, b) = (result(&doc), result(&coarse));
    assert_eq!(a.faces().len(), 2);
    assert_eq!(a, b);
}
