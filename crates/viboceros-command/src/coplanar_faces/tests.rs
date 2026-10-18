use super::*;
use viboceros_geometry::MeshFace;
fn split_rectangle() -> Brep {
    let t = Tolerance::DEFAULT;
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 3., 0.).unwrap(),
            Point3::try_new(0., 3., 0.).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        t,
    )
    .unwrap();
    Brep::try_from_mesh(&mesh, true, t).unwrap()
}
#[test]
fn coplanar_merge_options_and_source_filter_are_strict() {
    let r = CommandRegistry::with_builtins();
    let p = r
        .object_selection_prompt("MergeAllCoplanarFaces")
        .unwrap()
        .unwrap();
    assert_eq!(p.filter, ObjectSelectionFilter::CoplanarFaces);
    assert!(
        r.object_selection_prompt("MergeAllCoplanarFaces Tolerance=1")
            .is_err()
    );
    let mut d = Document::default();
    let id = d
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "MergeAllCoplanarFaces").is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn coplanar_merge_portable_decline_preserves_document() {
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::Brep(split_rectangle())).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut d, "MergeAllCoplanarFaces"),
        Err(CommandError::CoplanarMergeRequiresNativeKernel)
    ));
    assert_eq!(format!("{d:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn coplanar_merge_preserves_attributes_groups_selection_and_one_history_step() {
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::Brep(split_rectangle())).unwrap();
    let second = d
        .add_geometry(Geometry::Brep(
            split_rectangle()
                .transformed(
                    AffineTransform3::from_translation(Vector3::try_new(10., 20., 30.).unwrap()),
                    d.tolerance(),
                )
                .unwrap(),
        ))
        .unwrap();
    d.set_object_names([(id, Some("Panel".into()))]).unwrap();
    let group = d.add_group(None, [id, second]).unwrap();
    let group_members = d.group(group).unwrap().members().collect::<Vec<_>>();
    d.select_objects_direct([id, second], SelectionMode::Replace)
        .unwrap();
    let before = d.object(id).unwrap().clone();
    let second_before = d.object(second).unwrap().clone();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "MergeAllCoplanarFaces").unwrap();
    let after = d.object(id).unwrap().clone();
    let second_after = d.object(second).unwrap().clone();
    assert!(matches!(second_after.geometry(),Geometry::Brep(b) if b.faces().len()==1));
    assert_eq!(after.attributes(), before.attributes());
    assert_eq!(
        d.group(group).unwrap().members().collect::<Vec<_>>(),
        group_members
    );
    assert!(d.is_selected(id));
    assert!(d.is_selected(second));
    let Geometry::Brep(b) = after.geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 1);
    assert!((b.area(d.tolerance()).unwrap() - 12.).abs() < 1e-8);
    assert_eq!(d.undo_label(), Some("MergeAllCoplanarFaces"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.object(id), Some(&before));
    assert_eq!(d.object(second), Some(&second_before));
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.object(id), Some(&after));
    assert_eq!(d.object(second), Some(&second_after));
}
#[cfg(feature = "native-smlib")]
#[test]
fn coplanar_merge_later_failure_and_stale_sources_preserve_history() {
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::Brep(split_rectangle())).unwrap();
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "MergeAllCoplanarFaces").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([id, point], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "MergeAllCoplanarFaces").is_err());
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let staged = CoplanarMergeSelection::prepare(&d).unwrap();
    d.replace_object_geometries([(id, Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))])
        .unwrap();
    let before = format!("{d:?}");
    assert!(staged.commit(&mut d).is_err());
    assert_eq!(format!("{d:?}"), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn coplanar_merge_native_failure_does_not_commit_earlier_sources() {
    let mut d = Document::default();
    let good = d.add_geometry(Geometry::Brep(split_rectangle())).unwrap();
    let t = d.tolerance();
    let points = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 3., 0.],
        [0., 3., 0.],
        [2., -1., -1.],
        [2., 4., -1.],
        [2., 4., 1.],
        [2., -1., 1.],
    ]
    .map(|p| Point3::try_from(p).unwrap())
    .to_vec();
    let mesh =
        TriangleMesh::try_new(points, vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]], t).unwrap();
    let bad = d
        .add_geometry(Geometry::Brep(Brep::try_from_mesh(&mesh, true, t).unwrap()))
        .unwrap();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "Point 8,8,8").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([good, bad], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "MergeAllCoplanarFaces").is_err());
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
}
#[cfg(feature = "native-smlib")]
#[test]
fn coplanar_merge_noop_and_stale_tolerance_preserve_history() {
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::Brep(split_rectangle())).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let r = CommandRegistry::with_builtins();
    let staged = CoplanarMergeSelection::prepare(&d).unwrap();
    d.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let before = format!("{d:?}");
    assert!(staged.commit(&mut d).is_err());
    assert_eq!(format!("{d:?}"), before);
    r.execute(&mut d, "MergeAllCoplanarFaces").unwrap();
    r.execute(&mut d, "Point 8,8,8").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    r.execute(&mut d, "MergeAllCoplanarFaces").unwrap();
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
}

