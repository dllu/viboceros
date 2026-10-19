use super::*;

fn polyline() -> Geometry {
    Geometry::Polyline(
        Polyline3::try_new(
            [[0., 0., 0.], [1., 0.1, 0.], [2., 0., 0.], [3., 2., 0.]]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}
#[test]
fn invalid_input_noops_and_later_bad_sources_preserve_geometry_selection_and_redo() {
    let mut doc = Document::new(Tolerance::DEFAULT);
    let registry = CommandRegistry::with_builtins();
    let id = doc.add_geometry(polyline()).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for text in [
        "ReducePolyline 0",
        "ReducePolyline -1",
        "ReducePolyline NaN",
        "ReducePolyline 0.1 0.2",
        "ReducePolyline Copy=Maybe",
        "ReducePolyline Tolerance=0.1 Tolerance=0.2",
    ] {
        assert!(registry.execute(&mut doc, text).is_err(), "{text}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.is_selected(id));
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
    registry
        .execute(&mut doc, "ReducePolyline 0.00001 Copy=Yes")
        .unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    let bad = doc
        .add_geometry(Geometry::Polyline(
            Polyline3::try_new(
                [
                    [10., 0., 0.],
                    [10.01, 0., 0.],
                    [10.01, 0.01, 0.],
                    [10., 0.01, 0.],
                    [10., 0., 0.],
                ]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([id, bad], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(registry.execute(&mut doc, "ReducePolyline 0.2").is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.is_selected(id) && doc.is_selected(bad));
    assert!(!doc.can_undo());
}

#[test]
fn parser_accepts_numeric_and_script_tolerance_forms_without_mutation() {
    for args in [
        vec!["0.2", "Copy=Yes"],
        vec!["Tolerance=0.2", "Copy", "Yes"],
        vec!["_Tolerance", "0.2", "_Copy=Yes"],
    ] {
        assert_eq!(
            parse(&args, Input::default()).unwrap(),
            Input {
                tolerance: Some(0.2),
                copy: Some(true)
            }
        );
    }
}
