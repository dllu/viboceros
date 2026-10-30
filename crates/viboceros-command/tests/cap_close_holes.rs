//! Analytic cap regression for nearby openings; not a Rhino cell-layout claim.
#[path = "../../viboceros-geometry/tests/support/holed_mesh.rs"]
mod source;
use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::Tolerance;

#[test]
fn close_openings_cap_atomically_with_copy_options_metadata_and_history() {
    for delete in [false, true] {
        for crease in [false, true] {
            // Existing boundary positions remain distinct even when generated
            // cap facets are thinner than the document's angular tolerance.
            let mut doc = Document::new(Tolerance::DEFAULT);
            let attrs = ObjectAttributes::on_layer(doc.current_layer_id())
                .with_name("Narrow openings")
                .with_object_color(ColorRgb::new(12, 34, 56));
            let id = doc
                .add_geometry_with_attributes(
                    Geometry::Mesh(source::narrow_openings(1e8)),
                    attrs.clone(),
                )
                .unwrap();
            let group = doc.add_group(Some("Walls".into()), [id]).unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "Cap DeleteInput={} Crease={}",
                        if delete { "Yes" } else { "No" },
                        if crease { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            assert_eq!(doc.objects().len(), if delete { 1 } else { 2 });
            let output = doc.objects().find(|o| delete || o.id() != id).unwrap();
            let Geometry::Mesh(mesh) = output.geometry() else {
                panic!("expected mesh")
            };
            assert!(mesh.topology().is_solid());
            assert_eq!(mesh.face_count(), 40);
            assert_eq!(mesh.vertices().len(), if crease { 48 } else { 24 });
            assert_eq!(output.attributes(), &attrs);
            if delete {
                assert_eq!(output.group_ids(), &[group]);
            } else {
                assert!(output.group_ids().is_empty());
            }
            let volume = source::expected_volume(1e8);
            assert!((mesh.signed_volume().unwrap() - volume).abs() < 1e-12 * volume);
            let after = doc.objects().cloned().collect::<Vec<_>>();
            assert_eq!(doc.undo_label(), Some("Cap"));
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            doc.redo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}
