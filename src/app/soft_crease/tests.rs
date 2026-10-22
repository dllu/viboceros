use super::*;
use crate::app::tests::test_app;
use crate::viewport::{ComponentClick, ComponentPick};
use viboceros_geometry::SubD;
mod native;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command_input();
}
fn setup() -> (VibocerosApp, ObjectId, Vec<usize>) {
    let mut app = test_app();
    let subd = SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [0., 0., 2.],
            [4., 0., 2.],
            [4., 3., 2.],
            [0., 3., 2.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![
            vec![0, 3, 2, 1],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
            vec![4, 5, 6, 7],
        ],
    )
    .unwrap();
    let edges = [[0, 1], [1, 2], [2, 3]]
        .map(|[a, b]| {
            subd.edges()
                .iter()
                .position(|e| *e == [a, b] || *e == [b, a])
                .unwrap()
        })
        .to_vec();
    let id = app.document.add_geometry(Geometry::SubD(subd)).unwrap();
    app.document.clear_history().unwrap();
    (app, id, edges)
}
fn pick(app: &mut VibocerosApp, id: ObjectId, edges: &[usize], preselection: bool) {
    for &index in edges {
        app.accept_component_click(ComponentClick {
            picks: vec![ComponentPick {
                object: id,
                kind: ComponentSelectionKind::SubDEdge,
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
}

#[test]
fn soft_crease_preselection_and_postselection_preview_then_commit_one_undo_step() {
    for preselection in [true, false] {
        let (mut app, id, edges) = setup();
        let original = app.document.object(id).unwrap().geometry().clone();
        if preselection {
            pick(&mut app, id, &edges, true);
            enter(&mut app, "SubDCrease");
        } else {
            enter(&mut app, "SubDCrease");
            assert!(app.picking_soft_crease_edges());
            pick(&mut app, id, &edges, false);
            enter(&mut app, "");
        }
        let prompt = app.soft_crease_prompt.as_ref().unwrap();
        assert!(!prompt.selecting(), "{:?}", app.command_log);
        assert_eq!(prompt.labels.len(), 4);
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        app.pick_soft_crease_vertex(0, 0);
        enter(&mut app, "80");
        let prompt = app.soft_crease_prompt.as_ref().unwrap();
        assert_ne!(
            prompt.scene().unwrap().object(id).unwrap().geometry(),
            &original
        );
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        assert!(!app.document.can_undo());
        app.pick_soft_crease_vertex(0, 1);
        enter(&mut app, "20");
        enter(&mut app, "");
        assert!(app.soft_crease_prompt.is_none());
        assert_eq!(app.document.undo_label(), Some("SubDCrease"));
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
fn soft_crease_invalid_weights_retry_and_escape_discards_all_preview_edits() {
    for cancel in ["Escape", "Cancel", "_Cancel"] {
        let (mut app, id, edges) = setup();
        enter(&mut app, "Point 20,30,40");
        enter(&mut app, "Undo");
        let original = app.document.object(id).unwrap().geometry().clone();
        pick(&mut app, id, &edges, true);
        enter(&mut app, "SubDCrease");
        app.pick_soft_crease_vertex(0, 0);
        for invalid in ["-1", "101", "NaN"] {
            enter(&mut app, invalid);
            assert!(matches!(
                app.soft_crease_prompt.as_ref().unwrap().getter,
                Getter::Vertex(0, 0)
            ));
            assert!(app.command_log.back().unwrap().starts_with("Error:"));
        }
        enter(&mut app, "80");
        assert!(matches!(
            app.soft_crease_prompt.as_ref().unwrap().getter,
            Getter::Outer
        ));
        app.document.enable_control_points([id]).unwrap();
        app.document
            .select_control_points(
                [viboceros_document::ControlPointId {
                    object: id,
                    index: 0,
                }],
                SelectionMode::Replace,
            )
            .unwrap();
        if cancel == "Escape" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, cancel);
        }
        assert!(app.soft_crease_prompt.is_none());
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        assert!(app.document.can_redo());
        assert_eq!(app.document.selected_control_points().count(), 0);
        assert!(app.document.grip_mode(id).is_some());
    }
}

#[test]
fn soft_crease_chain_options_and_text_size_remain_staged() {
    let (mut app, id, edges) = setup();
    pick(&mut app, id, &edges, true);
    enter(&mut app, "SubDCrease");
    app.pick_soft_crease_vertex(0, 0);
    enter(&mut app, "ConstantWeightChain");
    enter(&mut app, "70");
    let prompt = app.soft_crease_prompt.as_ref().unwrap();
    assert!(prompt.labels.iter().all(|l| (l.weight - 70.).abs() < 1e-5));
    enter(&mut app, "LargerText");
    assert_eq!(app.soft_crease_prompt.as_ref().unwrap().text_size, 16.);
    enter(&mut app, "SmallerText");
    assert_eq!(app.soft_crease_prompt.as_ref().unwrap().text_size, 14.);
    app.pick_soft_crease_vertex(0, 1);
    enter(&mut app, "CreaseThisChain");
    let prompt = app.soft_crease_prompt.as_ref().unwrap();
    let Geometry::SubD(subd) = prompt.scene().unwrap().object(id).unwrap().geometry() else {
        panic!()
    };
    assert!(edges.iter().all(|&e| subd.edge_tags()[e] == 2));
    assert!(!app.document.can_undo());
    enter(&mut app, "ConstantWeight");
    enter(&mut app, "Crease");
    enter(&mut app, "");
    assert_eq!(app.document.undo_label(), Some("SubDCrease"));
}

#[test]
fn soft_crease_stale_preview_is_released_and_whole_objects_do_not_select_edges() {
    let (mut app, id, edges) = setup();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "SubDCrease");
    assert!(app.picking_soft_crease_edges());
    pick(&mut app, id, &edges, false);
    enter(&mut app, "");
    enter(&mut app, "80");
    app.commands
        .execute(&mut app.document, "Tolerance Absolute=0.02")
        .unwrap();
    let before = app.document.object(id).unwrap().geometry().clone();
    app.validate_soft_crease();
    assert!(app.soft_crease_prompt.is_none());
    assert_eq!(app.document.object(id).unwrap().geometry(), &before);
    assert_ne!(app.document.undo_label(), Some("SubDCrease"));
    enter(&mut app, "SubDCrease");
    assert!(app.picking_soft_crease_edges());
    enter(&mut app, "");
    assert!(app.soft_crease_prompt.is_none());
}
