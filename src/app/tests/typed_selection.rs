use super::*;
use viboceros_document::{BlockContent, BlockMember, BlockReference, ObjectAttributes};
use viboceros_geometry::AffineTransform3;

#[test]
fn typed_selection_commands_execute_without_a_prompt_and_preserve_model_and_redo() {
    let mut app = test_app();
    app.commands
        .execute(&mut app.document, "Box 0,0,0 2,3,0 4")
        .unwrap();
    let extrusion = app.document.objects().next().unwrap().id();
    let layer = app.document.current_layer_id();
    let definition = app
        .document
        .add_block_definition(
            "Selection block",
            vec![BlockMember::new(
                BlockContent::Geometry(Geometry::Point(point(1., 2., 3.)).into()),
                ObjectAttributes::on_layer(layer),
            )],
        )
        .unwrap();
    let identity = AffineTransform3::identity();
    let mirror = AffineTransform3::try_nonuniform_scale(point(0., 0., 0.), [-1., 1., 1.]).unwrap();
    let normal = app
        .document
        .add_block_instance(BlockReference::try_new(definition, identity).unwrap())
        .unwrap();
    let mirrored = app
        .document
        .add_block_instance(BlockReference::try_new(definition, mirror).unwrap())
        .unwrap();
    app.document
        .add_geometry(Geometry::Point(point(99., 99., 99.)))
        .unwrap();
    app.document.undo().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    for (command, expected) in [
        ("SelExtrusion", vec![extrusion]),
        ("SelBlockInstance", vec![normal, mirrored]),
        ("SelMirroredBlocks", vec![mirrored]),
    ] {
        app.document
            .select_objects_direct([], SelectionMode::Replace)
            .unwrap();
        app.command_input = command.into();
        app.run_command();
        assert!(app.command_line_idle());
        assert!(app.object_prompt.is_none());
        let actual = app
            .document
            .selected_object_ids()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual, expected.into_iter().collect());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
        assert!(app.document.can_redo());
    }
}
