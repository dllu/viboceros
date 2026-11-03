use super::*;
#[test]
fn inset_whole_planar_surfaces_and_separate_defaults_replay_atomically() {
    let surface = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [8., 0., 0.], [8., 6., 0.], [0., 6., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    for nurbs in [false, true] {
        let mut doc = Document::default();
        let geometry = if nurbs {
            Geometry::NurbsSurface(surface.clone())
        } else {
            Geometry::Brep(Brep::try_surface_face(surface.clone(), doc.tolerance()).unwrap())
        };
        let id = doc.add_geometry(geometry).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let registry = CommandRegistry::with_builtins();
        assert_eq!(registry.inset_surface_options().distance, 1.);
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        registry
            .execute(&mut doc, "Inset .3 CreaseCorners=Yes Loose=On")
            .unwrap();
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(b.faces().len(), 5);
        assert!(doc.is_selected(id));
        assert_eq!(registry.inset_surface_options().distance, 0.3);
        assert!(registry.inset_surface_options().crease_corners);
        assert!(!registry.inset_surface_options().loose);
        assert_eq!(registry.inset_options(false).distance, 0.1);
        assert_eq!(registry.inset_options(true).distance, 1.);
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        for bad in [
            "Inset 0",
            "Inset .7 CreaseCorners=Maybe",
            "Inset .7 CreaseCorners=Yes CreaseCorners=No",
            "Inset .7 Split=No",
            "Inset .7 Loose=No",
            "Inset .7 Loose=Yes",
            "Inset .7 Loose=On Loose=Off",
            "Inset .7 Faces=1",
            "Inset 5",
        ] {
            assert!(registry.execute(&mut doc, bad).is_err(), "{bad}");
            assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
            assert_eq!(doc.redo_label(), Some("Inset"));
            assert_eq!(registry.inset_surface_options().distance, 0.3);
        }
    }
}
fn fixture(mesh: bool) -> (Document, ObjectId) {
    let mut doc = Document::default();
    let points = [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect();
    let geometry = if mesh {
        Geometry::Mesh(
            TriangleMesh::try_from_face_records(
                points,
                vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
            )
            .unwrap(),
        )
    } else {
        Geometry::SubD(
            viboceros_geometry::SubD::try_from_polygons(points, vec![vec![0, 1, 2, 3]]).unwrap(),
        )
    };
    let id = doc.add_geometry(geometry).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
#[test]
fn inset_explicit_face_commands_preserve_identity_and_replay_history() {
    for mesh in [false, true] {
        let (mut doc, id) = fixture(mesh);
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        let registry = CommandRegistry::with_builtins();
        assert_eq!(
            registry.inset_options(mesh).distance,
            if mesh { 1. } else { 0.1 }
        );
        registry
            .execute(&mut doc, "Inset .3 Faces=0 Mode=Single")
            .unwrap();
        let after = doc.object(id).unwrap().geometry_snapshot().clone();
        assert_ne!(before, after);
        assert!(doc.is_selected(id));
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(doc.undo_label(), Some("Inset"));
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        doc.redo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &after);
    }
}
#[test]
fn inset_invalid_and_mixed_inputs_preserve_geometry_redo_and_defaults() {
    for mesh in [false, true] {
        let (mut doc, id) = fixture(mesh);
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 30,20,10").unwrap();
        doc.undo().unwrap();
        let source = doc.object(id).unwrap().geometry_snapshot().clone();
        for input in [
            "Inset .3",
            "Inset 0 Faces=All",
            "Inset -1 Faces=0",
            "Inset NaN Faces=0",
            "Inset .3 Faces=999",
            "Inset .3 Faces=0 Loose=On",
            "Inset .3 Faces=-1",
            "Inset .3 Faces=0 Mode=Other",
            "Inset .3 Faces=0 Mode=Group Mode=Single",
            "Inset .3 Faces=0 Distance=.7",
        ] {
            let defaults = registry.inset_options(mesh);
            assert!(registry.execute(&mut doc, input).is_err(), "{input}");
            assert_eq!(registry.inset_options(mesh), defaults);
            assert_eq!(doc.redo_label(), Some("Point"));
            assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &source);
        }
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
            .unwrap();
        doc.select_objects_direct([id, point], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        assert!(registry.execute(&mut doc, "Inset .3 Faces=0").is_err());
        assert!(!doc.can_undo());
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &source);
    }
}
#[test]
fn inset_stale_attributes_groups_selection_tolerance_and_geometry_reject_commit() {
    for change in 0..5 {
        let (mut doc, id) = fixture(false);
        let plan = Plan::prepare(&doc, [(id, 0)], Options::default()).unwrap();
        match change {
            0 => {
                doc.clear_selection();
            }
            1 => {
                doc.set_object_names([(id, Some("Changed".into()))])
                    .unwrap();
            }
            2 => {
                doc.add_group(Some("Changed".into()), [id]).unwrap();
            }
            3 => {
                doc.set_tolerance(Tolerance::try_new(0.002, 1e-6, 0.01).unwrap());
            }
            _ => {
                doc.replace_object_geometries([(
                    id,
                    Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()),
                )])
                .unwrap();
            }
        }
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        let label = doc.undo_label().map(str::to_owned);
        assert!(matches!(
            plan.commit(&mut doc),
            Err(CommandError::InsetStale)
        ));
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}

#[test]
fn inset_group_on_a_closed_whole_net_is_a_noop_and_keeps_pending_redo() {
    let mut doc = Document::default();
    let source = viboceros_geometry::SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [0., 0., 5.],
            [4., 0., 5.],
            [4., 3., 5.],
            [0., 3., 5.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::SubD(source)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 20,30,40").unwrap();
    doc.undo().unwrap();
    let before = doc.object(id).unwrap().geometry_snapshot().clone();
    registry
        .execute(&mut doc, "Inset .3 Faces=All Mode=Group")
        .unwrap();
    assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
    assert_eq!(doc.redo_label(), Some("Point"));
}
