use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::Tolerance;

#[test]
fn tonurbs_converts_native_extrusions_with_attributes_identity_and_undo_redo() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/extrusions/tosubd_extrusion_rectangle_00.3dm");
    let model = viboceros_io::read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    let viboceros_io::ThreeDmGeometry::Brep(source) = &model.objects[0].geometry else {
        panic!("native extrusion missing")
    };
    assert!(source.is_extrusion());
    for delete in [false, true] {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Brep(source.clone()))
            .unwrap();
        document.add_group(Some("Original".into()), [id]).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        let prompt = registry
            .object_selection_prompt("ToNURBS")
            .unwrap()
            .unwrap();
        assert!(
            registry
                .object_selection_confirmation(&document, &prompt)
                .unwrap()
                .is_some()
        );
        registry
            .execute(
                &mut document,
                &format!(
                    "ToNURBS DeleteInputObjects={}",
                    if delete { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        assert_eq!(document.objects().len(), if delete { 1 } else { 2 });
        let output = if delete {
            document.object(id).unwrap()
        } else {
            document.objects().find(|o| o.id() != id).unwrap()
        };
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("explicit B-rep missing")
        };
        assert!(!brep.is_extrusion());
        assert_eq!(brep, &source.explicit_brep());
        assert_eq!(output.attributes(), before[0].attributes());
        assert_eq!(output.group_ids(), before[0].group_ids());
        if !delete {
            assert_eq!(document.object(id).unwrap(), &before[0]);
        }
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}
