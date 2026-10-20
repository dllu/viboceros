use super::*;

#[test]
fn typed_corner_points_match_native_acceptance_geometry_and_history() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    for (op, value) in support::corner_through_rows() {
        let (mut doc, scene) = support::setup(&op, &value);
        let mut group = doc.begin_history_group("OffsetCrvOnSrf").unwrap();
        support::check(&doc, &scene, &value, "before");
        let mut rejected = 0;
        for p in op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a.is_array())
        {
            let [x, y, z]: [f64; 3] = serde_json::from_value(p.clone()).unwrap();
            let before = format!("{doc:?}");
            let result=registry.execute_in_history_group(&mut doc,&format!("OffsetCrvOnSrf ThroughPoint={x},{y},{z} Source={} Surface={} Number=3 Flip=Yes",scene.source,scene.target),viboceros_command::CommandContext::default(),&mut group);
            if let Err(error) = result {
                assert!(
                    matches!(
                        error,
                        viboceros_command::CommandError::Geometry(
                            viboceros_geometry::GeometryError::OffsetThroughPointNoSolution
                        )
                    ),
                    "{}: {error}",
                    op["id"]
                );
                assert_eq!(format!("{doc:?}"), before);
                rejected += 1;
            }
        }
        let mut maximum = support::check(&doc, &scene, &value, "after_script");
        let count = value["after_script"].as_array().unwrap().len() - 2;
        if count > 0 {
            doc.undo().unwrap();
            maximum = maximum.max(support::check(&doc, &scene, &value, "undo"));
            assert!(doc.undo().unwrap().is_none());
            doc.redo().unwrap();
            maximum = maximum.max(support::check(&doc, &scene, &value, "redo"));
        } else {
            assert!(doc.undo_label().is_none());
        }
        println!(
            "SURFACE_OFFSET_CORNER_THROUGH {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":count,"rejected_points":rejected})
        );
        assert!(maximum < 1e-6, "{}: {maximum:e}", op["id"]);
    }
}

#[test]
fn selected_corner_points_skip_missing_offsets_before_one_native_history_commit() {
    use viboceros_command::{
        orient_surface::Target,
        surface_curve_offset::{Options, PointSetPlan, Source},
    };
    for (op, value) in support::corner_select_rows() {
        let (mut doc, scene) = support::setup(&op, &value);
        let ids = op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["curve"].as_u64())
            .filter(|i| *i >= 2)
            .map(|i| scene.references[i as usize - 2])
            .collect::<Vec<_>>();
        let before = format!("{doc:?}");
        let plan = PointSetPlan::prepare(
            &doc,
            Source::capture(&doc, scene.source, None, None).unwrap(),
            Target::capture(&doc, scene.target, None).unwrap(),
            &ids,
            Options {
                distance: 77.,
                number: 3,
                flip: true,
            },
        )
        .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert_eq!(format!("{doc:?}"), before);
        assert_eq!(
            plan.skipped,
            usize::from(op["id"] == "through_corner_select_on_source")
        );
        assert_eq!(
            plan.unresolved,
            usize::from(op["id"] == "through_corner_select_unresolved")
        );
        let mut group = doc.begin_history_group("OffsetCrvOnSrf").unwrap();
        plan.commit(&mut doc, &mut group).unwrap();
        let mut maximum = support::check(&doc, &scene, &value, "after_script");
        doc.undo().unwrap();
        maximum = maximum.max(support::check(&doc, &scene, &value, "undo"));
        assert!(doc.undo().unwrap().is_none());
        doc.redo().unwrap();
        maximum = maximum.max(support::check(&doc, &scene, &value, "redo"));
        println!(
            "SURFACE_OFFSET_CORNER_SELECT {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":plan.curves.len(),"on_source_points":plan.skipped,"unresolved_points":plan.unresolved})
        );
        assert!(maximum < 1e-6, "{}: {maximum:e}", op["id"]);
    }
}
