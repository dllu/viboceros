//! Replay actual component callbacks and transient history state against Rhino.
use super::*;
use crate::app::tests::{point, test_app};
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use serde_json::{Value, json};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn snapshot(app: &mut VibocerosApp, ids: &[ObjectId]) -> Value {
    let mut rows = native::snapshot(&app.document, ids);
    let picks = app.component_selection.valid_picks(&app.document);
    for (object, row) in app.document.objects().zip(rows.as_array_mut().unwrap()) {
        let mut components = picks
            .iter()
            .filter(|p| p.object == object.id())
            .map(|p| {
                [
                    json!(component_kind(p.kind).unwrap().to_lowercase()),
                    json!(p.index),
                ]
            })
            .collect::<Vec<_>>();
        components.sort_by_key(|p| (p[0].as_str().unwrap().to_owned(), p[1].as_u64().unwrap()));
        row["components"] = json!(components);
    }
    rows
}
fn canonical_native(expected: &Value) -> Value {
    let mut rows = expected.clone();
    for row in rows.as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("id");
        row.as_object_mut().unwrap().remove("crc");
        row["components"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|p| (p[0].as_str().unwrap().to_owned(), p[1].as_u64().unwrap()));
        row["components"].as_array_mut().unwrap().dedup();
    }
    rows
}
#[test]
fn extract_point_component_getters_replay_native_history_and_attributes() {
    for row in native::component_rows() {
        for preselected in [row["path"] == "preselected"] {
            let mut app = test_app();
            let (document, ids) = native::setup(&row);
            app.document = document;
            let storage = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            let picks = native::component_picks(&row)
                .iter()
                .map(|p| {
                    let object = ids[p[0].as_u64().unwrap() as usize];
                    let geometry = app.document.object(object).unwrap().geometry();
                    let kind = match (p[1].as_str().unwrap(), geometry) {
                        ("vertex", Geometry::Mesh(_)) => ComponentSelectionKind::MeshVertex,
                        ("edge", Geometry::Mesh(_)) => ComponentSelectionKind::MeshEdge,
                        ("face", Geometry::Mesh(_)) => ComponentSelectionKind::MeshFace,
                        ("vertex", Geometry::SubD(_)) => ComponentSelectionKind::SubDVertex,
                        ("edge", Geometry::SubD(_)) => ComponentSelectionKind::SubDEdge,
                        ("face", Geometry::SubD(_)) => ComponentSelectionKind::SubDFace,
                        ("edge", Geometry::Brep(_) | Geometry::NurbsSurface(_)) => {
                            ComponentSelectionKind::BrepEdge
                        }
                        ("face", Geometry::Brep(_) | Geometry::NurbsSurface(_)) => {
                            ComponentSelectionKind::BrepFace
                        }
                        _ => panic!(),
                    };
                    ComponentPick {
                        object,
                        kind,
                        index: p[2].as_u64().unwrap() as usize,
                    }
                })
                .collect::<Vec<_>>();
            let whole = app.document.selected_object_ids().collect::<Vec<_>>();
            let grips = app
                .document
                .selected_control_points()
                .map(|(g, _)| g)
                .collect::<Vec<_>>();
            if !preselected {
                app.document.clear_selection();
            }
            let mut input = native::command(&row);
            if row["value"]["input"]["flat"] == true {
                input.push_str(" SubDLocation=ControlNet");
            }
            if preselected {
                for pick in picks {
                    app.accept_component_click(ComponentClick {
                        picks: vec![pick],
                        preselection: true,
                        modifiers: Default::default(),
                    });
                }
                enter(&mut app, &input);
            } else {
                enter(&mut app, &input);
                assert!(app.picking_extract_points(), "{:?}", app.command_log);
                app.document
                    .select_objects_direct(whole, SelectionMode::Add)
                    .unwrap();
                app.document
                    .select_control_points(grips, SelectionMode::Add)
                    .unwrap();
                app.accept_component_window(ComponentWindow {
                    picks,
                    preselection: false,
                    modifiers: Default::default(),
                    crossing: true,
                    inverted: false,
                });
                enter(&mut app, "");
            }
            assert!(
                app.command_line_idle(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            assert_eq!(app.document.undo_label(), Some("ExtractPt"));
            let mut stages = Vec::new();
            for stage in ["after", "undo", "redo"] {
                if stage != "after" {
                    enter(&mut app, if stage == "undo" { "Undo" } else { "Redo" });
                }
                native::compare(&app.document, &ids, &row["value"][stage]);
                for (id, original) in ids.iter().zip(&storage) {
                    assert!(
                        app.document
                            .object(*id)
                            .unwrap()
                            .geometry_snapshot()
                            .shares_storage_with(original),
                        "{} {stage}",
                        row["id"]
                    );
                }
                let actual = snapshot(&mut app, &ids);
                let expected = canonical_native(&row["value"][stage]);
                for (a, e) in actual
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(expected.as_array().unwrap())
                {
                    assert_eq!(a["components"], e["components"], "{} {stage}", row["id"]);
                }
                stages.push(actual);
            }
            for (id, original) in ids.iter().zip(storage) {
                assert!(
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(&original)
                );
            }
            println!(
                "EXTRACT_POINTS_COMPONENT_GETTER {}",
                json!({"id":row["id"],"path":if preselected {"preselected"} else {"picked"},"success":true,"source_storage_unchanged":true,"after":stages[0],"undo":stages[1],"redo":stages[2]})
            );
        }
    }
}
#[test]
fn stale_component_source_rejects_extraction_and_preserves_redo() {
    let mut app = test_app();
    let row = native::component_rows().remove(0);
    let (document, ids) = native::setup(&row);
    app.document = document;
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: ids[0],
            kind: ComponentSelectionKind::MeshFace,
            index: 0,
        }],
        preselection: true,
        modifiers: Default::default(),
    });
    app.document
        .transform_objects(
            [ids[0]],
            viboceros_geometry::AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(1., 2., 3.).unwrap(),
            ),
        )
        .unwrap();
    app.document.clear_history().unwrap();
    app.document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtractPt");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.can_redo());
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("component source changed")
    );
}

