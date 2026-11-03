use super::*;

#[test]
fn inset_output_modes_preserve_or_recreate_identity_and_remember_completed_options() {
    for nurbs in [false, true] {
        for curves in [false, true] {
            let mut doc = Document::default();
            let surface = NurbsSurface::try_bilinear(
                [[0., 0., 0.], [8., 0., 0.], [8., 6., 0.], [0., 6., 0.]]
                    .map(|p| Point3::try_from(p).unwrap()),
            )
            .unwrap();
            let source = if nurbs {
                Geometry::NurbsSurface(surface)
            } else {
                Geometry::Brep(Brep::try_surface_face(surface, doc.tolerance()).unwrap())
            };
            let layer = doc
                .add_layer("Source", viboceros_document::ColorRgb::new(0, 0, 255))
                .unwrap();
            let id = doc
                .add_geometry_with_attributes(
                    source,
                    ObjectAttributes::on_layer(layer).with_name("Input"),
                )
                .unwrap();
            doc.add_group(Some("Source group".into()), [id]).unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let before = doc.object(id).unwrap().geometry_snapshot().clone();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(
                    &mut doc,
                    if curves {
                        "Inset .3 Split=No CreaseCorners=Yes"
                    } else {
                        "Inset .3 JoinOutputs=No CreaseCorners=Yes"
                    },
                )
                .unwrap();
            assert_eq!(doc.objects().len(), if curves { 6 } else { 5 });
            assert_eq!(doc.object(id).is_some(), curves);
            if curves {
                assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
            }
            for obj in doc.objects().filter(|o| o.id() != id) {
                assert_eq!(
                    obj.attributes(),
                    &ObjectAttributes::on_layer(doc.current_layer_id())
                );
                assert_eq!(obj.attributes().name(), None);
                assert_eq!(obj.attributes().layer_id(), doc.current_layer_id());
                assert!(obj.group_ids().is_empty());
                assert!(!doc.is_selected(obj.id()));
            }
            assert_eq!(registry.inset_surface_options().split, !curves);
            assert_eq!(registry.inset_surface_options().join_outputs, curves);
            let after = doc
                .objects()
                .map(|o| (o.id(), o.geometry_snapshot().clone()))
                .collect::<Vec<_>>();
            doc.undo().unwrap();
            assert_eq!(doc.objects().len(), 1);
            assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
            assert_eq!(registry.inset_surface_options().split, !curves);
            doc.redo().unwrap();
            assert_eq!(
                doc.objects()
                    .map(|o| (o.id(), o.geometry_snapshot().clone()))
                    .collect::<Vec<_>>(),
                after
            );
        }
    }
}