#[cfg(feature = "native-smlib")]
#[test]
fn coplanar_merge_bounds_work_before_native_import() {
    let mut d = Document::default();
    let source = Brep::try_combine(vec![split_rectangle(); 182], d.tolerance()).unwrap();
    let id = d.add_geometry(Geometry::Brep(source)).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    let error = CommandRegistry::with_builtins()
        .execute(&mut d, "MergeAllCoplanarFaces")
        .unwrap_err();
    assert!(error.to_string().contains("362 faces"));
    assert_eq!(format!("{d:?}"), before);
}

#[test]
fn mesh_coplanar_merge_preserves_colors_groups_and_atomic_history() {
    let mut d = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 3., 0.).unwrap(),
            Point3::try_new(0., 3., 0.).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        d.tolerance(),
    )
    .unwrap()
    .try_with_vertex_colors(Some(vec![[1, 2, 3, 4]; 4]))
    .unwrap();
    let id = d.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let group = d.add_group(Some("Panel".into()), [id]).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = d.object(id).unwrap().clone();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "MergeAllCoplanarFaces").unwrap();
    let after = d.object(id).unwrap().clone();
    let Geometry::Mesh(m) = after.geometry() else {
        panic!()
    };
    assert_eq!(m.face_count(), 1);
    assert_eq!(m.vertex_colors().unwrap(), &[[1, 2, 3, 4]; 4]);
    assert_eq!(before.attributes(), after.attributes());
    assert!(d.is_selected(id));
    assert_eq!(
        d.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(d.undo_label(), Some("MergeAllCoplanarFaces"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.object(id), Some(&before));
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.object(id), Some(&after));
    r.execute(&mut d, "Point 9,9,9").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let state = format!("{d:?}");
    r.execute(&mut d, "MergeAllCoplanarFaces").unwrap();
    assert_eq!(format!("{d:?}"), state);
    assert!(d.can_redo());
}

#[test]
fn later_degenerate_mesh_failure_preserves_staged_mesh_and_redo() {
    let mut d = Document::default();
    let t = d.tolerance();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let good = TriangleMesh::try_new(
        vec![p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)],
        vec![[0, 1, 2], [0, 2, 3]],
        t,
    )
    .unwrap();
    let bad = TriangleMesh::try_from_face_records(
        vec![p(0., 0.), p(1., 0.), p(2., 0.), p(0., 1.)],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 3])],
    )
    .unwrap();
    let first = d.add_geometry(Geometry::Mesh(good)).unwrap();
    let second = d.add_geometry(Geometry::Mesh(bad)).unwrap();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "Point 9,9,9").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "MergeAllCoplanarFaces").is_err());
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
}
