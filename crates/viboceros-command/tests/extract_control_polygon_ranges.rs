//! Native temporary ranges retain exact control vertices, metadata and history.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_command::CommandRegistry;
#[path = "support/control_polygon.rs"]
mod native;

#[test]
fn temporary_ranges_replay_native_controls_attributes_and_history() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let family = "control_polygon_components";
    let captured: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
            .unwrap(),
    )
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for row in captured["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["subcurve_parameters"].is_null() {
            continue;
        }
        let id = row["id"].as_str().unwrap();
        let (mut doc, sources) = native::setup(family, id, v, id);
        let original = doc.object(sources[0]).unwrap().geometry_snapshot().clone();
        let p = v["subcurve_parameters"].as_array().unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtractControlPolygon SubCrv={},{},{} OutputLayer={}",
                    sources[0],
                    p[0],
                    p[1],
                    v["input"]["layer"].as_str().unwrap()
                ),
            )
            .unwrap();
        let error = native::compare(&doc, &sources, &v["after"], id);
        assert_eq!(
            doc.object(sources[0]).unwrap().geometry_snapshot(),
            &original
        );
        assert_eq!(doc.undo_label(), Some("ExtractControlPolygon"));
        registry.execute(&mut doc, "Undo").unwrap();
        native::compare(&doc, &sources, &v["undo"], id);
        registry.execute(&mut doc, "Redo").unwrap();
        native::compare(&doc, &sources, &v["redo"], id);
        println!(
            "CONTROL_POLYGON_RANGE_NATIVE {}",
            json!({"id":id,"maximum_error":error,"metadata_exact":true,"selection_exact":true,"history_exact":true})
        );
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn extraction_plans_reject_changed_inputs_and_invalid_batches_without_history_edits() {
    use viboceros_command::{
        CommandError,
        extract_control_polygon::{Options, Selection, Target},
    };
    use viboceros_document::Document;
    for change in ["geometry", "attributes", "groups", "layer", "locked"] {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
        let source = doc.objects().next().unwrap().id();
        doc.select_objects_direct([source], viboceros_document::SelectionMode::Replace)
            .unwrap();
        let plan = Selection::prepare(
            &doc,
            [Target::CurveRange(source, [2., 8.])],
            Options::default(),
        )
        .unwrap();
        match change {
            "geometry" => {
                registry.execute(&mut doc, "Move 0,0,0 0,1,0").unwrap();
            }
            "attributes" => {
                doc.set_object_names([(source, Some("changed".into()))])
                    .unwrap();
            }
            "groups" => {
                doc.add_group(Some("changed".into()), [source]).unwrap();
            }
            "layer" => {
                let layer = doc
                    .add_layer("changed", viboceros_document::ColorRgb::BLACK)
                    .unwrap();
                doc.set_current_layer(layer).unwrap();
            }
            "locked" => {
                doc.set_objects_locked([source], true).unwrap();
            }
            _ => unreachable!(),
        }
        let before = doc
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry_snapshot().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        let undo = doc.undo_label().map(str::to_owned);
        assert!(
            matches!(
                plan.commit(&mut doc),
                Err(CommandError::ControlPolygonStale)
            ),
            "{change}"
        );
        assert_eq!(
            before,
            doc.objects()
                .map(|o| (
                    o.id(),
                    o.geometry_snapshot().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec()
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(doc.undo_label(), undo.as_deref());
    }
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    let source = doc.objects().next().unwrap().id();
    for parameters in [[2., 2.], [-1., 4.], [1., f64::NAN], [1., f64::INFINITY]] {
        let undo = doc.undo_label().map(str::to_owned);
        assert!(
            Selection::prepare(
                &doc,
                [
                    Target::CurveRange(source, [2., 8.]),
                    Target::CurveRange(source, parameters)
                ],
                Options::default()
            )
            .is_err()
        );
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(doc.undo_label(), undo.as_deref());
    }
}
