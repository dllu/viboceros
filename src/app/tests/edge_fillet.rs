use super::*;
use crate::viewport::{ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind;
fn submit(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn box_pick(app: &mut VibocerosApp) -> ComponentPick {
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 10.]; 3],
        app.document.tolerance(),
    )
    .unwrap();
    ComponentPick {
        object: app.document.add_geometry(Geometry::Brep(brep)).unwrap(),
        kind: ComponentSelectionKind::BrepEdge,
        index: 0,
    }
}
#[test]
fn command_first_keeps_geometry_until_confirmation_and_escape_cancels() {
    let mut app = test_app();
    let pick = box_pick(&mut app);
    let original = app.document.object(pick.object).unwrap().geometry().clone();
    submit(&mut app, "FilletEdge Radius=1");
    assert!(app.edge_fillet_prompt.is_some());
    assert!(!app.component_preselection_available());
    app.accept_component_window(ComponentWindow {
        picks: vec![pick],
        modifiers: egui::Modifiers::NONE,
        preselection: false,
        crossing: false,
        inverted: false,
    });
    submit(&mut app, "Radius=0.5");
    assert_eq!(
        app.document.object(pick.object).unwrap().geometry(),
        &original
    );
    submit(&mut app, "Cancel");
    assert!(app.edge_fillet_prompt.is_none());
    assert_eq!(
        app.document.object(pick.object).unwrap().geometry(),
        &original
    );
}
#[cfg(feature = "native-smlib")]
#[test]
fn picked_edge_confirmation_creates_one_undoable_fillet() {
    let mut app = test_app();
    let pick = box_pick(&mut app);
    app.document.clear_history().unwrap();
    let before = app.document.object(pick.object).unwrap().geometry().clone();
    submit(&mut app, "FilletEdge 1");
    app.accept_component_window(ComponentWindow {
        picks: vec![pick],
        modifiers: egui::Modifiers::NONE,
        preselection: false,
        crossing: false,
        inverted: false,
    });
    submit(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none());
    let Geometry::Brep(result) = app.document.object(pick.object).unwrap().geometry() else {
        panic!()
    };
    assert!(result.is_solid());
    assert_eq!(result.faces().len(), 7);
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
    submit(&mut app, "Undo");
    assert_eq!(
        app.document.object(pick.object).unwrap().geometry(),
        &before
    );
}
