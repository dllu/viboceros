use super::*;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut a = test_app();
    for x in ["SrfPt 0,0,0 4,0,0 4,6,2 0,6,0", "Point 2,3,0.5"] {
        enter(&mut a, x);
    }
    let ids = a.document.objects().map(|o| o.id()).collect();
    (a, ids)
}
#[test]
fn create_uv_surface_pick_then_optional_point_creates_selected_flat_output() {
    let (mut a, ids) = setup();
    a.document.clear_selection();
    enter(&mut a, "CreateUVCrv");
    a.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(
        a.intersection_prompt.as_ref().unwrap().name(),
        "CreateUVCrv"
    );
    a.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    enter(&mut a, "");
    assert!(a.intersection_prompt.is_none());
    assert_eq!(a.document.objects().len(), 4);
    assert_eq!(a.document.selected_object_count(), 3);
    enter(&mut a, "Undo");
    assert_eq!(a.document.objects().len(), 2);
    enter(&mut a, "Redo");
    assert_eq!(a.document.selected_object_count(), 2);
}
#[test]
fn preselection_and_empty_extra_selection_produce_rectangle_under_rotated_cplane() {
    let (mut a, ids) = setup();
    enter(&mut a, "CPlane World Front");
    a.document.select_command_results([ids[0]]).unwrap();
    enter(&mut a, "_CreateUVCrv");
    enter(&mut a, "");
    assert_eq!(a.document.objects().len(), 3);
    assert_eq!(a.document.selected_object_count(), 1);
}
#[test]
fn create_uv_cancel_restores_original_selection_without_editing() {
    let (mut a, ids) = setup();
    a.document.select_command_results([ids[0]]).unwrap();
    let history = a.document.undo_label().map(str::to_owned);
    enter(&mut a, "CreateUVCrv");
    a.cancel_interactive_command(true);
    assert_eq!(
        a.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..1]
    );
    assert_eq!(a.document.undo_label(), history.as_deref());
}
