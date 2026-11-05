use super::*;

fn setup() -> (Document, CommandRegistry, ObjectId) {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Line 0,0,0 4,2,1").unwrap();
    let id = doc.objects().last().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, commands, id)
}

#[test]
fn display_options_and_off_preserve_the_model_selection_and_pending_redo() {
    let (mut doc, commands, id) = setup();
    commands.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    commands.execute(&mut doc, "ShowDir").unwrap();
    let a = commands.direction_analysis_view(&doc).unwrap();
    let b = commands.direction_analysis_view(&doc).unwrap();
    assert!(Arc::ptr_eq(&a.markers, &b.markers));
    assert_eq!(a.sources, 1);
    assert_eq!(a.markers.len(), 3);
    commands
        .execute(&mut doc, "ShowDir U=No V=No Direction=No Color=12,34,56")
        .unwrap();
    let c = commands.direction_analysis_view(&doc).unwrap();
    assert!(Arc::ptr_eq(&a.markers, &c.markers));
    assert!(!c.options.direction && !c.options.u && !c.options.v);
    assert_eq!(c.options.color, [12, 34, 56]);
    commands.execute(&mut doc, "ShowDirOff").unwrap();
    assert!(commands.direction_analysis_view(&doc).is_none());
    assert!(!commands.object_has_direction_analysis(id));
    assert_eq!(format!("{doc:?}"), before);
    commands.execute(&mut doc, "ShowDir").unwrap();
    assert_eq!(
        commands.direction_analysis_view(&doc).unwrap().options,
        c.options
    );
}

#[test]
fn cached_markers_follow_edits_visibility_and_delete_undo() {
    let (mut doc, commands, id) = setup();
    commands.execute(&mut doc, "ShowDir").unwrap();
    let a = commands.direction_analysis_view(&doc).unwrap();
    commands.execute(&mut doc, "Move 0,0,0 0,0,5").unwrap();
    let b = commands.direction_analysis_view(&doc).unwrap();
    assert!(!Arc::ptr_eq(&a.markers, &b.markers));
    for (a, b) in a.markers.iter().zip(b.markers.iter()) {
        assert!((b.point.z() - a.point.z() - 5.).abs() < 1e-12);
    }
    commands.execute(&mut doc, "Hide").unwrap();
    assert!(commands.object_has_direction_analysis(id));
    assert!(
        commands
            .direction_analysis_view(&doc)
            .unwrap()
            .markers
            .is_empty()
    );
    commands.execute(&mut doc, "Show").unwrap();
    assert_eq!(
        commands
            .direction_analysis_view(&doc)
            .unwrap()
            .markers
            .len(),
        3
    );
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    commands.execute(&mut doc, "Delete").unwrap();
    assert_eq!(commands.direction_analysis_view(&doc).unwrap().sources, 0);
    doc.undo().unwrap();
    assert!(doc.object(id).is_some());
    assert_eq!(
        commands
            .direction_analysis_view(&doc)
            .unwrap()
            .markers
            .len(),
        3
    );
    commands.execute(&mut doc, "ShowDirOff").unwrap();
    doc.redo().unwrap();
    doc.undo().unwrap();
    assert!(commands.direction_analysis_view(&doc).is_none());
}

#[test]
fn edit_controls_use_current_selection_and_keep_native_history_labels() {
    let (mut doc, commands, id) = setup();
    commands.execute(&mut doc, "ShowDir").unwrap();
    let original = doc.object(id).unwrap().geometry_snapshot().clone();
    commands.execute(&mut doc, "Line 10,0,0 14,2,1").unwrap();
    let other = doc.objects().last().unwrap().id();
    doc.select_objects_direct([other], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(other).unwrap().geometry().clone();
    commands.execute(&mut doc, "ShowDir Flip").unwrap();
    assert_eq!(doc.undo_label(), Some("Flip"));
    let (Geometry::Line(before), Geometry::Line(after)) =
        (&before, doc.object(other).unwrap().geometry())
    else {
        panic!("expected unchanged line representation");
    };
    assert_eq!(after.start(), before.end());
    assert_eq!(after.end(), before.start());
    assert!(original.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    assert!(commands.object_has_direction_analysis(other));
    doc.clear_selection();
    let state = format!("{doc:?}");
    assert!(commands.execute(&mut doc, "ShowDir Flip").is_err());
    assert_eq!(format!("{doc:?}"), state);
}

#[test]
fn invalid_settings_and_membership_changes_are_atomic() {
    let (mut doc, commands, id) = setup();
    commands.execute(&mut doc, "ShowDir").unwrap();
    let initial = commands.direction_analysis_view(&doc).unwrap();
    let before = format!("{doc:?}");
    for input in [
        "ShowDir Color=1,2,999",
        "ShowDir U=No U=Yes",
        "ShowDir Direction=Maybe",
        "ShowDir Add Remove",
        "ShowDirOff U=No",
    ] {
        assert!(commands.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(
            commands.direction_analysis_view(&doc).unwrap().options,
            initial.options
        );
        assert!(commands.object_has_direction_analysis(id));
        assert_eq!(format!("{doc:?}"), before);
    }
    commands.execute(&mut doc, "ShowDir Remove").unwrap();
    assert!(!commands.object_has_direction_analysis(id));
    commands.execute(&mut doc, "ShowDir Add").unwrap();
    assert!(commands.object_has_direction_analysis(id));
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn small_trimmed_regions_keep_direction_arrows_when_the_interior_grid_misses() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let tolerance = Tolerance::DEFAULT;
    let surface =
        NurbsSurface::try_bilinear([p(0., 0.), p(100., 0.), p(100., 100.), p(0., 100.)]).unwrap();
    let cut = Polyline3::try_new(
        vec![p(1., 1.), p(2., 1.), p(2., 2.), p(1., 2.), p(1., 1.)],
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let pieces = Brep::try_split_rectangular_surface_face_with_closed_curve(
        surface,
        0.0..=1.0,
        0.0..=1.0,
        cut,
        false,
        tolerance,
    )
    .unwrap();
    let tiny = pieces
        .into_iter()
        .find(|b| {
            b.faces()[0]
                .contains_parameters(0.015, 0.015, tolerance)
                .unwrap()
        })
        .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(tiny)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "ShowDir").unwrap();
    let view = commands.direction_analysis_view(&doc).unwrap();
    assert_eq!(view.markers.len(), 1);
    let point = view.markers[0].point;
    assert!(
        (1. - 1e-8..=2. + 1e-8).contains(&point.x())
            && (1. - 1e-8..=2. + 1e-8).contains(&point.y())
    );
    assert!((view.markers[0].axes[2].unwrap().z() - 1.).abs() < 1e-12);
}

#[test]
fn persistent_controls_only_edit_types_admitted_to_persistent_analysis() {
    let (mut doc, commands, curve) = setup();
    commands.execute(&mut doc, "ShowDir").unwrap();
    commands
        .execute(&mut doc, "SubDPlane XCount=2 YCount=2 0,0,0 4,4,0")
        .unwrap();
    let id = doc.objects().last().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let before = format!("{doc:?}");
    commands.execute(&mut doc, "ShowDir Flip").unwrap();
    assert_eq!(format!("{doc:?}"), before);
    assert!(commands.object_has_direction_analysis(curve));
    assert!(!commands.object_has_direction_analysis(id));
}
