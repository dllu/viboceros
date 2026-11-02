use super::*;

fn setup() -> (Document, CommandRegistry, ObjectId) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "SelAll").unwrap();
    (doc, registry, id)
}

#[test]
fn membership_is_additive_and_survives_model_history() {
    let (mut doc, r, id) = setup();
    r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    assert!(!doc.is_selected(id));
    assert!(r.object_has_draft_angle_analysis(id));
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "Delete").unwrap();
    assert_eq!(r.draft_angle_analysis_view(&doc).unwrap().sources, 0);
    r.execute(&mut doc, "Undo").unwrap();
    assert!(r.object_has_draft_angle_analysis(id));
    assert_eq!(r.draft_angle_analysis_view(&doc).unwrap().sources, 1);
    r.execute(&mut doc, "DraftAngleAnalysisOff").unwrap();
    r.execute(&mut doc, "Redo").unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    assert!(r.draft_angle_analysis_view(&doc).is_none());
}

#[test]
fn options_are_atomic_retained_and_do_not_clear_redo() {
    let (mut doc, r, id) = setup();
    r.execute(
        &mut doc,
        "DraftAngleAnalysis Range=-5,5 Up=0,0,2 ShowIsoCurves=Yes",
    )
    .unwrap();
    let view = r.draft_angle_analysis_view(&doc).unwrap();
    assert_eq!(view.options.up, [0., 0., 2.]);
    for bad in [
        "Range=nan,5",
        "Range=-91,5",
        "Range=2,inf",
        "Up=0,0,0",
        "Up=1,2",
        "Range=0,5 Range=1,2",
        "Remove Add",
        "ShowIsoCurves=Maybe",
    ] {
        assert!(
            r.execute(&mut doc, &format!("DraftAngleAnalysis {bad}"))
                .is_err()
        );
        assert_eq!(
            r.draft_angle_analysis_view(&doc).unwrap().options,
            view.options
        );
    }
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "Move 0,0,0 0,0,2").unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis Range=5,-5")
        .unwrap();
    r.execute(&mut doc, "Redo").unwrap();
    assert!(r.object_has_draft_angle_analysis(id));
    r.execute(&mut doc, "DraftAngleAnalysisOff").unwrap();
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    assert_eq!(
        r.draft_angle_analysis_view(&doc).unwrap().options.range,
        [5., -5.]
    );
}

#[test]
fn add_remove_and_display_settings_have_independent_selection_policy() {
    let (mut doc, r, id) = setup();
    r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    let a = r.draft_angle_analysis_view(&doc).unwrap();
    let b = r.draft_angle_analysis_view(&doc).unwrap();
    assert!(Arc::ptr_eq(&a.objects, &b.objects));
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis ShowIsoCurves=Yes")
        .unwrap();
    assert!(doc.is_selected(id));
    r.execute(&mut doc, "DraftAngleAnalysis Remove").unwrap();
    assert!(!r.object_has_draft_angle_analysis(id));
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis Add").unwrap();
    assert!(r.object_has_draft_angle_analysis(id));
    assert!(!doc.is_selected(id));
}
