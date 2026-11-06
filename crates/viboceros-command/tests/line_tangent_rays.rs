#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_tangent_rays.rs"]
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
fn viewing_ray_tangent_lines_match_native_geometry_selection_history_and_archives() {
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
                "LINE_TANGENT_RAYS_COMMAND {}",
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
    assert_eq!(count, 3);
}
#[test]
fn rejected_native_free_mouse_points_preserve_sources_selection_and_pending_redo() {
    use viboceros_command::line::curve_input::{Input, Kind};
    let sources = inputs::sources();
    let r = CommandRegistry::with_builtins();
    let mut count = 0;
    for row in native::read("line_tangent_mouse")["results"]
        .as_array()
        .unwrap()
    {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let source = &sources[name];
        let mut doc = inputs::setup(v, source);
        let before = format!("{doc:?}");
        let mut input = Input::with_kind(Kind::Tangent, native::frame(v), doc.tolerance());
        let mut tokens = vec!["Line".to_string()];
        let mut has_endpoint = false;
        for (i, a) in source["actions"].as_array().unwrap().iter().enumerate() {
            if a.get("curve").is_some() {
                let id = inputs::guide(&doc, 0);
                let p = inputs::pick(v, i, a);
                input.pick_curve(&doc, id, None, p).unwrap();
                tokens.extend([
                    format!("Curve={id}"),
                    inputs::token(&serde_json::json!(p.to_array())),
                ]);
            } else if a.get("click").is_some() {
                if let Some(p) = inputs::mouse_point(v, i, &input) {
                    has_endpoint = true;
                    assert!(input.preview(p).is_none(), "{name}");
                    assert!(input.point(p).is_err(), "{name}");
                    tokens.push(inputs::token(&serde_json::json!(p.to_array())));
                }
            } else if let Some(s) = a.as_str() {
                if s == "Cancel" {
                    continue;
                }
                tokens.push(s.into());
                if s != "Tangent" {
                    assert!(input.option(s).unwrap());
                }
            } else {
                let n = a.as_f64().unwrap();
                input.number(n).unwrap();
                tokens.push(n.to_string());
            }
        }
        if has_endpoint {
            assert!(
                r.execute_in_context(
                    &mut doc,
                    &tokens.join(" "),
                    CommandContext {
                        construction_plane: native::frame(v)
                    }
                )
                .is_err(),
                "{name}"
            );
        }
        assert_eq!(format!("{doc:?}"), before);
        let mut error = replay::check(&doc, &v["after"], false);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        doc.redo().unwrap();
        error = error.max(replay::check(&doc, &v["pending_redo"], false));
        println!(
            "LINE_TANGENT_RAYS_COMMAND {}",
            serde_json::json!({"id":name,"maximum_coefficient_and_sample_error":error,"archives":0})
        );
        assert!(error <= 1e-6);
        count += 1;
    }
    assert_eq!(count, 8);
}
#[test]
fn perspective_fixed_pick_calibration_difference_remains_explicit() {
    let data = native::read("line_tangent_rays");
    let v = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "ray_fixed_elevated")
        .unwrap()["value"];
    let source = &inputs::sources()["ray_fixed_elevated"];
    let mut doc = inputs::setup(v, source);
    let command = inputs::command(&doc, v, source);
    CommandRegistry::with_builtins()
        .execute_in_context(
            &mut doc,
            &command,
            CommandContext {
                construction_plane: native::frame(v),
            },
        )
        .unwrap();
    let error = replay::check(&doc, &v["after"], false);
    assert!(error > 1e-6 && error < 1e-3);
    println!(
        "LINE_TANGENT_RAYS_KNOWN {}",
        serde_json::json!({"id":"ray_fixed_elevated","maximum_coefficient_and_sample_error":error})
    );
}
