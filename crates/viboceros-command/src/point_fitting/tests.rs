use super::*;
use serde_json::Value;
use viboceros_document::{ColorRgb, ControlPointId, SelectionMode};
fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn endpoints_match(line: LineSegment, native: &Value) -> bool {
    let expected = native.as_array().unwrap().iter().map(p).collect::<Vec<_>>();
    [false, true].into_iter().any(|reverse| {
        [line.start(), line.end()]
            .into_iter()
            .enumerate()
            .all(|(i, p)| {
                p.distance_bound_to(expected[if reverse { 1 - i } else { i }], 1e-6)
                    .unwrap()
                    .is_some()
            })
    })
}

#[test]
fn line_through_points_replays_native_complete_line_images_sources_and_history() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_through_points.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/point_fitting_rhino_20261016.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for op in request["operations"].as_array().unwrap() {
        let native = &response["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == op["id"])
            .unwrap()["value"];
        let points = op["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(p)
            .collect::<Vec<_>>();
        let mut doc = Document::default();
        doc.begin_transaction("fit sources").unwrap();
        let source_layer = doc
            .add_layer("LineFitSource", ColorRgb::new(255, 0, 0))
            .unwrap();
        let current_layer = doc
            .add_layer("LineFitCurrent", ColorRgb::new(0, 0, 255))
            .unwrap();
        doc.set_current_layer(current_layer).unwrap();
        let geometries = match op["source"].as_str().unwrap() {
            "points" => points
                .iter()
                .copied()
                .map(Geometry::Point)
                .collect::<Vec<_>>(),
            "cloud" => vec![Geometry::PointCloud(
                PointCloud3::try_new(points.clone()).unwrap(),
            )],
            "mesh" | "mesh_grips" => vec![Geometry::Mesh(
                TriangleMesh::try_new(points.clone(), vec![[0, 1, 2]], doc.tolerance()).unwrap(),
            )],
            "curve_grips" => vec![Geometry::NurbsCurve(
                NurbsCurve::try_clamped_uniform(1, points.clone()).unwrap(),
            )],
            _ => unreachable!(),
        };
        let ids = geometries
            .into_iter()
            .map(|g| {
                doc.add_geometry_with_attributes(
                    g,
                    ObjectAttributes::on_layer(source_layer)
                        .with_name("fit-source")
                        .with_object_color(ColorRgb::new(255, 0, 0)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        doc.commit_transaction().unwrap();
        if op["source"].as_str().unwrap().ends_with("_grips") {
            doc.enable_control_points(ids.iter().copied()).unwrap();
            doc.select_control_points(
                (0..points.len()).map(|index| ControlPointId {
                    object: ids[0],
                    index,
                }),
                SelectionMode::Replace,
            )
            .unwrap();
        } else {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let result = if op["postselect"] == true {
            registry.execute_postselected(&mut doc, "LineThroughPt", CommandContext::default())
        } else {
            registry.execute(&mut doc, "LineThroughPt")
        };
        assert_eq!(
            result.is_ok(),
            native["command_returned"].as_bool().unwrap(),
            "{}: {result:?}",
            op["id"]
        );
        let output = doc
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        assert_eq!(
            output.len(),
            native["outputs"].as_array().unwrap().len(),
            "{}",
            op["id"]
        );
        for object in &output {
            let Geometry::Line(line) = object.geometry() else {
                panic!("not a line")
            };
            assert!(
                endpoints_match(*line, &native["outputs"][0]["endpoints"]),
                "{}: {line:?}",
                op["id"]
            );
            assert_eq!(object.attributes().layer_id(), current_layer);
            assert!(object.attributes().name().is_none() && object.group_ids().is_empty());
            assert_eq!(
                object.attributes().color_source(),
                viboceros_document::ObjectColorSource::Layer
            );
            assert!(!doc.is_selected(object.id()));
        }
        for (i, id) in ids.iter().enumerate() {
            let object = doc.object(*id).unwrap();
            assert_eq!(object.geometry(), before[i].geometry());
            assert_eq!(object.attributes(), before[i].attributes());
            assert_eq!(
                doc.is_selected(*id),
                native["sources"][i]["selected"].as_bool().unwrap(),
                "{}",
                op["id"]
            );
        }
        assert_eq!(
            doc.selected_control_points().count(),
            native["sources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["grips_selected"].as_u64().unwrap() as usize)
                .sum::<usize>()
        );
        if !output.is_empty() {
            let after = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        } else {
            assert_eq!(doc.undo_label(), Some("fit sources"));
        }
        count += 1;
    }
    assert_eq!(count, 25);
}

#[test]
fn line_through_points_no_result_invalid_options_and_mesh_source_filter_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for _ in 0..3 {
        registry.execute(&mut doc, "Point 1,2,3").unwrap();
    }
    registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    registry.execute(&mut doc, "LineThroughPt").unwrap();
    assert_eq!(format!("{doc:?}"), before);
    for input in ["LineThroughPt 0,0,0", "LineThroughPt Unknown=Yes"] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 4);
}
