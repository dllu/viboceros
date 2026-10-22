use super::*;
use crate::app::tests::test_app;
use crate::viewport::{ComponentClick, ComponentPick};
use viboceros_geometry::SubD;
mod native;
mod vertex_seams;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
fn setup() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    let s = SubD::try_from_polygons(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(4., 0., 1.),
            p(0., 3., 0.),
            p(2., 3., 0.),
            p(4., 3., 1.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap();
    let id = app.document.add_geometry(Geometry::SubD(s)).unwrap();
    app.document.clear_history().unwrap();
    (app, id)
}
fn select(
    app: &mut VibocerosApp,
    id: ObjectId,
    kind: ComponentSelectionKind,
    index: usize,
    preselection: bool,
) {
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: id,
            kind,
            index,
        }],
        preselection,
        modifiers: if preselection {
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        },
    });
}
#[test]
fn preselection_commits_immediately_and_postselection_waits_for_enter() {
    for preselection in [false, true] {
        let (mut app, id) = setup();
        let original = app.document.object(id).unwrap().geometry().clone();
        if !preselection {
            enter(&mut app, "Crease");
            assert!(app.crease_prompt.is_some());
        }
        select(
            &mut app,
            id,
            ComponentSelectionKind::SubDVertex,
            0,
            preselection,
        );
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        enter(&mut app, if preselection { "Crease" } else { "" });
        assert!(app.crease_prompt.is_none(), "{:?}", app.command_log);
        assert_ne!(app.document.object(id).unwrap().geometry(), &original);
        assert_eq!(app.document.undo_label(), Some("Crease"));
        assert!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .is_empty()
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        enter(&mut app, "Redo");
        assert_ne!(app.document.object(id).unwrap().geometry(), &original);
    }
}
#[test]
fn cancellation_and_selnone_keep_geometry_history_and_redo_staged() {
    for cancel in ["Escape", "_Cancel", "-Cancel", "_-Cancel"] {
        let (mut app, id) = setup();
        enter(&mut app, "Point 9,9,9");
        enter(&mut app, "Undo");
        let redo = app.document.redo_label().map(str::to_owned);
        let original = app.document.object(id).unwrap().geometry().clone();
        enter(&mut app, "RemoveCrease");
        select(&mut app, id, ComponentSelectionKind::SubDVertex, 0, false);
        enter(&mut app, "SelNone");
        assert!(app.crease_prompt.is_some());
        assert!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .is_empty()
        );
        select(&mut app, id, ComponentSelectionKind::SubDVertex, 0, false);
        if cancel == "Escape" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, cancel);
        }
        assert!(app.crease_prompt.is_none());
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
}

#[test]
fn stale_geometry_and_tolerance_are_rejected_before_history_admission() {
    for changed_geometry in [false, true] {
        let (mut app, id) = setup();
        enter(&mut app, "Crease");
        select(&mut app, id, ComponentSelectionKind::SubDVertex, 0, false);
        if changed_geometry {
            app.document
                .replace_object_geometries([(id, Geometry::Point(p(7., 8., 9.)))])
                .unwrap();
        } else {
            app.commands
                .execute(&mut app.document, "Tolerance Absolute=0.02")
                .unwrap();
        }
        let before = app.document.object(id).unwrap().geometry().clone();
        let label = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "");
        assert!(app.crease_prompt.is_some());
        assert_eq!(app.document.object(id).unwrap().geometry(), &before);
        assert_eq!(app.document.undo_label(), label.as_deref());
        assert!(app.command_log.back().unwrap().contains("changed"));
    }
}
#[test]
fn no_op_subd_components_stay_selected_and_preserve_redo() {
    let (mut app, id) = setup();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    let index = s.edges().iter().position(|e| *e == [0, 1]).unwrap();
    select(&mut app, id, ComponentSelectionKind::SubDEdge, index, true);
    enter(&mut app, "Crease");
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn control_cage_vertex_picks_use_grips_and_keep_them_selected_after_commit() {
    let (mut app, id) = setup();
    app.document.enable_control_points([id]).unwrap();
    enter(&mut app, "Crease");
    select(&mut app, id, ComponentSelectionKind::SubDVertex, 0, false);
    assert_eq!(app.document.selected_control_points().count(), 1);
    enter(&mut app, "");
    assert!(app.crease_prompt.is_none());
    assert_eq!(app.document.selected_control_points().count(), 1);
    assert!(app.document.grip_mode(id).is_some());
}
