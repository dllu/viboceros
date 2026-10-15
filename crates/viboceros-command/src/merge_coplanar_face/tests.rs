use super::*;
fn grid(offset: f64) -> TriangleMesh {
    let t = Tolerance::DEFAULT;
    TriangleMesh::try_plane_grid(
        Frame3::try_from_normal(
            Point3::try_new(offset, 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            t,
        )
        .unwrap(),
        [0., 3.],
        [0., 1.],
        3,
        1,
        t,
    )
    .unwrap()
}
#[test]
fn selected_mesh_pair_region_and_disconnected_regions_preserve_history() {
    let mut doc = Document::default();
    let mesh = TriangleMesh::try_append(&[&grid(0.), &grid(10.)]).unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let group = doc.add_group(None, [id]).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let original = doc.object(id).unwrap().clone();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut doc, &format!("MergeCoplanarFace {id} 0,1"))
        .unwrap();
    let pair = doc.object(id).unwrap().clone();
    let Geometry::Mesh(m) = pair.geometry() else {
        panic!()
    };
    assert_eq!(m.ngons().len(), 1);
    assert_eq!(m.ngons()[0].faces().len(), 2);
    assert_eq!(m.face_count(), 6);
    assert!((m.area().unwrap() - 6.).abs() < 1e-12);
    assert_eq!(pair.attributes(), original.attributes());
    assert!(doc.is_selected(id));
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(doc.undo_label(), Some("MergeCoplanarFace"));
    r.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&original));
    r.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&pair));
    r.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    r.execute(&mut doc, &format!("MergeCoplanarFace Object={id} Face=0,2"))
        .unwrap();
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
    r.execute(
        &mut doc,
        &format!("MergeCoplanarFace MergeAllCoplanar Object={id} Face=4"),
    )
    .unwrap();
    let Geometry::Mesh(m) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(m.ngons().len(), 1);
    assert_eq!(m.ngons()[0].faces(), &[3, 4, 5]);
}
#[test]
fn selected_coplanar_parser_and_frozen_sources_are_strict() {
    let mut d = Document::default();
    let id = d.add_geometry(Geometry::Mesh(grid(0.))).unwrap();
    let r = CommandRegistry::with_builtins();
    for args in ["0,0", "-1,1", "0,9", "0,1,2", "0,", "true,1"] {
        let before = format!("{d:?}");
        assert!(
            r.execute(&mut d, &format!("MergeCoplanarFace {id} {args}"))
                .is_err(),
            "{args}"
        );
        assert_eq!(format!("{d:?}"), before);
    }
    for line in [
        "MergeCoplanarFace Bogus=Yes",
        "MergeCoplanarFace MergeAllCoplanar=Maybe",
        "MergeCoplanarFace MergeAllCoplanar MergeAllCoplanar",
    ] {
        assert!(r.execute(&mut d, line).is_err());
    }
    let staged = CoplanarMergeSelection::prepare_selected(&d, id, &[0, 1], false).unwrap();
    d.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let before = format!("{d:?}");
    assert!(staged.commit(&mut d).is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn selected_brep_indices_and_region_seed_keep_unrelated_components() {
    let mut d = Document::default();
    let t = d.tolerance();
    let mesh = TriangleMesh::try_append(&[&grid(0.), &grid(10.)]).unwrap();
    let brep = Brep::try_from_mesh(&mesh, true, t).unwrap();
    let id = d.add_geometry(Geometry::Brep(brep.clone())).unwrap();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, &format!("MergeCoplanarFace {id} 0,1"))
        .unwrap();
    let Geometry::Brep(b) = d.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 5);
    assert!((b.area(t).unwrap() - 6.).abs() < 1e-10);
    r.execute(&mut d, "Undo").unwrap();
    r.execute(
        &mut d,
        &format!("MergeCoplanarFace MergeAllCoplanar=Yes {id} 4"),
    )
    .unwrap();
    let Geometry::Brep(b) = d.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 4);
    let left = (0..b.faces().len())
        .filter(|&i| {
            b.duplicate_faces(&[i], t)
                .unwrap()
                .tight_bounds(t)
                .unwrap()
                .max()
                .x()
                < 5.
        })
        .count();
    assert_eq!(left, 3);
    assert_eq!(brep.faces().len(), 6);
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn selected_brep_requires_native_and_preserves_redo() {
    let mut d = Document::default();
    let b = Brep::try_from_mesh(&grid(0.), true, d.tolerance()).unwrap();
    let id = d.add_geometry(Geometry::Brep(b)).unwrap();
    let before = format!("{d:?}");
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut d, &format!("MergeCoplanarFace {id} 0,1")),
        Err(CommandError::CoplanarMergeRequiresNativeKernel)
    ));
    assert_eq!(format!("{d:?}"), before);
}
