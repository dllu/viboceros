#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_construction.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::Document;

#[test]
fn line_constructions_match_native_geometry_domains_metadata_history_and_archives() {
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            if inputs::DIAGNOSTICS.contains(&id)
                || v["success"] != true
                || id == "repeated_point"
                || id == "both_repeat"
                || sources[id]["macro"].is_string()
            {
                continue;
            }
            let mut doc = inputs::setup(v);
            let parents = doc.objects().cloned().collect::<Vec<_>>();
            let text = format!(
                "Line {}",
                sources[id]["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| inputs::token(a, false))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            registry
                .execute_in_context(
                    &mut doc,
                    &text,
                    CommandContext {
                        construction_plane: native::frame(v),
                    },
                )
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            let comparison = replay::compare(&doc, &v["after"], false);
            assert_eq!(
                comparison.native_valid,
                !inputs::NATIVE_INVALID.contains(&id)
            );
            let check = |d: &Document, target: &serde_json::Value, archive| {
                if comparison.native_valid {
                    replay::check(d, target, archive)
                } else {
                    replay::compare(d, target, archive).error
                }
            };
            let mut error = comparison.error;
            assert_eq!(doc.undo_label(), Some("Line"));
            doc.undo().unwrap();
            error = error.max(check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(check(&doc, &v["redo"], false));
            let mut archives = 0;
            if comparison.native_valid {
                let path = directory.path().join("line.3dm");
                registry
                    .execute(&mut doc, &format!("SaveAs {}", path.display()))
                    .unwrap();
                let mut restored = Document::new(doc.tolerance());
                registry
                    .execute(&mut restored, &format!("Open3dm {}", path.display()))
                    .unwrap();
                error = error.max(replay::check(&restored, &v["after"], true));
                archives = 1;
            }
            for parent in parents {
                let now = doc.object(parent.id()).unwrap();
                assert_eq!(now, &parent);
                assert!(
                    parent
                        .geometry_snapshot()
                        .shares_storage_with(now.geometry_snapshot())
                );
            }
            println!(
                "LINE_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"native_valid":comparison.native_valid,"archives":archives})
            );
            assert!(
                error <= 1e-6,
                "{id}: {error:e}; {:?}; native {}",
                doc.objects().map(|o| o.geometry()).collect::<Vec<_>>(),
                v["after"]
            );
            count += 1;
        }
    }
    assert_eq!(count, 40);
}

#[test]
fn incomplete_and_degenerate_line_modes_preserve_document_and_redo() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Point 9,9,9").unwrap();
    d.undo().unwrap();
    let before = format!("{d:?}");
    for text in [
        "Line",
        "Line BothSides 1,2,3",
        "Line Vertical 1,2,3 5",
        "Line FourPoint 0,0,0 0,0,0 1,2,3 4,5,6",
        "Line Bisector 0,0,0 4,0,0 -3,0,0 4,5,0",
        "Line Angled 1,2,3 5,2,3 30",
        "Line 0,0,0 0,0,0",
        "Line 0,0,0 4,0,0 Extra",
        "Line -1e308,0,0 1e308,0,0",
    ] {
        assert!(r.execute(&mut d, text).is_err(), "{text}");
        assert_eq!(format!("{d:?}"), before, "{text}");
    }
    assert!(d.can_redo());
}
