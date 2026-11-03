use super::*;
use crate::app::tests::subd_native as common;
use viboceros_oracle::subd_join::snapshot;
#[path = "../../../../crates/viboceros-oracle/tests/support/subd_extract.rs"]
mod support;

fn subd_pick(id: ObjectId, index: usize) -> ComponentPick {
    ComponentPick {
        object: id,
        index,
        kind: ComponentSelectionKind::SubDFace,
    }
}
#[test]
fn subd_extract_native_faces_options_selection_and_history_match_application() {
    let sources = support::sources();
    for (op, native) in ["subd_extract_command", "subd_extract_details"]
        .into_iter()
        .flat_map(support::cases_for)
    {
        for preselected in [false, true] {
            let mut app = test_app();
            let (doc, ids) = support::fixture(&native, &sources[op["scene"].as_str().unwrap()]);
            app.document = doc;
            let picks = native["input"]["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    subd_pick(
                        ids[p[0].as_u64().unwrap() as usize],
                        p[1].as_u64().unwrap() as usize,
                    )
                })
                .collect::<Vec<_>>();
            if preselected {
                for pick in &picks {
                    app.accept_component_click(ComponentClick {
                        picks: vec![*pick],
                        preselection: true,
                        modifiers: egui::Modifiers {
                            ctrl: true,
                            shift: true,
                            ..Default::default()
                        },
                    });
                }
            }
            enter(&mut app, "ExtractSrf");
            assert!(app.picking_extract_faces());
            enter(
                &mut app,
                &format!(
                    "Copy={} SubDAddCorners={} OutputLayer={}",
                    if op["copy"] == true { "Yes" } else { "No" },
                    if op["corners"] == true { "Yes" } else { "No" },
                    if native["input"]["current"] == true {
                        "Current"
                    } else {
                        "Input"
                    }
                ),
            );
            if !preselected {
                for pick in &picks {
                    app.accept_component_click(ComponentClick {
                        picks: vec![*pick],
                        preselection: false,
                        modifiers: Default::default(),
                    });
                }
                // Repeated picks are idempotent; Ctrl removes and a window can restore one.
                app.accept_component_click(ComponentClick {
                    picks: vec![picks[0]],
                    preselection: false,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                });
                app.accept_component_window(ComponentWindow {
                    picks: vec![picks[0]],
                    preselection: false,
                    modifiers: Default::default(),
                    crossing: true,
                    inverted: false,
                });
            }
            assert!(!app.document.can_undo());
            enter(&mut app, "");
            assert!(
                !app.picking_extract_faces(),
                "{op:?}: {:?}",
                app.command_log
            );
            let id = op["id"].as_str().unwrap();
            support::compare(
                &snapshot(&app.document, &ids),
                &support::expected(native["after_script"].clone()),
                id,
            );
            assert!(
                app.component_selection
                    .checked_picks(&app.document)
                    .unwrap()
                    .is_empty()
            );
            if op["scene"] == "all" {
                assert!(!app.document.can_undo());
                continue;
            }
            enter(&mut app, "Undo");
            support::compare(
                &snapshot(&app.document, &ids),
                &support::expected(native["undo"].clone()),
                id,
            );
            enter(&mut app, "Redo");
            support::compare(
                &snapshot(&app.document, &ids),
                &support::expected(native["redo"].clone()),
                id,
            );
        }
    }
}
#[test]
fn subd_extract_options_cancel_and_invalid_inputs_preserve_redo() {
    let mut app = test_app();
    let cases = support::cases_for("subd_extract_command");
    let (_, native) = &cases[0];
    let sources = support::sources();
    let (doc, ids) = support::fixture(native, &sources["single"]);
    app.document = doc;
    enter(&mut app, "Point 20,20,20");
    enter(&mut app, "Undo");
    enter(&mut app, "ExtractSrf SubDAddCorners=Yes");
    app.accept_component_click(ComponentClick {
        picks: vec![subd_pick(ids[0], 0)],
        preselection: false,
        modifiers: Default::default(),
    });
    enter(&mut app, "SubDAddCorners=Maybe");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtractSrf {
            subd_add_corners: true,
            ..
        })
    ));
    app.cancel_interactive_command(true);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "ExtractSrf");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtractSrf {
            subd_add_corners: true,
            ..
        })
    ));
    app.cancel_interactive_command(true);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}
