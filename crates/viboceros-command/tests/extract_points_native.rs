#[path = "support/extract_points.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn extract_points_selected_grips_replay_native_locations_metadata_and_history() {
    let registry = CommandRegistry::with_builtins();
    let rows = native::rows();
    assert_eq!(rows.len(), 63);
    for row in rows {
        let label = row["id"].as_str().unwrap();
        eprintln!("Replay {label}");
        let (mut doc, sources) = native::setup(&row);
        let storage = sources
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        if row["path"] == "picked" {
            registry
                .execute_postselected(&mut doc, &native::command(&row), Default::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, &native::command(&row)).unwrap();
        }
        native::compare(&doc, &sources, &row["value"]["after"]);
        let after = native::snapshot(&doc, &sources);
        assert_eq!(doc.undo_label(), Some("ExtractPt"), "{label}");
        registry.execute(&mut doc, "Undo").unwrap();
        native::compare(&doc, &sources, &row["value"]["undo"]);
        let undo = native::snapshot(&doc, &sources);
        registry.execute(&mut doc, "Redo").unwrap();
        native::compare(&doc, &sources, &row["value"]["redo"]);
        let redo = native::snapshot(&doc, &sources);
        for (id, original) in sources.iter().zip(&storage) {
            assert!(
                doc.object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(original),
                "{label}"
            );
        }
        println!(
            "EXTRACT_POINTS_GRIPS_COMMAND {}",
            serde_json::json!({"id":label,"path":row["path"],"success":true,"source_storage_unchanged":true,"after":after,"undo":undo,"redo":redo})
        );
    }
}

#[test]
fn extract_points_components_replay_native_locations_metadata_and_history() {
    let registry = CommandRegistry::with_builtins();
    let rows = native::component_rows();
    for row in rows {
        let label = row["id"].as_str().unwrap();
        eprintln!("Replay {label}");
        let (mut doc, sources) = native::setup(&row);
        let storage = sources
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        let command = native::component_command(&row, &sources);
        if row["path"] == "picked" {
            registry
                .execute_postselected(&mut doc, &command, Default::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, &command).unwrap();
        }
        let mut snapshots = Vec::new();
        for stage in ["after", "undo", "redo"] {
            if stage == "undo" {
                registry.execute(&mut doc, "Undo").unwrap();
            }
            if stage == "redo" {
                registry.execute(&mut doc, "Redo").unwrap();
            }
            native::compare(&doc, &sources, &row["value"][stage]);
            for (id, original) in sources.iter().zip(&storage) {
                assert!(
                    doc.object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(original),
                    "{label} {stage}"
                );
            }
            snapshots.push(native::snapshot(&doc, &sources));
        }
        for (id, original) in sources.iter().zip(&storage) {
            assert!(
                doc.object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(original)
            );
        }
        println!(
            "EXTRACT_POINTS_COMPONENT_COMMAND {}",
            serde_json::json!({"id":label,"success":true,"source_storage_unchanged":true,"after":snapshots[0],"undo":snapshots[1],"redo":snapshots[2]})
        );
    }
}

#[test]
fn subd_edge_forms_match_independent_native_profiles() {
    for row in native::edge_form_rows() {
        let (doc, ids) = native::setup(&row);
        let viboceros_document::Geometry::SubD(subd) = doc.object(ids[0]).unwrap().geometry()
        else {
            panic!()
        };
        let indices = (0..subd.edges().len()).collect::<Vec<_>>();
        let curves = subd.edge_nurbs_curves(&indices).unwrap();
        let expected = row["value"]["curve_profile"].as_array().unwrap();
        assert_eq!(curves.len(), expected.len());
        for (index, (curve, native)) in curves.iter().zip(expected).enumerate() {
            let points = curve
                .control_points()
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>();
            let reference = native["controls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    viboceros_geometry::Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(p.clone()).unwrap(),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let error = points
                .iter()
                .zip(&reference)
                .map(|(a, b)| a.distance_to(*b).unwrap())
                .fold(0., f64::max);
            eprintln!(
                "{} edge {index}: actual {} expected {} max {error}",
                row["id"],
                points.len(),
                reference.len()
            );
            assert_eq!(points.len(), reference.len());
            assert!(error < 1e-7);
            let domain = curve.domain();
            let samples = (0..=16)
                .map(|i| {
                    curve
                        .evaluate(
                            *domain.start() + (*domain.end() - *domain.start()) * i as f64 / 16.,
                        )
                        .unwrap()
                        .to_array()
                })
                .collect::<Vec<_>>();
            println!(
                "EXTRACT_POINTS_EDGE_FORM {}",
                serde_json::json!({"id":row["id"],"edge":index,"degree":curve.degree(),"controls":points.iter().map(|p| p.to_array()).collect::<Vec<_>>(),"knots":&curve.knots()[1..curve.knots().len()-1],"samples":samples})
            );
        }
    }
}
