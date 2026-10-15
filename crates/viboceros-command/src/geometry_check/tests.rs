use super::*;
use viboceros_geometry::MeshFace;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn bad_mesh() -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_from_face_records(
            vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 0., 0.)],
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap(),
    )
}
#[test]
fn check_reports_native_errors_and_preserves_geometry_selection_and_redo() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let good = d.add_geometry(Geometry::Point(p(1., 2., 3.))).unwrap();
    let bad = d.add_geometry(bad_mesh()).unwrap();
    d.add_geometry(Geometry::Point(p(9., 9., 9.))).unwrap();
    d.undo().unwrap();
    d.select_objects_direct([good, bad], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    let result = r.execute(&mut d, "Check").unwrap();
    assert!(
        result.starts_with("Check: 1 valid, 1 invalid, 0 not checked"),
        "{result}"
    );
    assert!(result.contains(&format!("{bad}: invalid")));
    assert!(result.contains("ON_Mesh"));
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
    assert!(r.execute(&mut d, "Check Everything").is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[test]
fn bad_selection_is_additive_direct_and_respects_object_and_layer_restrictions() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let peer = d.add_geometry(Geometry::Point(p(2., 3., 4.))).unwrap();
    let previous = d.add_geometry(Geometry::Point(p(3., 4., 5.))).unwrap();
    let bad = d.add_geometry(bad_mesh()).unwrap();
    d.add_group(None, [peer, bad]).unwrap();
    let hidden = d
        .add_geometry_with_attributes(
            bad_mesh(),
            ObjectAttributes::on_layer(d.current_layer_id()).with_visibility(false),
        )
        .unwrap();
    let locked = d
        .add_geometry_with_attributes(
            bad_mesh(),
            ObjectAttributes::on_layer(d.current_layer_id()).with_locked(true),
        )
        .unwrap();
    let hidden_layer = d.add_layer("hidden", ColorRgb::BLACK).unwrap();
    let locked_layer = d.add_layer("locked", ColorRgb::BLACK).unwrap();
    let hidden_on_layer = d
        .add_geometry_with_attributes(bad_mesh(), ObjectAttributes::on_layer(hidden_layer))
        .unwrap();
    let locked_on_layer = d
        .add_geometry_with_attributes(bad_mesh(), ObjectAttributes::on_layer(locked_layer))
        .unwrap();
    d.set_layer_visibility(hidden_layer, false).unwrap();
    d.set_layer_locked(locked_layer, true).unwrap();
    d.select_objects_direct([previous], SelectionMode::Replace)
        .unwrap();
    let objects = d.objects().cloned().collect::<Vec<_>>();
    let undo = d.undo_label().map(str::to_owned);
    let result = r.execute(&mut d, "SelBadObjects").unwrap();
    assert_eq!(result, "Selected 1 bad object(s); 0 object(s) not checked");
    assert!(d.is_selected(bad));
    assert!(d.is_selected(previous));
    for id in [peer, hidden, locked, hidden_on_layer, locked_on_layer] {
        assert!(!d.is_selected(id));
    }
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(d.undo_label(), undo.as_deref());
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "SelBadObjects Extra").is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[test]
fn structural_checks_do_not_misclassify_collinear_faces_and_noop_preserves_redo() {
    let mut d = Document::default();
    let mesh = TriangleMesh::try_from_face_records(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(2., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let id = d.add_geometry(Geometry::Mesh(mesh)).unwrap();
    d.add_geometry(Geometry::Point(p(9., 9., 9.))).unwrap();
    d.undo().unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    let r = CommandRegistry::with_builtins();
    assert!(
        r.execute(&mut d, "Check")
            .unwrap()
            .starts_with("Check: 1 valid, 0 invalid, 0 not checked")
    );
    assert_eq!(
        r.execute(&mut d, "SelBadObjects").unwrap(),
        "Selected 0 bad object(s); 0 object(s) not checked"
    );
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
}
#[test]
fn check_has_a_readonly_any_object_getter_and_requires_a_selection() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let prompt = r.object_selection_prompt("Check").unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Any);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::ConfirmAfterSelection
    );
    assert!(r.execute(&mut d, "Check").is_err());
}

#[test]
fn unrepresentable_native_copy_is_reported_separately_and_never_selected_as_bad() {
    let mut d = Document::default();
    let curve = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1e-300).unwrap(),
            WeightedPoint3::try_new(p(1e300, 0., 0.), 1e300).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let id = d.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    let r = CommandRegistry::with_builtins();
    let text = r.execute(&mut d, "Check").unwrap();
    assert!(
        text.starts_with("Check: 0 valid, 0 invalid, 1 not checked"),
        "{text}"
    );
    assert_eq!(
        r.execute(&mut d, "SelBadObjects").unwrap(),
        "Selected 0 bad object(s); 1 object(s) not checked"
    );
    assert_eq!(format!("{d:?}"), before);
}
#[test]
fn native_instance_reference_check_does_not_expand_bad_definition_members() {
    let mut d = Document::default();
    let r = CommandRegistry::with_builtins();
    let source = d.add_geometry(bad_mesh()).unwrap();
    d.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "Block 0,0,0 bad_member").unwrap();
    let root = d.objects().last().unwrap().id();
    d.select_objects_direct([root], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        d.object(root).unwrap().geometry(),
        Geometry::BlockInstance(_)
    ));
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "Check")
            .unwrap()
            .starts_with("Check: 1 valid, 0 invalid, 0 not checked")
    );
    assert_eq!(
        r.execute(&mut d, "SelBadObjects").unwrap(),
        "Selected 0 bad object(s); 0 object(s) not checked"
    );
    assert_eq!(format!("{d:?}"), before);
}
