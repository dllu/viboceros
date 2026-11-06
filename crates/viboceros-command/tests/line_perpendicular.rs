#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_perpendicular.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::Document;
#[test]
fn perpendicular_lines_match_native_geometry_selection_history_and_archives() {
    let sources = inputs::sources();
    let r = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            if v["success"] != true || inputs::diagnostic(id) {
                continue;
            }
            let mut doc = inputs::setup(v, &sources[id]);
            let parents = doc.objects().cloned().collect::<Vec<_>>();
            let command = inputs::command(&doc, v, &sources[id]);
            r.execute_in_context(
                &mut doc,
                &command,
                CommandContext {
                    construction_plane: native::frame(v),
                },
            )
            .unwrap_or_else(|e| panic!("{id}: {e}"));
            let mut error = replay::check(&doc, &v["after"], false);
            assert_eq!(doc.undo_label(), Some("Line"));
            doc.undo().unwrap();
            error = error.max(replay::check(&doc, &v["undo"], false));
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            error = error.max(replay::check(&doc, &v["redo"], false));
            let path = directory.path().join("perpendicular.3dm");
            r.execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(doc.tolerance());
            r.execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(replay::check(&restored, &v["after"], true));
            for parent in parents {
                let now = doc.object(parent.id()).unwrap();
                assert_eq!(now, &parent);
                assert!(
                    parent
                        .geometry_snapshot()
                        .shares_storage_with(now.geometry_snapshot())
                );
            }
            println!(
                "LINE_PERPENDICULAR_COMMAND {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            assert!(
                error <= 1e-6,
                "{id}: {error:e}; {:?}",
                doc.objects().map(|o| o.geometry()).collect::<Vec<_>>()
            );
            count += 1;
        }
    }
    assert_eq!(count, 48);
}
#[test]
fn invalid_perpendicular_inputs_leave_selection_sources_and_pending_redo_unchanged() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    r.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    r.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for text in [
        "Line Perpendicular".into(),
        "Line Perpendicular Curve=999999 5,0,0 7,4,3".into(),
        format!("Line Perpendicular Curve={id} 5,0,0 NaN 7,4,3"),
        format!("Line Perpendicular Curve={id} 5,0,0 0 7,4,3"),
        format!("Line Perpendicular Planar=Maybe Curve={id} 5,0,0 7,4,3"),
        format!("Line Perpendicular Face={id},0 5,0,0 7,4,3"),
    ] {
        assert!(r.execute(&mut doc, &text).is_err(), "{text}");
        assert_eq!(format!("{doc:?}"), before, "{text}");
    }
    assert!(doc.can_redo());
}
#[test]
fn a_perpendicular_edge_binding_keeps_its_parent_topology_and_geometry_storage() {
    use viboceros_document::Geometry;
    use viboceros_geometry::{Brep, Point3, Tolerance};
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Brep(
            Brep::try_command_box(
                CommandContext::default().construction_plane,
                [[0., 10.]; 3],
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    let parent = doc.object(id).unwrap().clone();
    r.execute(
        &mut doc,
        &format!("Line Perpendicular Edge={id},0 5,0,0 7,4,3"),
    )
    .unwrap();
    let Geometry::Line(line) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(
        line.start()
            .distance_to(Point3::try_from([7., 0., 0.]).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(
        line.end()
            .distance_to(Point3::try_from([7., 4., 3.]).unwrap())
            .unwrap()
            < 1e-12
    );
    assert_eq!(doc.object(id).unwrap(), &parent);
    assert!(
        parent
            .geometry_snapshot()
            .shares_storage_with(doc.object(id).unwrap().geometry_snapshot())
    );
    doc.undo().unwrap();
    assert!(!doc.can_undo());
    assert_eq!(doc.objects().len(), 1);
}
