use super::*;
fn setup() -> (Document, CommandRegistry, ObjectId) {
    let mut doc = Document::default();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut doc, "Sphere 0,0,0 2").unwrap();
    let id = doc.objects().next().unwrap().id();
    r.execute(&mut doc, "SelAll").unwrap();
    (doc, r, id)
}

#[test]
fn options_membership_and_colors_stay_outside_history_and_preserve_redo() {
    let (mut doc, r, id) = setup();
    r.execute(&mut doc, "CurvatureAnalysis Style=Gaussian Range=-1,1")
        .unwrap();
    assert!(doc.is_selected(id));
    let before = r.curvature_analysis_view(&doc).unwrap();
    let after = r.curvature_analysis_view(&doc).unwrap();
    assert!(Arc::ptr_eq(&before.meshes, &after.meshes));
    for bad in [
        "Range=nan,1",
        "Style=NoSuchStyle",
        "ShowIsoCurves=Maybe",
        "Range=1,2 Range=3,4",
        "Add Remove",
    ] {
        assert!(
            r.execute(&mut doc, &format!("CurvatureAnalysis {bad}"))
                .is_err()
        );
        assert_eq!(
            r.curvature_analysis_view(&doc).unwrap().options,
            before.options
        );
    }
    r.execute(&mut doc, "Move 0,0,0 0,0,3").unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    r.execute(&mut doc, "CurvatureAnalysis Style=Mean Range=0,2")
        .unwrap();
    r.execute(&mut doc, "Redo").unwrap();
    r.execute(&mut doc, "CurvatureAnalysis Style=Gaussian")
        .unwrap();
    assert_eq!(
        r.curvature_analysis_view(&doc).unwrap().options.range(),
        [-1., 1.]
    );
    r.execute(&mut doc, "CurvatureAnalysis MaxRange").unwrap();
    let range = r.curvature_analysis_view(&doc).unwrap().options.range();
    assert!((range[0] - 0.25).abs() < 1e-7 && (range[1] - 0.25).abs() < 1e-7);
}

#[test]
fn surface_color_modes_replace_only_the_affected_objects() {
    let (mut doc, r, a) = setup();
    r.execute(&mut doc, "Sphere 5,0,0 2").unwrap();
    let b = doc.objects().last().unwrap().id();
    r.execute(&mut doc, "SelAll").unwrap();
    r.execute(&mut doc, "CurvatureAnalysis").unwrap();
    doc.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut doc, "DraftAngleAnalysis").unwrap();
    assert!(!r.object_has_curvature_analysis(a));
    assert!(r.object_has_draft_angle_analysis(a));
    assert!(r.object_has_curvature_analysis(b));
    doc.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut doc, "CurvatureAnalysis Add").unwrap();
    assert!(!r.object_has_draft_angle_analysis(a));
    assert!(r.object_has_curvature_analysis(a));
    assert!(r.object_has_curvature_analysis(b));
}
