use super::*;

fn source(doc: &mut Document) -> ObjectId {
    doc.add_geometry(Geometry::Brep(
        Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., 2.], [0., 3.], [0., 4.]],
            doc.tolerance(),
        )
        .unwrap(),
    ))
    .unwrap()
}

#[test]
fn silhouette_rejects_bad_options_without_geometry_or_history_changes() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = source(&mut doc);
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    for command in [
        "Silhouette InternalEdges=Maybe",
        "Silhouette Direction=0,0,0",
        "Silhouette Direction=NaN,0,1",
        "Silhouette InternalEdges=Yes InternalEdges=No",
        "Silhouette Unknown=Yes",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[cfg(not(feature = "native-smlib"))]
#[test]
fn silhouette_reports_the_native_requirement_without_editing_sources() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = source(&mut doc);
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "Silhouette"),
        Err(CommandError::SilhouetteRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn silhouette_box_outputs_spatial_top_curves_and_replays_one_undo_entry() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = source(&mut doc);
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(id).unwrap().clone();
    registry
        .execute(&mut doc, "Silhouette Direction=0,0,10")
        .unwrap();
    assert_eq!(doc.objects().count(), 5);
    assert_eq!(doc.object(id).unwrap(), &before);
    assert_eq!(doc.selected_object_count(), 4);
    assert_eq!(doc.undo_label(), Some("Silhouette"));
    for o in doc.selected_objects() {
        let Geometry::NurbsCurve(c) = o.geometry() else {
            panic!()
        };
        assert!(
            c.control_points()
                .iter()
                .all(|p| (p.point().z() - 4.).abs() < 1e-8)
        );
    }
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().count(), 1);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 5);
    assert_eq!(doc.object(id).unwrap(), &before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn later_unsupported_mesh_rolls_back_earlier_staged_silhouettes_and_preserves_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = source(&mut doc);
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(0., 1., 0.),
            p(0., -1., 0.),
            p(0., 0., 1.),
        ],
        vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
        doc.tolerance(),
    )
    .unwrap();
    let bad = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    registry.execute(&mut doc, "Point 8,8,8").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([id, bad], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(registry.execute(&mut doc, "Silhouette").is_err());
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}

#[cfg(feature = "native-smlib")]
#[test]
fn silhouette_accepts_naked_surfaces_from_both_sides_and_manifold_meshes() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let surface = viboceros_geometry::NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(0., 0., 2.), p(2., 0., 2.), p(0., 3., 2.), p(2., 3., 2.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(0., 0., 1.)],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (geometry, direction, count) in [
        (Geometry::NurbsSurface(surface.clone()), "0,0,1", 4),
        (Geometry::NurbsSurface(surface), "0,0,-1", 4),
        (Geometry::Mesh(mesh), "0,0,1", 3),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut doc, &format!("Silhouette Direction={direction}"))
            .unwrap();
        assert_eq!(doc.selected_object_count(), count);
        assert_eq!(doc.objects().count(), count + 1);
    }
}
