#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_tangent_arcs.rs"]
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
fn bounded_tangent_lines_match_native_geometry_selection_history_and_archives() {
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
                "LINE_TANGENT_BOUNDARY_COMMAND {}",
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
    assert_eq!(count, 24);
}
#[test]
fn tangent_fallback_rejects_interior_and_zero_length_projections_atomically() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    r.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    r.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for endpoint in ["7,4,3", "10,4,3", "0,4,3"] {
        assert!(
            r.execute(
                &mut doc,
                &format!("Line Tangent Curve={id} 5,0,0 {endpoint}")
            )
            .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        assert!(doc.can_redo());
    }
}

#[test]
fn public_sdk_contacts_evaluate_on_independently_prescribed_curves() {
    use viboceros_geometry::{CurveRef, Vector3};
    let ui = inputs::sources();
    let sdk: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/line_tangent_arcs_sdk_sources.json"
    ))
    .unwrap();
    let mut cases = 0;
    let mut queries = 0;
    for family in [
        "line_tangent_arcs",
        "line_tangent_arcs_ends",
        "line_tangent_arcs_controls",
        "line_tangent_arcs_sdk",
        "line_tangent_arcs_bounds",
    ] {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            if id == "arc_scaled" {
                continue;
            }
            let v = &row["value"];
            let is_ui = v.get("contact_sdk").is_some();
            let (source, target, contacts) = if is_ui {
                (
                    &ui[id]["curves"][0],
                    ui[id]["actions"].as_array().unwrap().last().unwrap(),
                    &v["contact_sdk"],
                )
            } else {
                (&sdk[id]["source"], &sdk[id]["target"], &v["contacts"])
            };
            let geometry = reference_inputs::geometry(source);
            let original = geometry.curve_ref().unwrap();
            let nurbs = original.to_nurbs().unwrap();
            let target = native::point(target);
            let mut error: f64 = 0.;
            let mut count = 0;
            let mut failures = 0;
            for variant in contacts.as_array().unwrap() {
                let c = if variant["representation"] == "analytic" {
                    original
                } else {
                    CurveRef::NurbsCurve(&nurbs)
                };
                for q in variant["queries"].as_array().unwrap() {
                    count += 1;
                    if q["success"] != true {
                        failures += 1;
                        continue;
                    }
                    let t = q["parameter"].as_f64().unwrap();
                    let sample = c.evaluate_with_tangent(t).unwrap();
                    error = error.max(
                        sample
                            .point()
                            .distance_to(native::point(&q["point"]))
                            .unwrap(),
                    );
                    let native_tangent =
                        Vector3::try_from(native::point(&q["tangent"]).to_array()).unwrap();
                    let tangent = sample.tangent().as_vector();
                    error = error.max(
                        Vector3::try_from(std::array::from_fn(|i| {
                            tangent.to_array()[i] - native_tangent.to_array()[i]
                        }))
                        .unwrap()
                        .length()
                        .unwrap(),
                    );
                    error = error.max(
                        sample
                            .point()
                            .vector_to(target)
                            .unwrap()
                            .cross(tangent)
                            .unwrap()
                            .length()
                            .unwrap(),
                    );
                }
            }
            println!(
                "LINE_TANGENT_CONTACTS {}",
                serde_json::json!({"family":family,"id":id,"queries":count,"failures":failures,"maximum_point_tangent_and_residual_error":error})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            cases += 1;
            queries += count;
        }
    }
    assert_eq!(cases, 49);
    assert_eq!(queries, 34708);
}
