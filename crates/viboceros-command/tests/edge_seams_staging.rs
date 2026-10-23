use viboceros_command::{
    CommandError, CommandRegistry,
    component_seams::{ComponentSeamEdit, EdgeSeamMode, SeamTarget},
};
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{Point3, SubD};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn setup() -> (Document, ObjectId, usize) {
    let mut doc = Document::default();
    let s = SubD::try_from_polygons(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(4., 0., 1.),
            p(0., 3., 0.),
            p(2., 3., 0.),
            p(4., 3., 1.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap();
    let edge = s
        .edges()
        .iter()
        .position(|&e| e == [1, 4] || e == [4, 1])
        .unwrap();
    let id = doc.add_geometry(Geometry::SubD(s)).unwrap();
    doc.clear_history().unwrap();
    (doc, id, edge)
}
#[test]
fn invalid_and_stale_batches_leave_geometry_history_and_redo_intact() {
    let registry = CommandRegistry::with_builtins();
    for change in ["index", "type", "geometry", "tolerance", "lock"] {
        let (mut doc, id, edge) = setup();
        let target = SeamTarget {
            object: id,
            index: edge,
        };
        let plan = ComponentSeamEdit::prepare(
            &doc,
            EdgeSeamMode::Unweld {
                modify_normals: false,
            },
            [target],
        )
        .unwrap();
        match change {
            "index" => {
                assert!(matches!(
                    ComponentSeamEdit::prepare(
                        &doc,
                        EdgeSeamMode::Weld,
                        [
                            target,
                            SeamTarget {
                                index: usize::MAX,
                                ..target
                            }
                        ]
                    ),
                    Err(CommandError::WeldEdgeIndexOutOfRange { .. })
                ));
            }
            "type" => {
                let point = doc.add_geometry(Geometry::Point(p(8., 9., 10.))).unwrap();
                doc.clear_history().unwrap();
                assert!(matches!(
                    ComponentSeamEdit::prepare(
                        &doc,
                        EdgeSeamMode::Weld,
                        [
                            target,
                            SeamTarget {
                                object: point,
                                index: 0
                            }
                        ]
                    ),
                    Err(CommandError::UnsupportedWeldEdgeGeometry)
                ));
            }
            "geometry" => {
                doc.replace_object_geometries([(id, Geometry::Point(p(1., 2., 3.)))])
                    .unwrap();
            }
            "tolerance" => {
                registry
                    .execute(&mut doc, "Tolerance Absolute=0.02")
                    .unwrap();
            }
            _ => {
                doc.select_object(id, SelectionMode::Replace).unwrap();
                registry.execute(&mut doc, "Lock").unwrap();
            }
        }
        let before = doc.object(id).unwrap().geometry().clone();
        let label = doc.undo_label().map(str::to_owned);
        let redo = doc.redo_label().map(str::to_owned);
        if !matches!(change, "index" | "type") {
            assert!(matches!(
                plan.commit(&mut doc),
                Err(CommandError::EdgeSeamStale)
            ));
        }
        assert_eq!(doc.object(id).unwrap().geometry(), &before);
        assert_eq!(doc.undo_label(), label.as_deref());
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
}
#[test]
fn scripts_share_point_index_all_and_alias_admission() {
    let registry = CommandRegistry::with_builtins();
    for method in ["index", "all", "point", "control_point"] {
        let (mut doc, id, edge) = setup();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let original = doc.object(id).unwrap().geometry().clone();
        let command = match method {
            "all" => "UnweldMeshEdge EdgeIndices All ModifyNormals No".into(),
            "point" => {
                let Geometry::SubD(s) = &original else {
                    panic!()
                };
                let points = s.display_edge_polylines().unwrap();
                let [x, y, z] = points[edge][points[edge].len() / 2].to_array();
                format!("UnweldEdge {x},{y},{z}")
            }
            "control_point" => {
                doc.enable_control_points([id]).unwrap();
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
                "UnweldEdge 2,1.5,0".into()
            }
            _ => format!("UnweldEdge Edges={edge}"),
        };
        registry.execute(&mut doc, &command).unwrap();
        let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(s.control_points().len(), 8);
        assert_eq!(s.edges().len(), 8);
        assert_eq!(doc.undo_label(), Some("UnweldEdge"));
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut doc, "WeldMeshEdge Edges=All")
            .unwrap();
        let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(s.control_points().len(), 6);
        assert_eq!(s.edges().len(), 7);
        doc.undo().unwrap();
        let redo = doc.redo_label().map(str::to_owned);
        ComponentSeamEdit::prepare(
            &doc,
            EdgeSeamMode::Unweld {
                modify_normals: true,
            },
            [SeamTarget {
                object: id,
                index: edge,
            }],
        )
        .unwrap()
        .commit(&mut doc)
        .unwrap();
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
}
#[test]
fn selecting_the_duplicate_weld_side_keeps_its_identity_and_maps_original_edges() {
    let (mut doc, id, edge) = setup();
    ComponentSeamEdit::prepare(
        &doc,
        EdgeSeamMode::Unweld {
            modify_normals: true,
        },
        [SeamTarget {
            object: id,
            index: edge,
        }],
    )
    .unwrap()
    .commit(&mut doc)
    .unwrap();
    let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    let cut = s.clone();
    let output = cut.try_weld_edges_mapped(&[7]).unwrap();
    assert_eq!(output.edge_map[edge], 6);
    assert_eq!(output.edge_map[7], 6);
    assert_eq!(output.subd.control_points()[4], cut.control_points()[6]);
    assert_eq!(output.subd.control_points()[5], cut.control_points()[7]);
    let out = ComponentSeamEdit::prepare(
        &doc,
        EdgeSeamMode::Weld,
        [SeamTarget {
            object: id,
            index: 7,
        }],
    )
    .unwrap()
    .commit(&mut doc)
    .unwrap();
    assert_eq!(
        out.retained_edges,
        vec![SeamTarget {
            object: id,
            index: 6
        }]
    );
}
