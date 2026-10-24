use viboceros_document::{
    BlockContent, BlockMember, BlockReference, Document, Geometry, ObjectAttributes, SelectionMode,
};
use viboceros_geometry::{AffineTransform3, Point3, Tolerance, Vector3};

fn point(x: f64) -> Point3 {
    Point3::try_new(x, x + 1., x + 2.).unwrap()
}

#[test]
fn replacing_geometry_with_an_empty_mesh_retains_identity_attributes_groups_and_history() {
    let mut document = Document::default();
    let id = document
        .add_geometry_with_attributes(
            Geometry::Point(point(1000.)),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("outside"),
        )
        .unwrap();
    let group = document.add_group(Some("results".into()), [id]).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let source = document.object(id).unwrap().geometry_snapshot().clone();
    document.begin_transaction("Empty outside").unwrap();
    document
        .replace_object_geometries([(id, Geometry::EmptyMesh)])
        .unwrap();
    document.commit_transaction().unwrap();
    let object = document.object(id).unwrap();
    assert_eq!(object.attributes().name(), Some("outside"));
    assert_eq!(object.group_ids(), [group]);
    assert!(document.is_selected(id));
    assert_eq!(document.bounds(), None);
    assert_eq!(object.geometry().bounds(), None);
    assert!(object.geometry().tight_bounds(Tolerance::DEFAULT).is_err());
    assert!(
        object
            .geometry()
            .extract_point_locations()
            .unwrap()
            .is_empty()
    );
    document.undo().unwrap();
    assert!(
        document
            .object(id)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&source)
    );
    document.redo().unwrap();
    assert_eq!(document.bounds(), None);
    document
        .add_geometry(Geometry::Point(point(8000.)))
        .unwrap();
    assert_eq!(document.bounds().unwrap().min(), point(8000.));
    assert_eq!(document.bounds().unwrap().max(), point(8000.));
}

#[test]
fn empty_block_members_do_not_add_an_origin_to_nested_instance_bounds() {
    let mut document = Document::default();
    let member = |geometry: Geometry| {
        BlockMember::new(
            BlockContent::Geometry(geometry.into()),
            ObjectAttributes::on_layer(document.current_layer_id()),
        )
    };
    let empty = document
        .add_block_definition("empty", vec![member(Geometry::EmptyMesh)])
        .unwrap();
    let placement =
        AffineTransform3::from_translation(Vector3::try_new(1000., 2000., 3000.).unwrap());
    let id = document
        .add_block_instance(BlockReference::try_new(empty, placement).unwrap())
        .unwrap();
    assert_eq!(document.object(id).unwrap().geometry().bounds(), None);
    assert_eq!(document.bounds(), None);
    let mixed = document
        .add_block_definition(
            "mixed",
            vec![
                BlockMember::new(
                    BlockContent::Reference(BlockReference::try_new(empty, placement).unwrap()),
                    ObjectAttributes::on_layer(document.current_layer_id()),
                ),
                BlockMember::new(
                    BlockContent::Geometry(Geometry::Point(point(8000.)).into()),
                    ObjectAttributes::on_layer(document.current_layer_id()),
                ),
            ],
        )
        .unwrap();
    let id = document
        .add_block_instance(BlockReference::try_new(mixed, placement).unwrap())
        .unwrap();
    let expected = placement.transform_point(point(8000.)).unwrap();
    let geometry = document.object(id).unwrap().geometry();
    assert_eq!(geometry.bounds().unwrap().min(), expected);
    assert_eq!(
        geometry.tight_bounds(Tolerance::DEFAULT).unwrap().min(),
        expected
    );
    assert_eq!(document.bounds().unwrap().min(), expected);
}
