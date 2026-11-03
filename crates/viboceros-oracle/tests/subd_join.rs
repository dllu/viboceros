use viboceros_command::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_oracle::subd_join::{Fixture, expected, snapshot};
#[path = "support/subd_join.rs"]
mod support;
#[test]
fn subd_join_commands_match_native_selection_attributes_history_and_geometry() {
    replay("subd_join_history", true);
}
#[test]
fn subd_join_original_command_completions_match_without_macro_cleanup_projection() {
    replay("subd_join_command", false);
}

#[test]
fn subd_join_outputs_and_soft_seams_roundtrip_native_3dm() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("joined.3dm");
    for (op, native) in support::cases_for("subd_join_history") {
        if !native["success"].as_bool().unwrap() {
            continue;
        }
        let fixture: Fixture = serde_json::from_value(native["input"].clone()).unwrap();
        let (mut doc, ids) = fixture.document();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let command = format!(
            "{} SubDJoinedEdges={}",
            op["command"].as_str().unwrap(),
            if fixture.crease { "Crease" } else { "Smooth" }
        );
        let registry = CommandRegistry::with_builtins();
        if op["preselect"].as_bool().unwrap() {
            registry.execute(&mut doc, &command).unwrap();
        } else {
            registry
                .execute_postselected(&mut doc, &command, Default::default())
                .unwrap();
        }
        let soft = doc
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .find_map(|o| {
                let viboceros_document::Geometry::SubD(s) = o.geometry() else {
                    unreachable!()
                };
                s.edge_face_counts()
                    .iter()
                    .enumerate()
                    .find(|&(i, &n)| n == 2 && s.edge_tags()[i] != 2)
                    .map(|(edge, _)| {
                        (
                            o.id(),
                            s.try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
                                .unwrap()
                                .0,
                        )
                    })
            });
        if let Some((id, subd)) = soft {
            doc.replace_object_geometries([(id, viboceros_document::Geometry::SubD(subd))])
                .unwrap();
        }
        doc.clear_selection();
        let before = snapshot(&doc, &ids);
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let (restored, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        support::compare(
            &snapshot(&restored, &ids),
            &before,
            op["id"].as_str().unwrap(),
        );
        for (a, b) in doc.objects().zip(restored.objects()) {
            let (viboceros_document::Geometry::SubD(a), viboceros_document::Geometry::SubD(b)) =
                (a.geometry(), b.geometry())
            else {
                unreachable!()
            };
            assert_eq!(a.edge_sharpness(), b.edge_sharpness());
        }
    }
}
fn replay(family: &str, history: bool) {
    for (op, native) in support::cases_for(family) {
        let fixture: Fixture = serde_json::from_value(native["input"].clone()).unwrap();
        let (mut doc, ids) = fixture.document();
        let registry = CommandRegistry::with_builtins();
        let pre = op["preselect"].as_bool().unwrap();
        let id = op["id"].as_str().unwrap();
        if pre {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        support::compare(&snapshot(&doc, &ids), &expected(&native["before"]), id);
        if !pre {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        let command = format!(
            "{} SubDJoinedEdges={}",
            op["command"].as_str().unwrap(),
            if fixture.crease { "Crease" } else { "Smooth" }
        );
        let result = if pre {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        };
        assert_eq!(
            result.is_ok(),
            native["success"].as_bool().unwrap(),
            "{id}: {result:?}"
        );
        support::compare(&snapshot(&doc, &ids), &expected(&native["after"]), id);
        if !history {
            continue;
        }
        // Rhino's RunScript releases command-first picks after EndCommand.
        // Reproduce that explicit driver phase, then compare its full snapshot.
        if !pre {
            doc.clear_selection();
        }
        support::compare(
            &snapshot(&doc, &ids),
            &expected(&native["after_script"]),
            &format!("{id} script return"),
        );
        let _ = doc.undo();
        support::compare(
            &snapshot(&doc, &ids),
            &expected(&native["undo"]),
            &format!("{id} undo"),
        );
        let _ = doc.redo();
        support::compare(
            &snapshot(&doc, &ids),
            &expected(&native["redo"]),
            &format!("{id} redo"),
        );
    }
}