#[test]
fn extract_points_ngon_region_and_cancel_preserve_atomic_history() {
    use viboceros_geometry::{MeshFace, TriangleMesh};
    let mut app = test_app();
    let mesh = TriangleMesh::try_from_face_records(
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(4., 0., 0.),
            point(0., 3., 0.),
            point(2., 3., 0.),
            point(4., 3., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 4, 3]), MeshFace::Quad([1, 2, 5, 4])],
    )
    .unwrap();
    let ngon = mesh.ngon_from_faces(vec![0, 1]).unwrap();
    let id = app
        .document
        .add_geometry(Geometry::Mesh(mesh.try_with_ngons(vec![ngon]).unwrap()))
        .unwrap();
    app.document.clear_history().unwrap();
    app.document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    app.document.undo().unwrap();
    let source = app.document.object(id).unwrap().geometry_snapshot().clone();
    enter(&mut app, "ExtractPt");
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: id,
            kind: ComponentSelectionKind::MeshFace,
            index: 0,
        }],
        preselection: false,
        modifiers: Default::default(),
    });
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    enter(&mut app, "Cancel");
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: id,
            kind: ComponentSelectionKind::MeshFace,
            index: 0,
        }],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "ExtractPt");
    assert_eq!(app.document.objects().len(), 7, "{:?}", app.command_log);
    let actual = app
        .document
        .objects()
        .filter_map(|o| {
            if let Geometry::Point(p) = o.geometry() {
                Some(*p)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            point(0., 0., 0.),
            point(0., 3., 0.),
            point(2., 0., 0.),
            point(2., 3., 0.),
            point(4., 0., 0.),
            point(4., 3., 0.)
        ]
    );
    assert_eq!(app.document.undo_label(), Some("ExtractPt"));
    assert!(
        app.document
            .object(id)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&source)
    );
}
