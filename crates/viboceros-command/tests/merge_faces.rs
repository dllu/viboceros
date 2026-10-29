#[path = "support/merge_faces.rs"]
mod support;
use viboceros_command::merge_faces::MergeSelection;
use viboceros_command::merge_faces::{Results, Target};
use viboceros_command::{CommandRegistry, ComponentSelectionKind};
use viboceros_document::SelectionMode;

fn scene(name: &str) -> support::Fixture {
    support::fixture(
        &support::rows()
            .into_iter()
            .find(|r| r["id"] == name)
            .unwrap(),
    )
}
#[test]
fn actual_native_merge_faces_options_owners_attributes_geometry_and_history() {
    for row in support::rows() {
        let value = &row["value"];
        let mut f = support::fixture(&row);
        assert_eq!(f.ids.len(), value["before"].as_array().unwrap().len());
        if value["command_result"] == "Cancel" {
            f.document.clear_selection();
        }
        let out = if f.targets.is_empty() {
            None
        } else {
            Some(
                MergeSelection::prepare(&f.document, f.targets.clone(), f.results)
                    .unwrap()
                    .commit(&mut f.document)
                    .unwrap(),
            )
        };
        support::compare_phase(
            &f.document,
            out.as_ref().map_or(&[], |o| &o.selected_components),
            &row,
            "after",
            &f.ids,
        );
        f.document.undo().unwrap();
        support::compare_phase(&f.document, &[], &row, "undone", &f.ids);
        f.document.redo().unwrap();
        support::compare_phase(
            &f.document,
            out.as_ref().map_or(&[], |o| &o.replay_components),
            &row,
            "redone",
            &f.ids,
        );
    }
}

#[test]
fn scripts_use_explicit_typed_selectors_and_retain_persistent_owner_ids() {
    let commands = CommandRegistry::with_builtins();
    assert!(commands.command_names().contains(&"MergeFaces"));
    for kind in ["mesh", "subd"] {
        for selector in ["_Faces=0,1", "Faces All", "Edges=All", "Vertices=All"] {
            let mut f = scene(&format!("{kind}-faces-Selected"));
            let id = f.ids["Input 0"];
            let geometry = f.document.object(id).unwrap().geometry().clone();
            let attributes = f.document.object(id).unwrap().attributes().clone();
            f.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            commands
                .execute(
                    &mut f.document,
                    &format!("_-MergeFaces {selector} Results=_NotSelected _Enter"),
                )
                .unwrap();
            assert_eq!(commands.merge_faces_results(), Results::NotSelected);
            assert_eq!(f.document.undo_label(), Some("MergeFaces"));
            assert!(!f.document.is_selected(id));
            assert_eq!(f.document.object(id).unwrap().attributes(), &attributes);
            f.document.undo().unwrap();
            assert_eq!(f.document.object(id).unwrap().geometry(), &geometry);
            f.document.redo().unwrap();
            assert_eq!(f.document.object(id).unwrap().id(), id);
        }
    }
}

