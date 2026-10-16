use super::*;
use viboceros_document::{ObjectColorSource, SelectionMode};
fn setup() -> (CommandRegistry, Document, [ObjectId; 2]) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    registry.execute(&mut doc, "Line 3,2,0 7,2,0").unwrap();
    let ids = doc
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    (registry, doc, ids)
}
#[test]
fn crv_deviation_report_and_options_preserve_model_and_redo() {
    let (r, mut d, ids) = setup();
    r.execute(&mut d, "Point 8,9,10").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    let text = r.execute(&mut d, "_-CrvDeviation").unwrap();
    assert!(text.contains("Minimum deviation = 2"));
    let v = r.curve_deviation_view(&d).unwrap();
    assert_eq!(v.ends[0].points[0].to_array(), [3., 0., 0.]);
    assert_eq!(v.ends[1].points[0].to_array(), [7., 0., 0.]);
    assert!(v.approximate_overlap);
    r.execute(&mut d, "CrvDeviation KeepMarks=Yes").unwrap();
    assert!(r.curve_deviation_view(&d).unwrap().keep_marks);
    r.execute(&mut d, "CrvDeviation End KeepMarks=No").unwrap();
    assert!(r.curve_deviation_view(&d).is_none());
    assert_eq!(format!("{d:?}"), before);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().len(), 3);
}
#[test]
fn crv_deviation_kept_marks_have_native_colors_and_one_undo_step() {
    let (r, mut d, ids) = setup();
    let layer = d.add_layer("Marks", ColorRgb::new(9, 8, 7)).unwrap();
    d.set_current_layer(layer).unwrap();
    r.execute(&mut d, "CrvDeviation KeepMarks=Yes").unwrap();
    assert_eq!(d.objects().len(), 2);
    r.execute(&mut d, "CrvDeviation End").unwrap();
    assert_eq!(d.objects().len(), 14);
    let mut colors = BTreeMap::new();
    for o in d.objects().filter(|o| !ids.contains(&o.id())) {
        assert!(!d.is_selected(o.id()));
        assert_eq!(o.attributes().layer_id(), layer);
        assert_eq!(o.attributes().color_source(), ObjectColorSource::Object);
        let c = o.attributes().object_color();
        *colors.entry([c.red, c.green, c.blue]).or_insert(0) += 1;
    }
    assert_eq!(
        colors,
        BTreeMap::from([([255, 255, 255], 6), ([0, 255, 0], 3), ([255, 0, 0], 3)])
    );
    assert_eq!(d.undo_label(), Some("CrvDeviation"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().len(), 14);
}
#[test]
fn crv_deviation_cancel_and_invalid_arguments_do_not_keep_marks() {
    let (r, mut d, _) = setup();
    r.execute(&mut d, "CrvDeviation KeepMarks=Yes").unwrap();
    for input in [
        "CrvDeviation KeepMarks=maybe",
        "CrvDeviation KeepMarks=Yes KeepMarks=No",
        "CrvDeviation End junk",
        "CrvDeviation Cancel KeepMarks=Yes",
        "CrvDeviation SubCrv",
    ] {
        assert!(r.execute(&mut d, input).is_err(), "{input}");
        assert!(r.curve_deviation_view(&d).is_some());
    }
    r.execute(&mut d, "CrvDeviation Cancel").unwrap();
    assert!(r.curve_deviation_view(&d).is_none());
    assert_eq!(d.objects().len(), 2);
}
#[test]
fn crv_deviation_geometry_and_tolerance_changes_expire_display_and_keep_request() {
    for mode in 0..3 {
        let (r, mut d, _) = setup();
        r.execute(&mut d, "CrvDeviation KeepMarks=Yes").unwrap();
        if mode == 0 {
            r.execute(&mut d, "Move 0,0,0 0,0,1").unwrap();
        }
        if mode == 1 {
            r.execute(&mut d, "Delete").unwrap();
        }
        if mode == 2 {
            d.set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-10).unwrap());
        }
        assert!(r.execute(&mut d, "CrvDeviation End").is_err());
        assert!(r.curve_deviation_view(&d).is_none());
        assert_eq!(d.objects().len(), if mode == 1 { 0 } else { 2 });
    }
}
#[test]
fn crv_deviation_coincident_witnesses_keep_one_point_per_mark() {
    let (r, mut d, ids) = setup();
    r.execute(&mut d, "Line 0,0,0 10,0,0").unwrap();
    let other = d.objects().last().unwrap().id();
    d.select_objects_direct([ids[0], other], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "CrvDeviation KeepMarks=Yes").unwrap();
    r.execute(&mut d, "CrvDeviation End").unwrap();
    let marks = d
        .objects()
        .filter(|o| ![ids[0], ids[1], other].contains(&o.id()))
        .collect::<Vec<_>>();
    assert_eq!(marks.len(), 4);
    assert!(
        marks
            .iter()
            .all(|o| matches!(o.geometry(), Geometry::Point(_)))
    );
}
#[test]
fn crv_deviation_postselection_and_wrong_counts() {
    let (r, mut d, ids) = setup();
    d.clear_selection();
    assert!(r.execute(&mut d, "CrvDeviation").is_err());
    d.select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    assert!(r.execute(&mut d, "CrvDeviation").is_err());
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    r.execute_postselected(&mut d, "CrvDeviation", CommandContext::default())
        .unwrap();
    assert_eq!(d.selected_object_count(), 0);
    assert!(r.curve_deviation_view(&d).is_some());
}
#[test]
fn crv_deviation_subcurve_inputs_override_whole_selection_and_preserve_redo() {
    let (r, mut d, ids) = setup();
    r.execute(&mut d, "Point 8,9,10").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    r.execute(&mut d, &format!("CrvDeviation SubCrv={},3,7", ids[0]))
        .unwrap();
    let v = r.curve_deviation_view(&d).unwrap();
    assert_eq!(v.ends[0].parameters, [3., 0.]);
    assert_eq!(v.ends[1].parameters, [7., 4.]);
    assert_eq!(v.extrema.minimum.distance, 2.);
    assert_eq!(v.extrema.maximum.distance, 2.);
    assert_eq!(format!("{d:?}"), before);
    r.execute(&mut d, "CrvDeviation End").unwrap();
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().len(), 3);
}
#[test]
fn crv_deviation_reverse_subcurve_keeps_negative_native_domain_and_undoable_marks() {
    let (r, mut d, ids) = setup();
    let sources = ids.map(|id| d.object(id).unwrap().geometry().clone());
    r.execute(
        &mut d,
        &format!("CrvDeviation SubCrv={},7,3 KeepMarks=Yes", ids[0]),
    )
    .unwrap();
    let v = r.curve_deviation_view(&d).unwrap();
    assert_eq!(v.ends[0].parameters, [-7., 4.]);
    assert_eq!(v.ends[1].parameters, [-3., 0.]);
    assert_eq!(v.ends[0].points[0].to_array(), [7., 0., 0.]);
    r.execute(&mut d, "CrvDeviation End").unwrap();
    assert_eq!(d.objects().len(), 14);
    for (id, source) in ids.into_iter().zip(&sources) {
        assert_eq!(d.object(id).unwrap().geometry(), source);
    }
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
}
#[test]
fn crv_deviation_two_parts_of_one_owner_bind_to_original_geometry() {
    let (r, mut d, ids) = setup();
    d.clear_selection();
    r.execute(
        &mut d,
        &format!("CrvDeviation SubCrv={},0,6 SubCrv={},4,10", ids[0], ids[0]),
    )
    .unwrap();
    let v = r.curve_deviation_view(&d).unwrap();
    assert_eq!(v.extrema.maximum.distance, 0.);
    d.select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "Move 0,0,0 0,0,1").unwrap();
    assert!(r.curve_deviation_view(&d).is_none());
    assert_eq!(d.objects().len(), 2);
}
#[test]
fn crv_deviation_invalid_parts_leave_report_and_document_unchanged() {
    let (r, mut d, ids) = setup();
    r.execute(&mut d, "CrvDeviation").unwrap();
    let before = format!("{d:?}");
    for suffix in [
        format!("SubCrv={},3,3", ids[0]),
        format!("SubCrv={},-1,7", ids[0]),
        format!("SubCrv={},NaN,7", ids[0]),
        "SubCrv=999999,0,1".into(),
        format!("End SubCrv={},3,7", ids[0]),
        format!(
            "SubCrv={},0,2 SubCrv={},3,4 SubCrv={},6,8",
            ids[0], ids[0], ids[0]
        ),
    ] {
        assert!(
            r.execute(&mut d, &format!("CrvDeviation {suffix}"))
                .is_err(),
            "{suffix}"
        );
        assert_eq!(format!("{d:?}"), before);
        assert!(r.curve_deviation_view(&d).is_some());
    }
}
#[test]
fn crv_deviation_new_parts_restart_analysis_instead_of_only_updating_options() {
    let (r, mut d, ids) = setup();
    r.execute(&mut d, "CrvDeviation").unwrap();
    assert_eq!(
        r.curve_deviation_view(&d).unwrap().extrema.intervals[0],
        [0.3, 0.7]
    );
    r.execute(&mut d, &format!("CrvDeviation SubCrv={},3,7", ids[0]))
        .unwrap();
    assert_eq!(
        r.curve_deviation_view(&d).unwrap().extrema.intervals[0],
        [0., 1.]
    );
}
#[cfg(feature = "native-smlib")]
#[test]
fn crv_deviation_curved_backend_is_usable_and_exposes_overlap_limit() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Circle 0,0,0 2").unwrap();
    r.execute(&mut d, "Circle 0,0,0 3").unwrap();
    let ids = d.objects().map(|o| o.id()).collect::<Vec<_>>();
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "CrvDeviation").unwrap();
    let v = r.curve_deviation_view(&d).unwrap();
    assert!(v.approximate_overlap);
    assert!((v.extrema.maximum.distance - 1.).abs() < 1e-6);
    assert!((v.extrema.minimum.distance - 1.).abs() < 1e-6);
    assert_eq!(v.ends[0].parameters, [0., 0.]);
    assert!(v.ends[1].parameters[0] > 0. && v.ends[1].parameters[1] > 0.);
}
