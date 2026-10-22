use std::collections::BTreeSet;
use viboceros_command::{
    CommandError, CommandRegistry,
    vertex_seams::{VertexSeamEdit, VertexSeamMode, VertexSeamTarget},
};
use viboceros_document::{ControlPointId, Document, Geometry, GripMode, SelectionMode};
use viboceros_geometry::{Point3, SubD, Tolerance};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn setup() -> (Document, viboceros_document::ObjectId) {
    let points = vec![
        p(0., 0., 0.),
        p(4., 0., 0.),
        p(4., 3., 0.),
        p(0., 3., 0.),
        p(0., 0., 2.),
        p(4., 0., 2.),
        p(4., 3., 2.),
        p(0., 3., 2.),
    ];
    let faces = vec![
        vec![0, 3, 2, 1],
        vec![0, 1, 5, 4],
        vec![1, 2, 6, 5],
        vec![2, 3, 7, 6],
        vec![3, 0, 4, 7],
        vec![4, 5, 6, 7],
    ];
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::SubD(
            SubD::try_from_polygons(points, faces).unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
#[test]
fn invalid_or_stale_vertex_batches_do_not_mutate_geometry_or_history() {
    let registry = CommandRegistry::with_builtins();
    for mutation in ["index", "type", "geometry", "tolerance", "lock"] {
        let (mut doc, id) = setup();
        let target = VertexSeamTarget {
            object: id,
            index: 0,
        };
        let plan = VertexSeamEdit::prepare(
            &doc,
            VertexSeamMode::Unweld {
                modify_normals: true,
            },
            [target],
        )
        .unwrap();
        match mutation {
            "index" => {
                assert!(matches!(
                    VertexSeamEdit::prepare(
                        &doc,
                        VertexSeamMode::Weld,
                        [
                            target,
                            VertexSeamTarget {
                                index: usize::MAX,
                                ..target
                            }
                        ]
                    ),
                    Err(CommandError::WeldVertexIndexOutOfRange { .. })
                ));
            }
            "type" => {
                let point = doc.add_geometry(Geometry::Point(p(7., 8., 9.))).unwrap();
                doc.clear_history().unwrap();
                assert!(matches!(
                    VertexSeamEdit::prepare(
                        &doc,
                        VertexSeamMode::Weld,
                        [
                            target,
                            VertexSeamTarget {
                                object: point,
                                index: 0
                            }
                        ]
                    ),
                    Err(CommandError::UnsupportedWeldVerticesGeometry)
                ));
            }
            "geometry" => {
                doc.replace_object_geometries([(id, Geometry::Point(p(7., 8., 9.)))])
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
        if !matches!(mutation, "index" | "type") {
            assert!(matches!(
                plan.commit(&mut doc),
                Err(CommandError::VertexSeamStale)
            ));
        }
        assert_eq!(doc.object(id).unwrap().geometry(), &before);
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}
#[test]
fn scripted_selectors_points_grips_and_noops_share_one_subd_policy() {
    let registry = CommandRegistry::with_builtins();
    for edit in [false, true] {
        let (mut doc, id) = setup();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(&mut doc, if edit { "EditPtOn" } else { "PointsOn" })
            .unwrap();
        assert_eq!(
            doc.grip_mode(id),
            Some(if edit {
                GripMode::Edit
            } else {
                GripMode::Control
            })
        );
        let grip = ControlPointId {
            object: id,
            index: 0,
        };
        doc.select_control_points([grip], SelectionMode::Replace)
            .unwrap();
        let original = doc.object(id).unwrap().geometry().clone();
        registry
            .execute(&mut doc, "UnweldVertex ModifyNormals=No")
            .unwrap();
        assert_ne!(doc.object(id).unwrap().geometry(), &original);
        assert_eq!(
            doc.selected_control_points()
                .map(|(g, _)| g)
                .collect::<Vec<_>>(),
            vec![grip]
        );
        registry.execute(&mut doc, "WeldVertex").unwrap();
        let Geometry::SubD(subd) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(subd.vertex_tags().iter().all(|&t| t == 1));
        doc.undo().unwrap();
        let redo = doc.redo_label().map(str::to_owned);
        registry
            .execute(&mut doc, "UnweldVertex Vertices=0")
            .unwrap_err(); // no parent is selected
        VertexSeamEdit::prepare(
            &doc,
            VertexSeamMode::Unweld {
                modify_normals: true,
            },
            [VertexSeamTarget {
                object: id,
                index: 0,
            }],
        )
        .unwrap()
        .commit(&mut doc)
        .unwrap(); // already a corner
        assert_eq!(doc.redo_label(), redo.as_deref());
    }
    for surface in [false, true] {
        let (mut doc, id) = setup();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let point = if surface {
            s.surface_points()[0]
        } else {
            s.control_points()[0]
        };
        let c = point.to_array();
        registry
            .execute(
                &mut doc,
                &format!("UnweldVertex {},{},{}", c[0], c[1], c[2]),
            )
            .unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(&mut doc, "WeldVertices Vertices=All")
            .unwrap();
        let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(s.vertex_tags().iter().all(|&t| t == 1));
    }
}
#[test]
fn surface_edit_grips_use_limit_locations_and_the_existing_interpolation_solver() {
    let (mut doc, id) = setup();
    doc.enable_edit_points([id]).unwrap();
    let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    let original = s.surface_points().to_vec();
    let Geometry::SubD(edited) = doc
        .object(id)
        .unwrap()
        .geometry()
        .with_transformed_grips_in_mode(
            &BTreeSet::from([0]),
            viboceros_geometry::AffineTransform3::from_translation(
                viboceros_geometry::Vector3::try_new(0.1, 0.2, 0.3).unwrap(),
            ),
            Tolerance::default(),
            GripMode::Edit,
        )
        .unwrap()
    else {
        panic!()
    };
    let point = original[0];
    assert!(
        edited.surface_points()[0]
            .distance_to(p(point.x() + 0.1, point.y() + 0.2, point.z() + 0.3))
            .unwrap()
            <= 1e-6
    );
    assert_eq!(&edited.control_points()[1..], &s.control_points()[1..]);
}