#[test]
fn invalid_scripts_and_late_components_preserve_model_selection_and_redo() {
    let commands = CommandRegistry::with_builtins();
    let mut f = scene("mixed-owners-Selected");
    let owners = [f.ids["Input 0"], f.ids["Input 1"]];
    f.document
        .select_objects_direct(owners, SelectionMode::Replace)
        .unwrap();
    commands.execute(&mut f.document, "Point 99,99,99").unwrap();
    f.document.undo().unwrap();
    f.document
        .select_objects_direct(owners, SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", f.document);
    let redo = f.document.redo_entry_id();
    for input in [
        "MergeFaces",
        "MergeFaces Faces=0,999999",
        "MergeFaces Faces=-1",
        "MergeFaces Faces=0 Faces=1",
        "MergeFaces Results=Unknown Faces=All",
        "MergeFaces Results=Selected Results=NotSelected Faces=All",
        "MergeFaces Faces=All _Enter garbage",
        "MergeFaces Edges=999999",
    ] {
        assert!(commands.execute(&mut f.document, input).is_err(), "{input}");
        assert_eq!(format!("{:?}", f.document), before, "{input}");
        assert_eq!(f.document.redo_entry_id(), redo);
    }
    let mut targets = f.targets.clone();
    targets.push(Target {
        object: owners[1],
        kind: ComponentSelectionKind::SubDFace,
        index: usize::MAX,
    });
    assert!(MergeSelection::prepare(&f.document, targets, Results::Selected).is_err());
    assert_eq!(format!("{:?}", f.document), before);
    let unavailable = f.ids["Undo witness"];
    assert!(
        commands
            .execute(
                &mut f.document,
                &format!("MergeFaces Faces=All Object={unavailable}")
            )
            .is_err()
    );
    assert_eq!(format!("{:?}", f.document), before);
}

#[test]
fn prepared_sources_and_permissions_are_checked_again_before_commit() {
    for lock in [false, true] {
        let mut f = scene("mixed-owners-Selected");
        let plan =
            MergeSelection::prepare(&f.document, f.targets.clone(), Results::Selected).unwrap();
        let id = f.ids["Input 1"];
        if lock {
            f.document.set_objects_locked([id], true).unwrap();
        } else {
            f.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            CommandRegistry::with_builtins()
                .execute(&mut f.document, "Move 0,0,0 1,2,3")
                .unwrap();
        }
        let before = format!("{:?}", f.document);
        assert!(plan.commit(&mut f.document).is_err());
        assert_eq!(format!("{:?}", f.document), before);
    }
}

#[test]
fn noop_clears_input_selection_without_consuming_the_existing_redo_branch() {
    let mut f = scene("mesh-single-Selected");
    f.document.undo().unwrap(); // Remove the earlier witness; retain its redo.
    let redo = f.document.redo_entry_id();
    let out = MergeSelection::prepare(&f.document, f.targets, Results::Selected)
        .unwrap()
        .commit(&mut f.document)
        .unwrap();
    assert!(out.changed_objects.is_empty());
    assert!(out.selected_components.is_empty());
    assert!(out.replay_components.is_empty());
    assert_eq!(f.document.redo_entry_id(), redo);
    assert!(f.document.undo_entry_id().is_none());
    f.document.redo().unwrap();
    assert!(f.document.object(f.ids["Undo witness"]).is_some());
}

#[test]
fn command_audit_keeps_geometry_properties_and_stable_owner_order_strict() {
    for result in ["Selected", "NotSelected"] {
        let row = support::rows()
            .into_iter()
            .find(|r| r["id"] == format!("mesh-reverse-selection-{result}"))
            .unwrap();
        let mut f = support::fixture(&row);
        let out = MergeSelection::prepare(&f.document, f.targets, f.results)
            .unwrap()
            .commit(&mut f.document)
            .unwrap();
        let audit = |row: &serde_json::Value| {
            support::compare_phase(&f.document, &out.selected_components, row, "after", &f.ids)
        };
        audit(&row);
        for field in ["coordinate", "color", "layer", "identity", "witness-order"] {
            let mut bad = row.clone();
            match field {
                "coordinate" => {
                    bad["value"]["after"][1]["geometry"]["vertices"][0][0] = serde_json::json!(123.)
                }
                "color" => bad["value"]["after"][1]["color"][0] = serde_json::json!(99),
                "layer" => bad["value"]["after"][1]["layer"] = serde_json::json!("Other"),
                "identity" => {
                    bad["value"]["after"][1]["object_id"] =
                        serde_json::json!("00000000-0000-4000-8000-000000000000")
                }
                "witness-order" => bad["value"]["after"].as_array_mut().unwrap().swap(0, 1),
                _ => unreachable!(),
            }
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| audit(&bad))).is_err(),
                "{result} {field}"
            );
        }
        let mut reordered = row.clone();
        reordered["value"]["after"]
            .as_array_mut()
            .unwrap()
            .swap(1, 2);
        let check = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| audit(&reordered)));
        assert_eq!(check.is_ok(), result == "NotSelected");
        f.document.undo().unwrap();
        let mut bad = row.clone();
        bad["value"]["undone"].as_array_mut().unwrap().swap(0, 1);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| support::compare_phase(
                &f.document,
                &[],
                &bad,
                "undone",
                &f.ids
            )))
            .is_err()
        );
    }
}
