#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::Document;
#[test]
fn line_extensions_match_native_sources_geometry_domains_history_and_archives() {
    let sources = inputs::sources();
    let r = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            if v["success"] != true {
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
                "LINE_REFERENCE_COMMAND {}",
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
    assert_eq!(count, 30);
}
#[test]
fn invalid_line_extension_bindings_leave_sources_selection_and_redo_unchanged() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Line 0,0,0 10,0,0").unwrap();
    let id = d.objects().next().unwrap().id();
    r.execute(&mut d, "Point 9,9,9").unwrap();
    d.undo().unwrap();
    let before = format!("{d:?}");
    for text in [
        "Line Extension".into(),
        format!("Line Extension Curve={id} 9,0,0"),
        format!("Line Extension Curve={id} 9,0,0 10,0,0"),
        format!("Line Extension Curve={id} 9,0,0 15,0,0 Extra"),
        "Line Extension Curve=999999 9,0,0 15,0,0".into(),
    ] {
        assert!(r.execute(&mut d, &text).is_err(), "{text}");
        assert_eq!(format!("{d:?}"), before, "{text}");
    }
    assert!(d.can_redo());
}

#[test]
fn line_extension_resolves_brep_edges_without_changing_parent_topology() {
    use viboceros_document::Geometry;
    use viboceros_geometry::{Brep, Point3, Tolerance};
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let frame = CommandContext::default().construction_plane;
    let id = doc
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                frame,
                [[0., 3.], [0., 4.], [0., 5.]],
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    let parent = doc.object(id).unwrap().clone();
    let (p, q) = {
        let resolved = viboceros_command::curve_reference::resolve(
            parent.geometry(),
            Some(0),
            doc.tolerance(),
        )
        .unwrap();
        let c = resolved.curve();
        let p = c.end_point().unwrap();
        let t = c
            .evaluate_with_tangent(*c.domain().end())
            .unwrap()
            .tangent();
        (p, p.translated(t.as_vector().scaled(3.).unwrap()).unwrap())
    };
    let token = |p: Point3| {
        let [x, y, z] = p.to_array();
        format!("{x},{y},{z}")
    };
    r.execute(
        &mut doc,
        &format!("Line Extension Edge={id},0 {} {}", token(p), token(q)),
    )
    .unwrap();
    let Geometry::Line(line) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(line.start().distance_to(p).unwrap() < 1e-6);
    assert!(line.end().distance_to(q).unwrap() < 1e-6);
    doc.undo().unwrap();
    assert_eq!(doc.object(id).unwrap(), &parent);
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert!(
        parent
            .geometry_snapshot()
            .shares_storage_with(doc.object(id).unwrap().geometry_snapshot())
    );
    let before = format!("{doc:?}");
    assert!(
        r.execute(
            &mut doc,
            &format!("Line Extension Edge={id},999 w0,0,0 w1,0,0")
        )
        .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}
