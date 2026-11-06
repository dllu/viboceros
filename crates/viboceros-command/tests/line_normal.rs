#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_normal.rs"]
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
fn normal_lines_match_native_sources_geometry_domains_history_and_archives() {
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
            let command = inputs::command(&doc, &sources[id]);
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
            let path = directory.path().join("extension.3dm");
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
                "LINE_NORMAL_COMMAND {}",
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
    assert_eq!(count, 39);
}
#[test]
fn invalid_normal_bindings_do_not_change_sources_selection_or_pending_redo() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    r.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    r.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for text in [
        "Line Normal".into(),
        format!("Line Normal Curve={id} 5,0,0"),
        format!("Line Normal Curve={id} 5,0,0 5,0,0 0"),
        format!("Line Normal Face={id},0 5,0,0 5,0,0 5,3,2"),
        format!("Line Normal Edge={id},0 5,0,0 5,0,0 5,3,2"),
        format!("Line Normal IgnoreTrims=Maybe Curve={id} 5,0,0 5,0,0 5,3,2"),
        "Line Normal Surface=999999 0,0,0 0,0,0 0,0,5".into(),
    ] {
        assert!(r.execute(&mut doc, &text).is_err(), "{text}");
        assert_eq!(format!("{doc:?}"), before, "{text}");
    }
    assert!(doc.can_redo());
}
#[test]
fn face_and_edge_bindings_leave_the_parent_topology_and_storage_unchanged() {
    use viboceros_document::Geometry;
    use viboceros_geometry::{Brep, Point3, Tolerance};
    let mut doc = Document::default();
    let r = CommandRegistry::with_builtins();
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
    let parent = doc.object(id).unwrap().clone();
    doc.clear_history().unwrap();
    for (arguments, start, end) in [
        (
            format!("Face={id},5 5,5,10 0,3,4 5"),
            [0., 3., 10.],
            [0., 3., 15.],
        ),
        (
            format!("Edge={id},0 5,0,0 5,0,0 7,4,3"),
            [5., 0., 0.],
            [5., 4., 3.],
        ),
    ] {
        r.execute(&mut doc, &format!("Line Normal {arguments}"))
            .unwrap();
        let Geometry::Line(line) = doc.objects().last().unwrap().geometry() else {
            panic!()
        };
        assert!(
            line.start()
                .distance_to(Point3::try_from(start).unwrap())
                .unwrap()
                < 1e-12
        );
        assert!(
            line.end()
                .distance_to(Point3::try_from(end).unwrap())
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
}
