//! Native object policy and complete geometry for the registered Ribbon command.
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandError, CommandRegistry};
use viboceros_document::{ColorRgb, Document, Geometry, Object, ObjectAttributes, SelectionMode};
use viboceros_geometry::*;
#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "support/interpolation_plane.rs"]
#[allow(dead_code)]
mod curves;
#[path = "../../viboceros-oracle/tests/support/ribbon.rs"]
#[allow(dead_code)]
mod support;

fn attrs(doc: &Document, object: &Object, row: &Value) {
    let a = &row["attributes"];
    assert_eq!(
        object.attributes().name().unwrap_or(""),
        a["name"].as_str().unwrap()
    );
    assert_eq!(
        doc.layer(object.attributes().layer_id()).unwrap().name(),
        a["layer"].as_str().unwrap()
    );
    assert_eq!(
        object.attributes().object_color(),
        ColorRgb::new(
            a["color"][0].as_u64().unwrap() as u8,
            a["color"][1].as_u64().unwrap() as u8,
            a["color"][2].as_u64().unwrap() as u8
        )
    );
    assert_eq!(
        format!("{:?}", object.attributes().color_source()),
        if a["color_source"] == "ColorFromObject" {
            "Object"
        } else {
            "Layer"
        }
    );
    assert_eq!(
        object.group_ids().len(),
        a["groups"].as_array().unwrap().len()
    );
    assert_eq!(
        doc.is_selected(object.id()),
        row["selected"].as_bool().unwrap()
    );
}
#[test]
fn ribbon_command_matches_native_geometry_attributes_selection_and_history() {
    let mut cases = 0;
    let mut gaps = 0;
    for family in ["ribbon_options", "ribbon_workflows"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/rhino_oracle/fixtures");
        let q: Value =
            serde_json::from_slice(&std::fs::read(root.join(format!("{family}.json"))).unwrap())
                .unwrap();
        let n = captured::read(family);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(n["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let v = &row["value"];
            assert_eq!(v["success"], true);
            assert!(!v["history"].as_str().unwrap().contains("Unknown command"));
            let original = curves::curve(&v["before"][0]["curve"]);
            let mut doc = Document::new(captured::model_tolerance());
            let layer = doc.add_layer("Ribbon source", ColorRgb::BLACK).unwrap();
            let output_layer = doc.add_layer("Ribbon output", ColorRgb::BLACK).unwrap();
            let id = doc
                .add_geometry_with_attributes(
                    Geometry::NurbsCurve(original.clone()),
                    ObjectAttributes::on_layer(layer)
                        .with_name("Ribbon guide")
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap();
            doc.add_group(Some("Ribbon group".into()), [id]).unwrap();
            doc.set_current_layer(output_layer).unwrap();
            let post = op["postselected"] == true;
            if !post {
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            attrs(&doc, doc.object(id).unwrap(), &v["before"][0]);
            doc.clear_history().unwrap();
            if post {
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            let source = doc.object(id).unwrap().clone();
            let registry = CommandRegistry::with_builtins();
            let point = op["side"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            let yes = |k| if op[k] == true { "Yes" } else { "No" };
            let text = format!(
                "Ribbon Distance={} w{point} BothSides={} Loose={} Corner={} InCPlane={} OutputLayer={}",
                op["distance"],
                yes("both_sides"),
                yes("loose"),
                op["corner"].as_str().unwrap(),
                yes("in_cplane"),
                op["output_layer"].as_str().unwrap()
            );
            let before = format!("{doc:?}");
            let result = if post {
                registry.execute_postselected(&mut doc, &text, CommandContext::default())
            } else {
                registry.execute(&mut doc, &text)
            };
            let gap = family == "ribbon_workflows"
                && ["tilted-cplane", "spatial-curve-plane", "spatial-cplane"]
                    .contains(&op["id"].as_str().unwrap());
            if gap {
                assert!(
                    matches!(result, Err(CommandError::RibbonCPlaneUnavailable)),
                    "{result:?}"
                );
                assert_eq!(format!("{doc:?}"), before);
                assert!(!doc.can_undo());
                println!(
                    "RIBBON_COMMAND {}",
                    json!({"family":family,"id":op["id"],"native_success":true,"local_supported":false,"reason":"ordinary_cplane_fitter"})
                );
                gaps += 1;
                continue;
            }
            result.unwrap();
            let outputs = doc.objects().filter(|o| o.id() != id).collect::<Vec<_>>();
            let native = v["created"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["source"] == false)
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), native.len());
            assert_eq!(outputs.len(), 1);
            attrs(&doc, doc.object(id).unwrap(), &v["after"][0]);
            attrs(&doc, outputs[0], native[0]);
            let Geometry::Brep(brep) = outputs[0].geometry() else {
                panic!("Ribbon B-rep");
            };
            let targets = native[0]["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .map(captured::surface)
                .collect::<Vec<_>>();
            let parents = brep
                .faces()
                .iter()
                .map(|f| f.surface().clone())
                .collect::<Vec<_>>();
            let image = support::coverage(brep, &parents, &native[0]["brep"]["geometry"], &targets)
                .unwrap();
            assert!(image <= 1e-9);
            let mut used = vec![false; targets.len()];
            let mut face_map = Vec::new();
            for face in brep.faces() {
                let found = targets
                    .iter()
                    .enumerate()
                    .find(|(i, s)| {
                        !used[*i]
                            && face
                                .surface()
                                .parameter_surface_deviation_bound(s, 1e-9)
                                .unwrap()
                                .is_some()
                    })
                    .unwrap()
                    .0;
                used[found] = true;
                face_map.push(found);
            }
            let incidence =
                support::incidence(brep, &native[0]["brep"]["geometry"], &face_map).unwrap();
            assert!(incidence <= 1e-9);
            let output = outputs[0].clone();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().len(), 1);
            attrs(&doc, doc.object(id).unwrap(), &v["undo"][0]);
            assert_eq!(doc.object(id).unwrap().geometry(), source.geometry());
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().len(), 2);
            attrs(&doc, doc.object(id).unwrap(), &v["redo"][0]);
            let restored = doc.object(output.id()).unwrap();
            assert_eq!(restored.geometry(), output.geometry());
            assert_eq!(restored.attributes(), output.attributes());
            assert!(!doc.is_selected(restored.id()));
            println!(
                "RIBBON_COMMAND {}",
                json!({"family":family,"id":op["id"],"native_success":true,"local_supported":true,
                "image_bound":image,"incidence_bound":incidence,"attributes_selection_history":true,"full_command_parity":false})
            );
            cases += 1;
        }
    }
    assert_eq!((cases, gaps), (22, 3));
}
