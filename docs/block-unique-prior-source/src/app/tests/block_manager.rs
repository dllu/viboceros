use super::*;
use crate::app::block_manager::Action;

fn part(app: &mut VibocerosApp, name: &str, x: f64) -> viboceros_document::BlockDefinitionId {
    let id = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
        .unwrap();
    app.document
        .create_block_from_objects(name, Point3::try_new(0., 0., 0.).unwrap(), [id])
        .unwrap()
        .0
}

#[test]
fn opening_block_manager_preserves_the_pending_points_transaction() {
    let mut app = test_app();
    app.command_input = "Points".into();
    app.run_command();
    app.accept_drafting_point(Point3::try_new(1., 2., 3.).unwrap());
    let before = format!("{:?}", app.document);
    let active = app.active_command;
    app.command_input = "BlockManager".into();
    app.run_command();
    assert!(app.block_manager.open);
    assert_eq!(app.active_command, active);
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn manager_selection_is_direct_and_rename_delete_replay_through_model_history() {
    let mut app = test_app();
    let id = part(&mut app, "part", 1.);
    let root = app.document.objects().next().unwrap().id();
    let peer = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()))
        .unwrap();
    app.document.add_group(None, [root, peer]).unwrap();
    app.apply_block_manager_action(Action::Select(id));
    assert!(app.document.is_selected(root));
    assert!(!app.document.is_selected(peer));
    app.apply_block_manager_action(Action::Rename(id, "Part A".into()));
    assert_eq!(app.document.block_definition(id).unwrap().name(), "Part A");
    assert_eq!(app.document.undo_label(), Some("Rename block definition"));
    app.apply_block_manager_action(Action::Delete(id));
    assert!(app.document.block_definition(id).is_none());
    assert!(app.document.object(peer).is_some());
    app.document.undo().unwrap();
    assert_eq!(app.document.block_definition(id).unwrap().name(), "Part A");
    assert!(app.document.object(root).is_some());
}

#[test]
fn block_manager_renders_a_definition_table_with_natural_name_order() {
    let mut app = test_app();
    part(&mut app, "Block10", 10.);
    part(&mut app, "Block2", 2.);
    app.block_manager.open = true;
    let ctx = egui::Context::default();
    let mut output = None;
    for _ in 0..2 {
        output = Some(ctx.run_ui(egui::RawInput::default(), |ui| {
            app.show_block_manager(ui.ctx())
        }));
    }
    let labels = output
        .unwrap()
        .shapes
        .into_iter()
        .filter_map(|s| match s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(labels.iter().any(|t| t == "Block definitions"));
    let two = labels.iter().position(|t| t == "Block2").unwrap();
    let ten = labels.iter().position(|t| t == "Block10").unwrap();
    assert!(two < ten);
}
