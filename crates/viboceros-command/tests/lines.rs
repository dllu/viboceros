#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/lines.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::CommandRegistry;
use viboceros_document::Document;

#[test]
fn adjoining_line_commands_match_native_segments_domains_metadata_history_and_archives() {
    let names = [
        "open_two",
        "open_four",
        "closed_three",
        "closed_reversed",
        "nonplanar",
        "translated",
        "spatial_plane",
        "short_segments",
        "coarse_tolerance",
    ];
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            if !names.contains(&id) {
                continue;
            }
            let v = &row["value"];
            let source = &sources[id];
            let mut doc = inputs::setup(v, source);
            let registry = CommandRegistry::with_builtins();
            let mut text = "Lines".to_owned();
            for point in source["actions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p.is_array())
            {
                text.push(' ');
                text.push_str(
                    &point
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| n.as_f64().unwrap().to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            if source["actions"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("Close"))
            {
                text.push_str(" Close");
            }
            registry.execute(&mut doc, &text).unwrap();
            let mut error = replay::check(&doc, &v["after"], false);
            inputs::check_types(&doc, &v["after"]);
            let selected = doc.selected_object_ids().collect::<Vec<_>>();
            registry
                .execute(&mut doc, "SelLast DeselectOthersBeforeSelect=No")
                .unwrap();
            error = error.max(replay::check(&doc, &v["last_selection"], false));
            doc.set_transient_selection(selected).unwrap();
            doc.undo().unwrap();
            error = error.max(replay::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(replay::check(&doc, &v["redo"], false));
            let path = directory.path().join("lines.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            registry
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(replay::check(&restored, &v["after"], true));
            inputs::check_types(&restored, &v["after"]);
            println!(
                "LINES_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            assert!(error <= 1e-6);
            count += 1;
        }
    }
    assert_eq!(count, names.len());
}

#[test]
fn invalid_complete_lines_commands_preserve_geometry_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for text in [
        "Lines",
        "Lines 0,0,0",
        "Lines 0,0,0 4,0,0 4,0,0",
        "Lines 0,0,0 4,0,0 Close",
        "Lines 0,0,0 4,0,0 4,3,0 Close Extra",
        "Lines -1e308,0,0 1e308,0,0",
    ] {
        assert!(registry.execute(&mut doc, text).is_err(), "{text}");
        assert_eq!(format!("{doc:?}"), before, "{text}");
    }
    assert!(doc.can_redo());
}
