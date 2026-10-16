use super::*;
use serde_json::Value;
use viboceros_document::{ControlPointId, SelectionMode};

fn p(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

#[test]
fn plane_through_points_replays_seventeen_native_geometry_and_selection_cases() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/plane_through_points.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/plane_through_points_rhino_20261016.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut count = 0;
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        let points = op["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(p)
            .collect::<Vec<_>>();
        let mut doc = Document::default();
        doc.begin_transaction("Plane sources").unwrap();
        let source_layer = doc
            .add_layer(
                "PlaneFitSource",
                viboceros_document::ColorRgb::new(255, 0, 0),
            )
            .unwrap();
        let output_layer = doc
            .add_layer(
                "PlaneFitCurrent",
                viboceros_document::ColorRgb::new(0, 0, 255),
            )
            .unwrap();
        doc.set_current_layer(output_layer).unwrap();
        let geometries = match op["source"].as_str().unwrap() {
            "points" => points
                .iter()
                .copied()
                .map(Geometry::Point)
                .collect::<Vec<_>>(),
            "cloud" => vec![Geometry::PointCloud(
                PointCloud3::try_new(points.clone()).unwrap(),
            )],
            "mesh_grips" => vec![Geometry::Mesh(
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
                let attributes = ObjectAttributes::on_layer(source_layer).with_name("fit-source");
                doc.add_geometry_with_attributes(g, attributes).unwrap()
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
        let post = op["postselect"].as_bool().unwrap();
        let result = if post {
            registry.execute_postselected(&mut doc, "PlaneThroughPt", CommandContext::default())
        } else {
            registry.execute(&mut doc, "PlaneThroughPt")
        };
        if row["value"]["command_returned"] == false {
            assert!(result.is_err(), "{label}");
        } else {
            result.unwrap();
        }
        let outputs = doc
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        let expected = &row["value"]["outputs"];
        assert_eq!(outputs.len(), expected.as_array().unwrap().len(), "{label}");
        if let Some(output) = outputs.first() {
            let Geometry::NurbsSurface(surface) = output.geometry() else {
                panic!("{label}")
            };
            let expected = &expected[0];
            assert_eq!([surface.degree_u(), surface.degree_v()], [1, 1]);
            assert_eq!(
                [
                    surface.control_point_count_u(),
                    surface.control_point_count_v()
                ],
                [2, 2]
            );
            let actual = surface
                .control_points()
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>();
            // Match one of the eight unit tensor chart symmetries. Exact
            // control-distance admission then bounds the whole bilinear image
            // by convexity, including differently oriented eigenvectors.
            let expected = expected["corners"]
                .as_array()
                .unwrap()
                .iter()
                .map(p)
                .collect::<Vec<_>>();
            let mut matched = false;
            for swap in [false, true] {
                for flip_u in [false, true] {
                    for flip_v in [false, true] {
                        matched |= actual.iter().enumerate().all(|(i, a)| {
                            let (u, v) = if swap { (i / 2, i % 2) } else { (i % 2, i / 2) };
                            let j = (if flip_u { 1 - u } else { u })
                                + 2 * (if flip_v { 1 - v } else { v });
                            a.distance_bound_to(expected[j], 1e-6).unwrap().is_some()
                        });
                    }
                }
            }
            assert!(
                matched,
                "{label}: whole bilinear image did not certify under a tensor chart symmetry"
            );
            assert_eq!(output.attributes().layer_id(), output_layer);
            assert!(output.attributes().name().is_none() && output.group_ids().is_empty());
            assert!(!doc.is_selected(output.id()));
        }
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(
                doc.object(*id).unwrap().geometry(),
                before[i].geometry(),
                "{label}"
            );
            assert_eq!(
                doc.object(*id).unwrap().attributes(),
                before[i].attributes(),
                "{label}"
            );
            assert_eq!(
                doc.is_selected(*id),
                row["value"]["sources"][i]["selected"].as_bool().unwrap(),
                "{label}"
            );
        }
        assert_eq!(
            doc.selected_control_points().count(),
            row["value"]["sources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["grips_selected"].as_u64().unwrap() as usize)
                .sum::<usize>()
        );
        if !outputs.is_empty() {
            let after = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{label}"
            );
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after, "{label}");
        } else {
            assert_eq!(doc.undo_label(), Some("Plane sources"));
        }
        count += 1;
    }
    assert_eq!(count, 17);
}

#[test]
fn plane_through_points_invalid_input_and_no_result_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for _ in 0..3 {
        registry.execute(&mut doc, "Point 1,2,3").unwrap();
    }
    registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "PlaneThroughPt"),
        Err(CommandError::OperationDeclined)
    ));
    assert_eq!(format!("{doc:?}"), before);
    for input in ["PlaneThroughPt 0,0,0", "PlaneThroughPt Unknown=Yes"] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 4);
}
