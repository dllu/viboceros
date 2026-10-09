use super::*;
use viboceros_document::{BlockContent, BlockMember, BlockReference};

fn fixture() -> (Document, ObjectId) {
    let mut document = Document::default();
    let definition = document
        .add_block_definition(
            "part",
            vec![BlockMember::new(
                BlockContent::Geometry(
                    Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()).into(),
                ),
                ObjectAttributes::on_layer(document.current_layer_id()),
            )],
        )
        .unwrap();
    let object = document
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    document
        .select_object(object, SelectionMode::Replace)
        .unwrap();
    (document, object)
}

#[test]
fn move_command_retains_instance_relationship_and_replays_geometry_metadata_and_selection() {
    let (mut document, object) = fixture();
    let registry = CommandRegistry::with_builtins();
    document
        .set_object_geometry_user_text([object], "tag", Some("placed part"))
        .unwrap();
    let before = document.object(object).unwrap().clone();
    registry.execute(&mut document, "Move 0,0,0 3,4,5").unwrap();
    let Geometry::BlockInstance(instance) = document.object(object).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        instance.reference().transform().translation().to_array(),
        [3., 4., 5.]
    );
    let definition = instance.reference().definition();
    let Geometry::Point(point) = &*instance.members()[0].geometry else {
        panic!()
    };
    assert_eq!(point.to_array(), [4., 6., 8.]);
    assert_eq!(
        document.object(object).unwrap().geometry_user_text()["tag"],
        "placed part"
    );
    assert!(document.is_selected(object));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(object).unwrap().geometry(),
        before.geometry()
    );
    registry.execute(&mut document, "Redo").unwrap();
    document
        .replace_block_definition_members(
            definition,
            vec![BlockMember::new(
                BlockContent::Geometry(
                    Geometry::Point(Point3::try_new(10., 20., 30.).unwrap()).into(),
                ),
                ObjectAttributes::on_layer(document.current_layer_id()),
            )],
        )
        .unwrap();
    let Geometry::BlockInstance(instance) = document.object(object).unwrap().geometry() else {
        panic!()
    };
    let Geometry::Point(point) = &*instance.members()[0].geometry else {
        panic!()
    };
    assert_eq!(point.to_array(), [13., 24., 35.]);
}

#[test]
fn structural_export_replaces_the_destination_without_mutating_document() {
    let (mut document, _) = fixture();
    let registry = CommandRegistry::with_builtins();
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("existing.3dm");
    std::fs::write(&file, b"existing destination").unwrap();
    let before = format!("{document:?}");
    registry
        .execute(&mut document, &format!("Export3dm \"{}\"", file.display()))
        .unwrap();
    assert_eq!(format!("{document:?}"), before);
    let model = viboceros_io::read_3dm_file_with_blocks(&file, document.tolerance()).unwrap();
    assert_eq!(model.definitions.len(), 1);
    assert_eq!(model.objects.len(), 1);
    assert!(matches!(
        model.objects[0].geometry,
        viboceros_io::ThreeDmGeometry::InstanceReference { .. }
    ));
}
