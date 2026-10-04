use super::*;
use serde_json::Value;

fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}

#[test]
fn circle_fit_points_commands_replay_22_native_circle_loci_and_selection_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/circle_fit_points.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/circle_fit_points.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut compared = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let label = op["id"].as_str().unwrap();
        let mut doc = Document::default();
        doc.begin_transaction("Circle fit sources").unwrap();
        let ids = op["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| doc.add_geometry(Geometry::Point(point(p))).unwrap())
            .collect::<Vec<_>>();
        doc.commit_transaction().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        registry
            .execute_postselected(&mut doc, "Circle FitPoints", CommandContext::default())
            .unwrap();
        assert_eq!(
            doc.selected_object_ids().collect::<Vec<_>>(),
            ids,
            "{label}"
        );
        let native = &row["value"]["sdk"];
        let radius = native["radius"].as_f64().unwrap();
        if radius == 0. {
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.undo_label(), Some("Circle fit sources"));
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().len(), 0);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        } else {
            let Geometry::Circle(circle) = doc.objects().last().unwrap().geometry() else {
                panic!("{label}")
            };
            let epsilon = (16. * (Real::from_bits(radius.to_bits() + 1) - radius)).max(1e-7);
            assert!(
                circle
                    .center()
                    .distance_to(point(&native["origin"]))
                    .unwrap()
                    + (circle.radius() - radius).abs()
                    <= epsilon,
                "{label}"
            );
            assert!(!doc.is_selected(doc.objects().last().unwrap().id()));
            assert_eq!(doc.undo_label(), Some("Circle"));
            let after = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{label}"
            );
            assert_eq!(doc.selected_object_count(), 0);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after, "{label}");
        }
        assert_eq!(doc.selected_object_count(), 0);
        compared += 1;
    }
    assert_eq!(compared, 22);
}

#[test]
fn circle_fit_points_invalid_invocations_and_degenerate_fits_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for x in 0..3 {
        registry
            .execute(&mut doc, &format!("Point {x},0,0"))
            .unwrap();
    }
    registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    registry.execute(&mut doc, "C _FitPoints").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), undo.as_deref());
    assert_eq!(doc.redo_label(), redo.as_deref());
    assert_eq!(doc.selected_object_count(), 3);
    let before = format!("{doc:?}");
    for input in ["Circle FitPoints extra", "Circle FitPoints 0,0,0"] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 4);
    doc.clear_selection();
    let before = format!("{doc:?}");
    assert!(registry.execute(&mut doc, "Circle FitPoints").is_err());
    assert_eq!(format!("{doc:?}"), before);
}
