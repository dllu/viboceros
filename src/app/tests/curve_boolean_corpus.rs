//! Native workflows through UI input, followed by output archive admission.
use super::boolean_intersection::enter;
use super::*;
#[path = "../../../crates/viboceros-oracle/tests/support/curve_boolean.rs"]
mod comparison;
use super::oracle_observation as observation;
#[test]
fn curve_boolean_native_corpus_uses_prompt_picks_cancel_history_and_3dm() {
    use viboceros_oracle::curve_boolean::{Fixture, snapshot};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("curve-boolean.3dm");
    for (op, native) in comparison::rows(&base, observation::read) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let (doc, ids) = f.setup(tolerance).unwrap();
        let temporary = f
            .temporary_ids(&ids)
            .into_iter()
            .map(|id| doc.object(id).unwrap().clone())
            .collect::<Vec<_>>();
        let mut app = test_app();
        app.document = doc;
        app.viewports[app.active_viewport]
            .set_construction_plane(f.context(tolerance).unwrap().construction_plane);
        let before = snapshot(&app.document, &ids).unwrap();
        let input = f.command().unwrap();
        let start = input
            .split_whitespace()
            .filter(|w| w.contains('='))
            .collect::<Vec<_>>()
            .join(" ");
        enter(&mut app, &format!("CurveBoolean {start}"));
        if !f.preselect {
            if let Some(selections) = &f.selections {
                if let Some(phase) = &f.cancel_subcurve {
                    enter(&mut app, "SubCrv");
                    if phase != "source" {
                        enter(&mut app, &ids[selections[0].source].to_string());
                    }
                    if phase == "end" {
                        app.apply_drafting_point(
                            Point3::try_from(selections[0].range.unwrap()[0]).unwrap(),
                        );
                    }
                    app.cancel_current_prompt_or_selection();
                    assert!(app.curve_boolean_prompt.as_ref().unwrap().selecting());
                    app.cancel_current_prompt_or_selection();
                } else {
                    for selection in selections {
                        if let Some(points) = selection.range {
                            enter(&mut app, "SubCrv");
                            if let Some(edge) = selection.edge {
                                assert!(app.picking_subcurve_edge());
                                assert!(app.pick_subcurve_edge(crate::viewport::EdgePick {
                                    object: ids[selection.source],
                                    edge,
                                }));
                            } else {
                                enter(&mut app, &ids[selection.source].to_string());
                            }
                            app.apply_drafting_point(Point3::try_from(points[0]).unwrap());
                            if let Some(length) = selection.length {
                                enter(&mut app, &length.to_string());
                            }
                            if selection.confirmation.unwrap_or(true) {
                                app.apply_drafting_point(Point3::try_from(points[1]).unwrap());
                            }
                        } else {
                            enter(&mut app, &ids[selection.source].to_string());
                        }
                    }
                }
            } else {
                for id in &ids {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(*id),
                        mode: SelectionMode::Replace,
                    });
                }
            }
            if f.cancel_subcurve.is_none() {
                enter(&mut app, "");
            }
        }
        let enclosed = app
            .curve_boolean_prompt
            .as_ref()
            .is_some_and(|p| !p.selecting());
        if enclosed {
            if f.all_regions {
                enter(&mut app, "AllRegions");
            }
            for point in &f.points {
                app.apply_drafting_point(Point3::try_from(*point).unwrap());
            }
            // Every preview is isolated from document geometry and Undo.
            assert!(!app.document.can_undo(), "{}", op["id"]);
            assert_eq!(
                snapshot(&app.document, &ids).unwrap()["objects"]
                    .as_array()
                    .unwrap()
                    .len(),
                before["objects"].as_array().unwrap().len()
            );
            if f.cancel {
                app.cancel_current_prompt_or_selection();
            } else {
                enter(&mut app, "");
            }
        } else if app.curve_boolean_prompt.is_some() {
            app.cancel_current_prompt_or_selection();
        }
        let after = snapshot(&app.document, &ids).unwrap();
        for original in &temporary {
            let current = app.document.object(original.id()).unwrap();
            assert_eq!(current, original, "{} temporary parent", op["id"]);
            assert!(
                current
                    .geometry_snapshot()
                    .shares_storage_with(original.geometry_snapshot())
            );
        }
        if app.document.can_undo() {
            enter(&mut app, "Undo");
        }
        let undo = snapshot(&app.document, &ids).unwrap();
        if app.document.can_redo() {
            enter(&mut app, "Redo");
        }
        let redo = snapshot(&app.document, &ids).unwrap();
        let actual = serde_json::json!({"success":enclosed&&!f.cancel,"before":before,"after":after,"undo":undo,"redo":redo});
        let report =
            comparison::compare(&actual, &native).unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        // The common classifier records the same explicit native geometry gaps
        // for both the registry and the UI; it never widens the model gate.
        comparison::qualify(op["id"].as_str().unwrap(), report).unwrap();
        app.commands
            .execute(&mut app.document, &format!("SaveAs {}", path.display()))
            .unwrap();
        let model = viboceros_io::read_3dm_file(&path, tolerance).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        assert_eq!(model.objects.len(), app.document.objects().len());
        for saved in &model.objects {
            if let viboceros_io::ThreeDmGeometry::Brep(b) = &saved.geometry {
                b.tessellate_with_normals(8, tolerance)
                    .unwrap_or_else(|e| panic!("{} archived B-rep: {e:?}", op["id"]));
            }
        }
        let mut restored = Document::new(tolerance);
        app.commands
            .execute(&mut restored, &format!("Open3dm {}", path.display()))
            .unwrap();
        restored
            .select_objects_direct(app.document.selected_object_ids(), SelectionMode::Replace)
            .unwrap();
        let archived = snapshot(&restored, &ids).unwrap();
        let roundtrip = serde_json::json!({"success":true,"before":archived,"after":archived,"undo":archived,"redo":archived});
        let original = snapshot(&app.document, &ids).unwrap();
        let original = serde_json::json!({"success":true,"before":original,"after":original,"undo":original,"redo":original});
        let archive_error = comparison::compare(&roundtrip, &original)
            .unwrap_or_else(|e| panic!("{} archive: {e}", op["id"]));
        assert!(
            archive_error.model_error <= 1e-6 && !archive_error.topology_gap,
            "{}: {archive_error:?}",
            op["id"]
        );
        println!(
            "CURVE_BOOLEAN_APP {}",
            serde_json::json!({"id":op["id"],"model_error":report.model_error,"area_error":report.area_error,"relative_area_error":report.relative_area_error,"topology_gap":report.topology_gap,"archive_objects":model.objects.len()})
        );
    }
}
