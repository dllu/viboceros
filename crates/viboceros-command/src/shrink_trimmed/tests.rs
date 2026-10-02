use super::*;

fn plane() -> NurbsSurface {
    NurbsSurface::try_bilinear(
        [[0., 0., 0.], [10., 0., 0.], [0., 10., 0.], [10., 10., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap()
}

#[test]
fn batch_preserves_identity_attributes_groups_and_one_step_history() {
    for command in ["ShrinkTrimmedSrf", "ShrinkTrimmedSrfToEdge"] {
        for post in [false, true] {
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            let source =
                Brep::try_rectangular_surface_face(plane(), 0.2..=0.8, 0.1..=0.9, doc.tolerance())
                    .unwrap()
                    .reversed();
            let id = doc
                .add_geometry_with_attributes(
                    Geometry::Brep(source.clone()),
                    ObjectAttributes::on_layer(doc.current_layer_id()).with_name("Patch"),
                )
                .unwrap();
            let natural = doc.add_geometry(Geometry::NurbsSurface(plane())).unwrap();
            let group = doc.add_group(Some("Parts".into()), [id, natural]).unwrap();
            doc.select_objects_direct([natural, id], SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let output = if post {
                registry.execute_postselected(&mut doc, command, Default::default())
            } else {
                registry.execute(&mut doc, command)
            };
            assert_eq!(output.unwrap(), "Shrunk 1 surface(s); 1 already shrunk");
            assert_eq!(doc.object(id).unwrap().attributes(), before[0].attributes());
            assert_eq!(doc.object(id).unwrap().group_ids(), &[group]);
            assert_eq!(
                doc.objects().map(|o| o.id()).collect::<Vec<_>>(),
                vec![natural, id]
            );
            assert!(doc.is_selected(natural));
            assert_eq!(doc.is_selected(id), !post);
            let Geometry::Brep(result) = doc.object(id).unwrap().geometry() else {
                panic!()
            };
            assert_eq!(result.edges(), source.edges());
            assert!(result.faces()[0].is_reversed());
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            assert_eq!(doc.object(id).unwrap().group_ids(), &[group]);
        }
    }
}

#[test]
fn already_shrunk_command_preserves_redo_and_selection() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let id = doc.add_geometry(Geometry::NurbsSurface(plane())).unwrap();
    let peer = doc
        .add_geometry(Geometry::Point(Point3::try_new(20., 30., 0.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(id).unwrap().clone();
    registry
        .execute_postselected(&mut doc, "ShrinkTrimmedSrf", Default::default())
        .unwrap();
    assert_eq!(doc.object(id).unwrap(), &before);
    assert!(doc.is_selected(id));
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert!(doc.object(peer).is_some());
}

#[test]
fn face_targets_stage_atomically_preserve_neighbors_and_reject_stale_sources() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let part =
        Brep::try_rectangular_surface_face(plane(), 0.2..=0.8, 0.1..=0.9, doc.tolerance()).unwrap();
    let source = Brep::try_combine(vec![part.clone(), part.reversed()], doc.tolerance()).unwrap();
    let id = doc.add_geometry(Geometry::Brep(source.clone())).unwrap();
    let whole = doc.add_geometry(Geometry::Brep(source.clone())).unwrap();
    let group = doc.add_group(Some("Faces".into()), [id, whole]).unwrap();
    doc.clear_history().unwrap();
    assert!(
        registry
            .execute(&mut doc, &format!("ShrinkTrimmedSrf {id} 0,2"))
            .is_err()
    );
    assert!(!doc.can_undo());
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::Brep(source.clone())
    );
    doc.select_objects_direct([whole], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, &format!("ShrinkTrimmedSrf {id} 0,0"))
        .unwrap();
    let Geometry::Brep(result) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(result.faces()[1], source.faces()[1]);
    assert_eq!(result.edges(), source.edges());
    assert_eq!(doc.object(id).unwrap().group_ids(), &[group]);
    assert!(doc.is_selected(whole));
    assert!(!doc.is_selected(id));
    doc.undo().unwrap();
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::Brep(source.clone())
    );
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    let plan =
        ShrinkTrimmedSelection::prepare(&doc, BrepSurfaceShrinkMode::Standard, [(id, 1)]).unwrap();
    doc.replace_object_geometries([(id, Geometry::Brep(source.reversed()))])
        .unwrap();
    let changed = doc.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        plan.commit(&mut doc, true),
        Err(CommandError::ShrinkTrimmedStale)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), changed);
    assert!(
        registry
            .execute(&mut doc, &format!("ShrinkTrimmedSrfToEdge {id} 0"))
            .is_err()
    );
}

#[test]
fn natural_face_noop_preserves_redo_and_preparation_rejects_changed_tolerance() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsSurface(plane())).unwrap();
    let peer = doc
        .add_geometry(Geometry::Point(Point3::try_new(20., 0., 0.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    let before = doc.object(id).unwrap().clone();
    let plan =
        ShrinkTrimmedSelection::prepare(&doc, BrepSurfaceShrinkMode::Standard, [(id, 0)]).unwrap();
    plan.commit(&mut doc, false).unwrap();
    assert_eq!(doc.object(id).unwrap(), &before);
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert!(doc.object(peer).is_some());
    let plan =
        ShrinkTrimmedSelection::prepare(&doc, BrepSurfaceShrinkMode::Standard, [(id, 0)]).unwrap();
    doc.set_tolerance(Tolerance::try_new(0.01, 1e-6, 0.001).unwrap());
    assert!(matches!(
        plan.commit(&mut doc, false),
        Err(CommandError::ShrinkTrimmedStale)
    ));
    assert_eq!(doc.object(id).unwrap(), &before);
}
