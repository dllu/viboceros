//! Stored-channel welding commits one identity-preserving document edit.
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
#[path = "../../viboceros-geometry/tests/support/mesh_channels.rs"]
mod native;

#[test]
fn weld_retains_metadata_groups_and_stored_channels_through_undo_redo() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data: Value = serde_json::from_slice(
        &std::fs::read(root.join("tools/rhino_oracle/observations/mesh_weld_channels.json"))
            .unwrap(),
    )
    .unwrap();
    for name in ["pair_flat", "pair_bent_above", "nonmanifold"] {
        let row = data["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == name)
            .unwrap();
        let v = &row["value"];
        let source = native::mesh(&v["before"]);
        let expected = native::mesh(&v["after"]);
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let attrs = ObjectAttributes::on_layer(doc.current_layer_id())
            .with_name("attributed source")
            .with_object_color(ColorRgb::new(31, 71, 119));
        let id = doc
            .add_geometry_with_attributes(Geometry::Mesh(source.clone()), attrs.clone())
            .unwrap();
        let group = doc.add_group(Some("assembly".into()), [id]).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "Weld Angle={}",
                    v["input"]["angle"].as_f64().unwrap().to_degrees()
                ),
            )
            .unwrap();
        assert_eq!(doc.objects().len(), 1);
        let object = doc.object(id).unwrap();
        assert_eq!(object.attributes(), &attrs);
        assert_eq!(object.group_ids(), [group]);
        assert!(doc.is_selected(id));
        let Geometry::Mesh(mesh) = object.geometry() else {
            panic!()
        };
        assert_eq!(native::face_cycles(mesh), native::face_cycles(&expected));
        assert!(native::near(
            &mesh
                .vertices()
                .iter()
                .map(|p| p.to_array())
                .collect::<Vec<_>>(),
            &native::triples(&v["after"]["vertices"])
        ));
        assert!(native::near(
            mesh.texture_coordinates().unwrap(),
            expected.texture_coordinates().unwrap()
        ));
        assert!(native::near(
            &mesh
                .vertex_normals()
                .unwrap()
                .iter()
                .map(|n| n.to_array())
                .collect::<Vec<_>>(),
            &native::triples(&v["after"]["normals"])
        ));
        assert_eq!(mesh.vertex_colors(), expected.vertex_colors());
        let result = object.geometry_snapshot().clone();
        assert_eq!(doc.undo_label(), Some("Weld"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &Geometry::Mesh(source));
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &result);
        assert_eq!(doc.object(id).unwrap().attributes(), &attrs);
        assert_eq!(doc.object(id).unwrap().group_ids(), [group]);
    }
}
