//! Temporary curve references retain native controls, attributes and history.
#[path = "support/extract_points.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn circular_edge_storage_and_native_parameterization_survive_3dm_round_trips() {
    for kind in ["brep_cylinder", "brep_nurbs_cylinder"] {
        let row = native::range_rows()
            .into_iter()
            .find(|r| r["id"] == format!("range_form_{kind}_forward_points"))
            .unwrap();
        let (doc, ids) = native::setup(&row);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("edges.3dm");
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let reopened = viboceros_command::open_3dm_with_views(path.to_str().unwrap())
            .unwrap()
            .0;
        let source = doc.object(ids[0]).unwrap().geometry();
        let restored = reopened.objects().next().unwrap().geometry();
        let (viboceros_document::Geometry::Brep(a), viboceros_document::Geometry::Brep(b)) =
            (source, restored)
        else {
            panic!()
        };
        assert_eq!(a.edges().len(), b.edges().len());
        for (a, b) in a.edges().iter().zip(b.edges()) {
            assert_eq!(a.has_arc_representation(), b.has_arc_representation());
            assert_eq!(a.curve().degree(), b.curve().degree());
            assert_eq!(a.curve().knots(), b.curve().knots());
            assert_eq!(
                a.curve().control_points().len(),
                b.curve().control_points().len()
            );
            for (a, b) in a
                .curve()
                .control_points()
                .iter()
                .zip(b.curve().control_points())
            {
                assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
                assert!((a.weight() - b.weight()).abs() < 1e-12);
            }
        }
        assert_eq!(
            a.edges()[0].has_arc_representation(),
            kind == "brep_cylinder"
        );
    }
}

#[test]
fn extract_point_ranges_replay_native_controls_attributes_and_history() {
    let registry = CommandRegistry::with_builtins();
    for row in native::range_rows() {
        let (mut doc, ids) = native::setup(&row);
        let storage = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        native::select_range_grips(&mut doc, &row);
        registry
            .execute_postselected(
                &mut doc,
                &native::range_command(&row, &ids),
                Default::default(),
            )
            .unwrap();
        let mut stages = Vec::new();
        for stage in ["after", "undo", "redo"] {
            eprintln!("Replay {} {stage}", row["id"]);
            if stage != "after" {
                registry
                    .execute(&mut doc, if stage == "undo" { "Undo" } else { "Redo" })
                    .unwrap();
            }
            native::compare(&doc, &ids, &row["value"][stage]);
            for (id, original) in ids.iter().zip(&storage) {
                assert!(
                    doc.object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(original)
                );
            }
            stages.push(native::snapshot(&doc, &ids));
        }
        println!(
            "EXTRACT_POINTS_RANGE_COMMAND {}",
            serde_json::json!({"id":row["id"],"success":true,"source_storage_unchanged":true,
            "after":stages[0],"undo":stages[1],"redo":stages[2]})
        );
    }
}
