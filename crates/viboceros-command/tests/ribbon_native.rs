//! Native object policy and complete geometry for the registered Ribbon command.
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{ColorRgb, Document, Geometry, Object, ObjectAttributes, SelectionMode};
use viboceros_geometry::*;
#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "support/interpolation_plane.rs"]
#[allow(dead_code)]
mod curves;
#[path = "support/ribbon_images.rs"]
mod images;
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
fn family(family: &str, expected_cases: usize, expected_gaps: usize) {
    let mut cases = 0;
    let mut gaps = 0;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle/fixtures");
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
        let model = captured::model_tolerance();
        let model = Tolerance::try_new(
            model.absolute(),
            model.relative(),
            op["model_angle_tolerance"]
                .as_f64()
                .unwrap_or(model.angular()),
        )
        .unwrap();
        let mut doc = Document::new(model);
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
        let mut text = format!(
            "Ribbon Distance={} w{point} BothSides={} Loose={} Corner={} InCPlane={} OutputLayer={}",
            op["distance"],
            yes("both_sides"),
            yes("loose"),
            op["corner"].as_str().unwrap(),
            yes("in_cplane"),
            op["output_layer"].as_str().unwrap()
        );
        if let Some(t) = op["fit_tolerance"].as_f64() {
            text.push_str(&format!(" Tolerance={t}"));
        }
        let context = if let Some(f) = op["cplane"].as_array() {
            CommandContext {
                construction_plane: Frame3::try_from_directions(
                    captured::point(&f[0]),
                    Vector3::try_from(serde_json::from_value::<[f64; 3]>(f[1].clone()).unwrap())
                        .unwrap(),
                    Vector3::try_from(serde_json::from_value::<[f64; 3]>(f[2].clone()).unwrap())
                        .unwrap(),
                    doc.tolerance(),
                )
                .unwrap(),
            }
        } else {
            CommandContext::default()
        };
        let before = format!("{doc:?}");
        let prefs = registry.ribbon_options();
        let result = if post {
            registry.execute_postselected(&mut doc, &text, context)
        } else {
            registry.execute_in_context(&mut doc, &text, context)
        };
        if images::expected_gap(family, op["id"].as_str().unwrap()) {
            assert!(
                result.as_ref().err().is_some_and(images::unsupported),
                "{family}/{}: {result:?}",
                op["id"]
            );
            assert_eq!(format!("{doc:?}"), before);
            assert_eq!(registry.ribbon_options(), prefs);
            println!(
                "RIBBON_COMMAND {}",
                json!({"family":family,"id":op["id"],
                    "native_success":true,"local_supported":false,"reason":if family=="ribbon_cplane_holdouts"{"coarse_nonpolynomial_fit"}else{"unqualified_plane_switch"},
                    "failure_atomic":true,"full_command_parity":false,"u_correspondence_compared":false})
            );
            gaps += 1;
            cases += 1;
            continue;
        }
        result.unwrap_or_else(|e| panic!("{family}/{}: {e:?}", op["id"]));
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
        let legacy = ["ribbon_options", "ribbon_workflows"].contains(&family);
        let (image, incidence) = images::verify(
            brep,
            &original,
            native[0],
            &format!("{family}/{}", op["id"]),
            legacy,
        );
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
                "image_bound":image,"incidence_bound":incidence,"attributes_selection_history":true,"full_command_parity":false,"u_correspondence_compared":legacy})
        );
        cases += 1;
    }
    assert_eq!(cases, expected_cases);
    assert_eq!(gaps, expected_gaps);
}

macro_rules! replay {
    ($name:ident,$cases:literal,$gaps:literal) => {
        #[test]
        fn $name() {
            family(stringify!($name), $cases, $gaps);
        }
    };
}
replay!(ribbon_options, 13, 0);
replay!(ribbon_workflows, 12, 0);
replay!(ribbon_cplane, 32, 0);
replay!(ribbon_cplane_boundaries, 15, 6);
replay!(ribbon_cplane_policy, 20, 4);
replay!(ribbon_cplane_switches, 17, 16);
replay!(ribbon_cplane_thresholds, 16, 16);
replay!(ribbon_cplane_holdouts, 6, 4);
