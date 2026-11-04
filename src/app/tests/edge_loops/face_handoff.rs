use super::*;
use std::collections::BTreeSet;

fn partial_ngon() -> Value {
    faces::data("component_pattern_holdouts").1["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "mesh_ngon_grid_0")
        .unwrap()["value"]
        .clone()
}
#[test]
fn face_loop_outputs_feed_exact_faces_to_subdivide_and_extract_points() {
    let v = partial_ngon();
    for scripted in [false, true] {
        let (mut app, id) = fixture(&v);
        let source = app.document.object(id).unwrap().clone();
        let edge = v["after"]["ordinals"][0].as_u64().unwrap() as usize;
        if scripted {
            enter(&mut app, &format!("SelFaceLoop Edges={edge} Object={id}"));
        } else {
            enter(&mut app, "SelFaceLoop");
            app.accept_component_click(ComponentClick {
                picks: vec![pick(id, true, edge)],
                preselection: false,
                modifiers: egui::Modifiers::NONE,
            });
            enter(&mut app, "");
        }
        let expected = indices(&v["after"]["face_ordinals"]);
        assert_eq!(expected, [0, 1, 2, 3]);
        let raw = app.component_selection.raw_face_picks(&app.document);
        assert_eq!(raw.iter().map(|p| p.index).collect::<Vec<_>>(), expected);
        // Repeating the standalone command, or selecting an edge ring, must
        // retain the meaning of the already selected partial n-gon faces.
        for command in ["SelFaceLoop", "SelEdgeRing"] {
            enter(&mut app, command);
            assert_eq!(app.component_selection.raw_face_picks(&app.document), raw);
        }
        let args = app.extract_point_component_arguments().unwrap();
        let faces = args
            .split_whitespace()
            .filter_map(|s| {
                s.split_once(",Face,")
                    .map(|(_, n)| n.parse::<usize>().unwrap())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(faces, expected.iter().copied().collect());
        let Geometry::Mesh(mesh) = source.geometry() else {
            panic!()
        };
        let wanted = mesh.subdivided_faces(&expected).unwrap();
        enter(&mut app, "SubDivide");
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &Geometry::Mesh(wanted)
        );
        assert!(
            app.component_selection
                .raw_face_picks(&app.document)
                .is_empty()
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id), Some(&source));
    }
}
#[test]
fn face_loop_selector_failures_and_changed_sources_preserve_model_history() {
    let v = partial_ngon();
    let (mut app, id) = fixture(&v);
    enter(&mut app, "Point 40,50,60");
    enter(&mut app, "Undo");
    enter(&mut app, &format!("_SelFaceLoop Edges=5 Object={id}"));
    let before = app
        .component_selection
        .checked_picks(&app.document)
        .unwrap();
    let raw = app.component_selection.raw_face_picks(&app.document);
    let source = app.document.object(id).unwrap().clone();
    for args in [
        "Edges=999",
        "Edges=5,999",
        "Edges=x",
        "Edges=5 Edge=1",
        "Faces=1",
    ] {
        assert!(!app.try_execute_command(&format!("SelFaceLoop {args} Object={id}")));
        assert_eq!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap(),
            before
        );
        assert_eq!(app.component_selection.raw_face_picks(&app.document), raw);
        assert_eq!(app.document.object(id), Some(&source));
        assert!(app.document.can_redo());
    }
    // A scripted replacement can bypass the UI selection wrapper. Raw-face
    // metadata must never describe components absent from the shared selection.
    assert!(app.try_execute_command(&format!("SelEdgeRing Edges=5 Object={id}")));
    assert!(
        app.component_selection
            .raw_face_picks(&app.document)
            .is_empty()
    );
    for hidden in [false, true] {
        let (mut app, id) = fixture(&v);
        enter(&mut app, "SelFaceLoop");
        app.accept_component_click(ComponentClick {
            picks: vec![pick(id, true, 5)],
            preselection: false,
            modifiers: egui::Modifiers::NONE,
        });
        assert!(
            !app.edge_pattern_prompt
                .as_ref()
                .unwrap()
                .preview()
                .is_empty()
        );
        if hidden {
            app.document.set_objects_visibility([id], false).unwrap();
        } else {
            app.document
                .replace_object_geometries([(id, Geometry::Point(point(1., 2., 3.)))])
                .unwrap();
        }
        let objects = app.document.objects().cloned().collect::<Vec<_>>();
        let undo = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "");
        assert!(app.edge_pattern_prompt.is_none());
        assert!(app.commands.component_selection().is_empty());
        assert!(
            app.component_selection
                .raw_face_picks(&app.document)
                .is_empty()
        );
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), undo.as_deref());
        assert!(app.command_log.back().unwrap().contains("source"));
    }
}
