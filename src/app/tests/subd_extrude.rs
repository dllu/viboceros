use super::*;
use crate::app::tests::subd_native as common;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind;
use viboceros_geometry::SubDExtrudeSelection;
use viboceros_oracle::subd_join::snapshot;
#[path = "../../../crates/viboceros-oracle/tests/support/subd_extrude.rs"]
mod support;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn extrude_subd_native_whole_and_component_preselection_and_command_first_history() {
    let sources = support::sources();
    for family in [
        "subd_extrude_command",
        "subd_extrude_extra",
        "subd_extrude_mixed",
    ] {
        for (op, native) in common::cases_for(family) {
            let target = op["target"].as_str().unwrap();
            let label = op["id"].as_str().unwrap();
            for preselect in [true, false] {
                let mut app = test_app();
                let (doc, id) = support::fixture(&sources[target.split('_').next().unwrap()]);
                app.document = doc;
                let mut picks = vec![];
                match support::selection(target) {
                    SubDExtrudeSelection::AllFaces => {
                        app.document
                            .select_objects_direct([id], SelectionMode::Replace)
                            .unwrap();
                    }
                    SubDExtrudeSelection::Components { faces, edges } => {
                        picks.extend(faces.into_iter().map(|index| ComponentPick {
                            object: id,
                            index,
                            kind: ComponentSelectionKind::SubDFace,
                        }));
                        picks.extend(edges.into_iter().map(|index| ComponentPick {
                            object: id,
                            index,
                            kind: ComponentSelectionKind::SubDEdge,
                        }));
                    }
                }
                if preselect {
                    for p in &picks {
                        app.accept_component_click(ComponentClick {
                            picks: vec![*p],
                            preselection: true,
                            modifiers: Default::default(),
                        });
                    }
                }
                enter(&mut app, "ExtrudeSubD");
                assert!(
                    app.extrude_subd_session.is_some(),
                    "{label}: {:?}",
                    app.command_log
                );
                if !preselect && !picks.is_empty() {
                    assert!(app.picking_extrude_subd());
                    app.accept_component_window(ComponentWindow {
                        picks: picks.clone(),
                        preselection: false,
                        modifiers: Default::default(),
                        crossing: false,
                        inverted: false,
                    });
                    // Command-first picks can be removed and restored without edits.
                    app.accept_component_click(ComponentClick {
                        picks: vec![picks[0]],
                        preselection: false,
                        modifiers: egui::Modifiers {
                            ctrl: true,
                            ..Default::default()
                        },
                    });
                    app.accept_component_click(ComponentClick {
                        picks: vec![picks[0]],
                        preselection: false,
                        modifiers: Default::default(),
                    });
                    enter(&mut app, "");
                }
                assert!(
                    !app.picking_extrude_subd(),
                    "{label}: {:?}",
                    app.command_log
                );
                assert!(!app.document.can_undo());
                enter(
                    &mut app,
                    support::options(&op)
                        .command()
                        .strip_prefix("ExtrudeSubD ")
                        .unwrap(),
                );
                assert!(
                    !app.extrude_subd_session
                        .as_ref()
                        .unwrap()
                        .edges()
                        .is_empty()
                );
                assert!(!app.document.can_undo());
                enter(&mut app, "");
                assert!(
                    app.extrude_subd_session.is_none(),
                    "{label}: {:?}",
                    app.command_log
                );
                assert!(
                    app.component_selection
                        .checked_picks(&app.document)
                        .unwrap()
                        .is_empty()
                );
                common::compare(
                    &snapshot(&app.document, &[id]),
                    &support::expected(&native["after"]),
                    label,
                );
                enter(&mut app, "SelNone");
                enter(&mut app, "Undo");
                common::compare(
                    &snapshot(&app.document, &[id]),
                    &support::expected(&native["undo"]),
                    label,
                );
                assert!(
                    app.component_selection
                        .checked_picks(&app.document)
                        .unwrap()
                        .is_empty()
                );
                enter(&mut app, "Redo");
                common::compare(
                    &snapshot(&app.document, &[id]),
                    &support::expected(&native["redo"]),
                    label,
                );
            }
        }
    }
}
#[test]
fn extrude_subd_preview_cancel_invalid_options_and_stale_sources_preserve_redo() {
    let sources = support::sources();
    let mut app = test_app();
    let (doc, id) = support::fixture(&sources["quad"]);
    app.document = doc;
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSubD 2 Basis=WCS Direction=X");
    assert!(app.extrude_subd_session.is_none());
    assert_eq!(app.document.undo_label(), Some("ExtrudeSubD"));
    enter(&mut app, "Undo");
    let before = snapshot(&app.document, &[id]);
    enter(&mut app, "ExtrudeSubD");
    enter(&mut app, "Distance=0");
    enter(&mut app, "Basis=UVN Direction=V");
    enter(&mut app, "Cancel");
    assert_eq!(snapshot(&app.document, &[id]), before);
    assert_eq!(app.document.redo_label(), Some("ExtrudeSubD"));
    enter(&mut app, "ExtrudeSubD");
    app.document.clear_selection();
    enter(&mut app, "");
    assert!(app.extrude_subd_session.is_some());
    assert!(app.command_log.iter().any(|s| s.contains("changed")));
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("ExtrudeSubD"));
    // Explicit command-line component indices work for selected objects.
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSubD 1 Faces=0 Basis=UVN Direction=N");
    assert_eq!(app.document.undo_label(), Some("ExtrudeSubD"));
}

mod viewport;

#[test]
fn extrude_subd_ambiguous_picks_resolve_before_numeric_distance_input() {
    let sources = support::sources();
    let mut app = test_app();
    let (doc, id) = support::fixture(&sources["quad"]);
    app.document = doc;
    enter(&mut app, "ExtrudeSubD");
    app.accept_component_click(ComponentClick {
        picks: [0, 1]
            .into_iter()
            .map(|index| ComponentPick {
                object: id,
                index,
                kind: ComponentSelectionKind::SubDEdge,
            })
            .collect(),
        preselection: false,
        modifiers: Default::default(),
    });
    assert!(app.component_selection.has_choices());
    enter(&mut app, "2");
    let picks = app
        .component_selection
        .checked_picks(&app.document)
        .unwrap();
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].index, 1);
    enter(&mut app, "");
    assert!(!app.picking_extrude_subd());
    assert!(!app.document.can_undo());
    enter(&mut app, "Cancel");
}
