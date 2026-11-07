#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_pick_context.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
#[path = "support/line_tangent.rs"]
#[allow(dead_code)]
mod tangent_inputs;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::Document;
#[test]
fn projected_curve_picks_and_tangent_lines_match_native_geometry_selection_history_and_archives() {
    let sources = inputs::sources();
    let r = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            if v["success"] != true || inputs::excluded(id) {
                continue;
            }
            let mut doc = inputs::setup(v, &sources[id]);
            let parents = doc.objects().cloned().collect::<Vec<_>>();
            let command = inputs::command(&doc, v, &sources[id]);
            r.execute_in_context(
                &mut doc,
                &command,
                CommandContext {
                    construction_plane: native::frame(v),
                },
            )
            .unwrap_or_else(|e| panic!("{id}: {e}"));
            let mut error = replay::check(&doc, &v["after"], false);
            assert_eq!(doc.undo_label(), Some("Line"));
            doc.undo().unwrap();
            error = error.max(replay::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(replay::check(&doc, &v["redo"], false));
            let path = directory.path().join("tangent.3dm");
            r.execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            r.execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(replay::check(&restored, &v["after"], true));
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
                "LINE_PICK_CONTEXT_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            assert!(
                error <= 1e-6,
                "{id}: {error:e}; {:?}",
                doc.objects().map(|o| o.geometry()).collect::<Vec<_>>()
            );
            count += 1;
        }
    }
    assert_eq!(count, 14);
}

#[test]
fn public_pick_context_points_match_independent_projection_and_native_fixed_contacts() {
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            let clicks = v["input_trace"][0]["clicks"].as_array().unwrap();
            let click = clicks.iter().find(|c| c["curve"].is_number()).unwrap();
            let i = click["action"].as_u64().unwrap() as usize;
            let actual = inputs::projected_point(v, i);
            let expected = native::point(&click["pick_context"]["point"]);
            let points = &v["after"].as_array().unwrap().last().unwrap()["curve"]["control_points"];
            let start = native::point(&points[0]["point"]);
            let end = native::point(&points[1]["point"]);
            let both = v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "BothSides");
            let contact = if both {
                start
                    .translated(start.vector_to(end).unwrap().scaled(0.5).unwrap())
                    .unwrap()
            } else {
                start
            };
            let error = actual
                .distance_to(expected)
                .unwrap()
                .max(expected.distance_to(contact).unwrap());
            let world_error = native::point(&click["sight_point"])
                .distance_to(expected)
                .unwrap();
            println!(
                "LINE_PICK_CONTEXT_SOURCE {}",
                serde_json::json!({"id":id,"maximum_projection_point_and_native_contact_error":error,"world_metric_station_difference":world_error})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 14);
}
