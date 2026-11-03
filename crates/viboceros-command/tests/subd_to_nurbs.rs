//! Actual Rhino command evidence, independent of the conversion implementation.
use serde_json::Value;
use std::io::Read;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::Geometry;
use viboceros_geometry::SubDNurbsContinuity;
#[path = "../../viboceros-geometry/tests/support/nurbs_common_basis.rs"]
mod basis;
#[path = "../../viboceros-geometry/tests/support/positive_tensor.rs"]
mod bound;
#[path = "support/subd_nurbs_images.rs"]
mod geometry;
#[path = "../../viboceros-geometry/tests/support/subd_packed_images.rs"]
mod images;
#[path = "../../viboceros-geometry/tests/support/subd_to_nurbs.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/subd_nurbs_state.rs"]
mod state;
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod surface;
#[path = "../../viboceros-geometry/tests/support/subd_packed_topology.rs"]
mod topology;

fn run(bytes: &[u8]) {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut raw)
        .unwrap();
    let capture: Value = serde_json::from_slice(&raw).unwrap();
    for row in capture["results"].as_array().unwrap() {
        let registry = CommandRegistry::with_builtins();
        let mut packed = true;
        let mut mode = SubDNurbsContinuity::G1x;
        for step in row["value"]["steps"].as_array().unwrap() {
            println!("{} {}", row["id"], step["options"]);
            let mut scene = state::setup(step);
            let options = &step["options"];
            let mut command = "ToNURBS".to_owned();
            for (field, name) in [
                ("delete", "DeleteInputObjects"),
                ("packed", "Faces"),
                ("mode", "ExtraordinaryVertex"),
            ] {
                if !options[field].is_null() {
                    let value = if field == "packed" {
                        if options[field] == true {
                            "Packed"
                        } else {
                            "Unpacked"
                        }
                    } else if field == "delete" {
                        if options[field] == true { "Yes" } else { "No" }
                    } else {
                        options[field].as_str().unwrap()
                    };
                    command.push_str(&format!(" {name}={value}"));
                }
            }
            let post = options["postselect"] == true;
            if !options["cancel"].is_null() {
                // Cancelled descriptors are discarded; their typed edits never
                // reach the command's remembered choices or history.
                let mut prompt = registry.object_selection_prompt(&command).unwrap().unwrap();
                if options["cancel"] != "selection" {
                    state::select(&mut scene, post);
                    prompt = registry
                        .object_selection_confirmation(&scene.doc, &prompt)
                        .unwrap()
                        .unwrap();
                }
                registry.accept_object_selection_options(&prompt).unwrap();
                if post {
                    scene.doc.clear_selection();
                }
                if !step["after_script"].is_null() {
                    state::check(&scene, &step["after_script"]);
                }
                continue;
            }
            if let Some(value) = options["packed"].as_bool() {
                packed = value;
            }
            if let Some(value) = options["mode"].as_str() {
                mode = match value {
                    "G0" => SubDNurbsContinuity::G0,
                    "G1" => SubDNurbsContinuity::G1,
                    "G2" => SubDNurbsContinuity::G2,
                    "G1x" => SubDNurbsContinuity::G1x,
                    "G1xx" => SubDNurbsContinuity::G1xx,
                    _ => unreachable!(),
                };
            }
            state::select(&mut scene, post);
            let before = scene.doc.objects().cloned().collect::<Vec<_>>();
            if post {
                registry
                    .execute_postselected(&mut scene.doc, &command, CommandContext::default())
                    .unwrap();
            } else {
                registry.execute(&mut scene.doc, &command).unwrap();
            }
            let settled = if !step["after_script"].is_null() {
                &step["after_script"]
            } else if post {
                &step["redo"]
            } else {
                &step["after"]
            };
            state::check(&scene, settled);
            for (object, expected) in scene
                .doc
                .objects()
                .zip(step["after"]["objects"].as_array().unwrap())
            {
                if let Geometry::Brep(brep) = object.geometry()
                    && expected["name"] == "Input 0"
                {
                    geometry::check(brep, &scene.source, packed, mode, &expected["brep"]);
                }
            }
            let after = scene.doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut scene.doc, "Undo").unwrap();
            state::check(&scene, &step["undo"]);
            assert_eq!(scene.doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut scene.doc, "Redo").unwrap();
            state::check(&scene, &step["redo"]);
            assert_eq!(scene.doc.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn native_subd_command_geometry_options_and_history() {
    run(include_bytes!(
        "../../../tools/rhino_oracle/observations/subd_nurbs_command.json.gz"
    ));
}

#[test]
fn native_settled_selection_mixed_mesh_and_subd_and_cancellation() {
    run(include_bytes!(
        "../../../tools/rhino_oracle/observations/subd_nurbs_lifecycle.json.gz"
    ));
}
