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
