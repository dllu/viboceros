use super::*;
use viboceros_document::{ControlPointId, GripMode};
use viboceros_geometry::NurbsCurve;

fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn curve(x: f64) -> Geometry {
    Geometry::NurbsCurve(
        NurbsCurve::try_clamped_uniform(2, vec![p(x, 0.), p(x + 1., 2.), p(x + 2., 0.)]).unwrap(),
    )
}
pub(super) fn setup() -> (Document, [ObjectId; 3]) {
    let mut doc = Document::default();
    let control = doc.add_geometry(curve(0.)).unwrap();
    let edit = doc.add_geometry(curve(10.)).unwrap();
    let point = doc.add_geometry(Geometry::Point(p(20., 0.))).unwrap();
    doc.enable_control_points([control]).unwrap();
    doc.enable_edit_points([edit]).unwrap();
    (doc, [control, edit, point])
}
fn selected(doc: &Document) -> Vec<ControlPointId> {
    doc.selected_control_points().map(|(id, _)| id).collect()
}

#[test]
fn selpt_adds_both_grip_modes_and_point_objects_without_selecting_parents_or_group_peers() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, [control, edit, point]) = setup();
    doc.add_group(None, [control, point]).unwrap();
    registry.execute(&mut doc, "SelPt").unwrap();
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [point]);
    assert_eq!(selected(&doc).len(), 6);
    assert_eq!(doc.grip_mode(control), Some(GripMode::Control));
    assert_eq!(doc.grip_mode(edit), Some(GripMode::Edit));
    registry.execute(&mut doc, "SelPt").unwrap();
    assert_eq!(selected(&doc).len(), 6);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [point]);
}

#[test]
fn invertpt_is_an_involution_on_visible_grips_and_preserves_object_picks_and_geometry() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, [control, edit, point]) = setup();
    let picks = [
        ControlPointId {
            object: control,
            index: 0,
        },
        ControlPointId {
            object: edit,
            index: 2,
        },
    ];
    doc.select_control_points(picks, SelectionMode::Add)
        .unwrap();
    doc.select_objects_direct([control, point], SelectionMode::Add)
        .unwrap();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let object_picks = doc.selected_object_ids().collect::<Vec<_>>();
    let original = selected(&doc);
    registry.execute(&mut doc, "InvertPt").unwrap();
    let inverted = selected(&doc);
    assert_eq!(inverted.len(), 4);
    assert!(inverted.iter().all(|p| !original.contains(p)));
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), object_picks);
    registry.execute(&mut doc, "InvertPt").unwrap();
    assert_eq!(selected(&doc), original);
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), object_picks);
}

#[test]
fn selectors_skip_hidden_locked_and_disabled_displays_and_preserve_redo_on_invalid_input() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, [control, edit, point]) = setup();
    doc.set_objects_visibility([control], false).unwrap();
    doc.set_objects_locked([edit], true).unwrap();
    doc.set_objects_visibility([point], false).unwrap();
    registry.execute(&mut doc, "Point 30,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    for command in ["SelPt", "InvertPt"] {
        registry.execute(&mut doc, command).unwrap();
        assert_eq!(format!("{doc:?}"), before);
        assert!(
            registry
                .execute(&mut doc, &format!("{command} extra"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 4);
    doc.disable_control_points();
    registry.execute(&mut doc, "InvertPt").unwrap();
    assert!(selected(&doc).is_empty());
}

#[test]
fn getter_filters_and_strict_input_parsing_do_not_admit_point_objects_or_change_model_history() {
    let (mut doc, [_, _, point]) = setup();
    doc.add_geometry(Geometry::Point(p(40., 0.))).unwrap();
    doc.undo().unwrap();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    let action = PointSelectionAction::from_input("_-sElPt")
        .unwrap()
        .unwrap();
    action.apply(&mut doc, |_| false).unwrap();
    assert!(!doc.is_selected(point));
    assert_eq!(selected(&doc).len(), 6);
    assert_eq!(doc.undo_label(), undo.as_deref());
    assert_eq!(doc.redo_label(), redo.as_deref());
    assert!(PointSelectionAction::from_input("InvertPt invalid").is_err());
    assert_eq!(
        PointSelectionAction::from_input("SelPtCloud").unwrap(),
        None
    );
    assert_eq!(PointSelectionAction::from_input("").unwrap(), None);
}

#[test]
fn coincident_mesh_grips_invert_by_identity_instead_of_location() {
    let mut doc = Document::default();
    let mesh = viboceros_geometry::TriangleMesh::try_new(
        vec![p(0., 0.), p(1., 0.), p(0., 1.), p(0., 0.)],
        vec![[0, 1, 2], [3, 2, 1]],
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.enable_control_points([id]).unwrap();
    doc.select_control_points(
        [ControlPointId {
            object: id,
            index: 0,
        }],
        SelectionMode::Add,
    )
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "InvertPt").unwrap();
    assert_eq!(
        selected(&doc).iter().map(|p| p.index).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    registry.execute(&mut doc, "SelPt").unwrap();
    assert_eq!(selected(&doc).len(), 4);
    assert!(!doc.is_selected(id));
}

#[test]
fn hide_show_validate_before_mutating_and_preserve_ordinary_selection_geometry_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, [control, edit, point]) = setup();
    let marker = doc.add_geometry(Geometry::Point(p(30., 0.))).unwrap();
    doc.undo().unwrap();
    doc.select_objects_direct([control, point], SelectionMode::Add)
        .unwrap();
    for id in [control, edit] {
        doc.select_control_points(
            [ControlPointId {
                object: id,
                index: 1,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    }
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let before = format!("{doc:?}");
    for input in ["HidePt extra", "ShowPt extra"] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    assert_eq!(
        registry.execute(&mut doc, "_-hIdEpT").unwrap(),
        "Hid 2 grip(s)"
    );
    assert_eq!(doc.control_points().count(), 4);
    assert!(selected(&doc).is_empty());
    assert!(registry.execute(&mut doc, "HidePt").is_err());
    assert!(doc.is_selected(control) && doc.is_selected(point));
    assert_eq!(
        registry.execute(&mut doc, "ShowPt").unwrap(),
        "Showed 2 grip(s)"
    );
    assert_eq!(
        registry.execute(&mut doc, "ShowPt").unwrap(),
        "Showed 0 grip(s)"
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert!(doc.object(marker).is_some());
}
