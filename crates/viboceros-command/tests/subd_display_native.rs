#[path = "support/extract_points.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn subd_display_toggles_preserve_native_state_and_drive_extraction() {
    let registry = CommandRegistry::with_builtins();
    for row in native::display_rows() {
        let (mut doc, ids) = native::setup(&row);
        let storage = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        let flat = row["value"]["input"]["flat"].as_bool().unwrap();
        doc.set_subd_control_net(!flat);
        let before = native::snapshot(&doc, &ids);
        registry.execute(&mut doc, "SubDDisplayToggle").unwrap();
        assert_eq!(doc.subd_control_net(), flat);
        assert_eq!(native::snapshot(&doc, &ids), before);
        assert!(!doc.can_undo());
        registry
            .execute_postselected(&mut doc, &native::command(&row), Default::default())
            .unwrap();
        let mut stages = Vec::new();
        let mut modes = Vec::new();
        for stage in ["after", "undo", "redo"] {
            if stage != "after" {
                registry
                    .execute(&mut doc, if stage == "undo" { "Undo" } else { "Redo" })
                    .unwrap();
            }
            for (id, source) in ids.iter().zip(&storage) {
                assert!(
                    doc.object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(source)
                );
            }
            native::compare(&doc, &ids, &row["value"][stage]);
            stages.push(native::snapshot(&doc, &ids));
            modes.push(doc.subd_control_net());
        }
        println!(
            "SUBD_DISPLAY_COMMAND {}",
            serde_json::json!({"id":row["id"],"success":true,
            "toggle_preserved":true,"source_storage_unchanged":true,"modes":modes,"after":stages[0],"undo":stages[1],"redo":stages[2]})
        );
    }
}

#[test]
fn subd_display_location_overrides_also_apply_to_explicit_object_inputs() {
    use viboceros_document::Geometry;
    let registry = CommandRegistry::with_builtins();
    for row in native::display_rows().into_iter().filter(|r| {
        r["value"]["input"]["flat"] == true && r["value"]["input"]["sources"][0]["mode"].is_null()
    }) {
        for flat in [false, true] {
            for location in ["Display", "Surface", "ControlNet"] {
                for explicit in [false, true] {
                    let (mut doc, ids) = native::setup(&row);
                    doc.set_subd_control_net(flat);
                    let controls = location == "ControlNet" || (location == "Display" && flat);
                    let mut input = format!("ExtractPt SubDLocation={location} Output=PointCloud");
                    if explicit {
                        doc.clear_selection();
                        input.push_str(&format!(" Object={}", ids[0]));
                    }
                    registry.execute(&mut doc, &input).unwrap();
                    let Geometry::PointCloud(cloud) = doc.objects().last().unwrap().geometry()
                    else {
                        panic!()
                    };
                    let vertices = row["value"]["subd_witness"]["sources"][0]["vertices"]
                        .as_array()
                        .unwrap();
                    assert_eq!(cloud.points().len(), vertices.len());
                    for (p, v) in cloud.points().iter().zip(vertices) {
                        let expected = serde_json::from_value::<[f64; 3]>(
                            v[if controls { "control" } else { "surface" }].clone(),
                        )
                        .unwrap();
                        assert!(
                            p.to_array()
                                .iter()
                                .zip(expected)
                                .all(|(a, b)| (a - b).abs() < 1e-7)
                        );
                    }
                }
            }
        }
    }
}
