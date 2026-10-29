use super::*;
use crate::app::tests::test_app;
use crate::viewport::ComponentClick;
#[path = "../../../crates/viboceros-command/tests/support/merge_faces.rs"]
mod support;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command_input();
}
fn picks(app: &VibocerosApp) -> Vec<Target> {
    app.component_selection
        .checked_picks(&app.document)
        .unwrap()
        .into_iter()
        .map(|p| Target {
            object: p.object,
            kind: p.kind,
            index: p.index,
        })
        .collect()
}
fn click(app: &mut VibocerosApp, target: Target, preselection: bool) {
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: target.object,
            kind: target.kind,
            index: target.index,
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
fn native_merge_faces_getter_results_and_selection_replay() {
    for row in support::rows() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let f = support::fixture(&row);
        assert_eq!(f.ids.len(), v["before"].as_array().unwrap().len());
        let mut app = test_app();
        app.document = f.document;
        app.commands.remember_merge_faces_results(f.results);
        let post = name.contains("-postselected-");
        if !post {
            for t in &f.targets {
                app.accept_component_click(ComponentClick {
                    picks: vec![ComponentPick {
                        object: t.object,
                        kind: t.kind,
                        index: t.index,
                    }],
                    preselection: true,
                    modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                });
            }
        }
        enter(&mut app, "_MergeFaces");
        if post {
            assert!(app.picking_merge_faces(), "{name}: {:?}", app.command_log);
            for t in &f.targets {
                app.accept_component_click(ComponentClick {
                    picks: vec![ComponentPick {
                        object: t.object,
                        kind: t.kind,
                        index: t.index,
                    }],
                    preselection: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            enter(&mut app, "");
        } else if v["command_result"] == "Cancel" {
            assert!(app.picking_merge_faces(), "{name}: {:?}", app.command_log);
            enter(&mut app, "");
        }
        assert!(!app.picking_merge_faces(), "{name}: {:?}", app.command_log);
        support::compare_phase(&app.document, &picks(&app), &row, "after", &f.ids);
        enter(&mut app, "Undo");
        support::compare_phase(&app.document, &picks(&app), &row, "undone", &f.ids);
        enter(&mut app, "Redo");
        support::compare_phase(&app.document, &picks(&app), &row, "redone", &f.ids);
    }
}
#[test]
fn results_preference_survives_cancel_and_is_isolated_between_apps() {
    let mut app = test_app();
    enter(&mut app, "MergeFaces");
    assert!(app.picking_merge_faces());
    enter(&mut app, "Results=_NotSelected");
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.merge_faces_results(), Results::NotSelected);
    assert!(!app.picking_merge_faces());
    assert!(app.document.undo_label().is_none());
    let independent = test_app();
    assert_eq!(
        independent.commands.merge_faces_results(),
        Results::Selected
    );
}

#[test]
fn separate_results_option_and_scripted_selectors_share_native_history_and_picks() {
    for script in [false, true] {
        let row = support::rows()
            .into_iter()
            .find(|r| r["id"] == "mesh-faces-NotSelected")
            .unwrap();
        let f = support::fixture(&row);
        let mut app = test_app();
        app.document = f.document;
        if script {
            app.document
                .select_objects_direct(
                    [f.ids["Input 0"]],
                    viboceros_document::SelectionMode::Replace,
                )
                .unwrap();
            enter(&mut app, "MergeFaces Faces=0,1 Results=NotSelected");
        } else {
            enter(&mut app, "MergeFaces");
            enter(&mut app, "_Results");
            enter(&mut app, "_NotSelected");
            assert!(app.picking_merge_faces());
            for t in f.targets {
                click(&mut app, t, false);
            }
            enter(&mut app, "");
        }
        assert_eq!(app.commands.merge_faces_results(), Results::NotSelected);
        support::compare_phase(&app.document, &picks(&app), &row, "after", &f.ids);
        enter(&mut app, "Undo");
        support::compare_phase(&app.document, &picks(&app), &row, "undone", &f.ids);
        enter(&mut app, "Redo");
        support::compare_phase(&app.document, &picks(&app), &row, "redone", &f.ids);
    }
    // An indexed script also publishes the Selected results to the viewport.
    let row = support::rows()
        .into_iter()
        .find(|r| r["id"] == "subd-faces-Selected")
        .unwrap();
    let f = support::fixture(&row);
    let mut app = test_app();
    app.document = f.document;
    app.document
        .select_objects_direct(
            [f.ids["Input 0"]],
            viboceros_document::SelectionMode::Replace,
        )
        .unwrap();
    enter(&mut app, "MergeFaces Faces=All");
    support::compare_phase(&app.document, &picks(&app), &row, "after", &f.ids);
    enter(&mut app, "Undo");
    enter(&mut app, "Redo");
    support::compare_phase(&app.document, &picks(&app), &row, "redone", &f.ids);
}

#[test]
fn stale_getter_picks_preserve_every_document_edit_and_remain_cancelable() {
    let row = support::rows()
        .into_iter()
        .find(|r| r["id"] == "mixed-owners-Selected")
        .unwrap();
    let f = support::fixture(&row);
    let mut app = test_app();
    app.document = f.document;
    enter(&mut app, "MergeFaces");
    for t in f.targets {
        click(&mut app, t, false);
    }
    app.document
        .set_objects_locked([f.ids["Input 1"]], true)
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.picking_merge_faces());
    assert!(app.command_log.iter().any(|s| s.contains("Error:")));
    enter(&mut app, "Cancel");
    assert!(!app.picking_merge_faces());
}

#[test]
fn consecutive_equal_history_labels_restore_only_their_own_result_components() {
    let row = support::rows()
        .into_iter()
        .find(|r| r["id"] == "mesh-reverse-selection-Selected")
        .unwrap();
    let f = support::fixture(&row);
    let mut app = test_app();
    app.document = f.document;
    let mut entries = Vec::new();
    for owner in [f.ids["Input 0"], f.ids["Input 1"]] {
        enter(&mut app, "SelNone");
        for &t in f.targets.iter().filter(|t| t.object == owner) {
            click(&mut app, t, true);
        }
        enter(&mut app, "MergeFaces");
        assert_eq!(app.document.undo_label(), Some("MergeFaces"));
        entries.push(app.document.undo_entry_id().unwrap());
        assert!(picks(&app).iter().all(|t| t.object == owner));
    }
    assert_ne!(entries[0], entries[1]);
    enter(&mut app, "Undo");
    assert!(picks(&app).is_empty());
    assert_eq!(app.document.redo_entry_id(), Some(entries[1]));
    enter(&mut app, "Undo");
    assert_eq!(app.document.redo_entry_id(), Some(entries[0]));
    enter(&mut app, "Redo");
    assert!(!picks(&app).is_empty());
    assert!(picks(&app).iter().all(|t| t.object == f.ids["Input 0"]));
    enter(&mut app, "SelNone");
    enter(&mut app, "Redo");
    assert!(picks(&app).iter().all(|t| t.object == f.ids["Input 1"]));
    assert!(!picks(&app).is_empty());
    // A new edit after Undo discards that redo's replay metadata too.
    enter(&mut app, "Undo");
    enter(&mut app, "Point 80,80,80");
    assert!(!app.document.has_history_entry(entries[1]));
    let before = format!("{:?}", app.document);
    enter(&mut app, "Redo");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(picks(&app).is_empty());
}
