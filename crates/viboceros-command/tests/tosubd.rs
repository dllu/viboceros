use viboceros_command::{
    CommandError, CommandRegistry,
    to_subd::{ToSubDOptions, ToSubDSelection, ToSubDTarget},
};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{Point3, Tolerance, TriangleMesh};
fn scene() -> (Document, viboceros_document::ObjectId) {
    let mut doc = Document::default();
    let points = [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec();
    let id = doc
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(points, vec![[0, 1, 2], [0, 2, 3]], Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
#[test]
fn invalid_options_components_and_stale_candidates_preserve_model_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, id) = scene();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    for args in [
        "UseMesh=Unknown",
        "DeleteInput=Maybe",
        "MeshCreases=Yes MeshCreases=No",
        "UseSurface=Location _UseSurface=ControlPoints",
        "Faces=-1",
        "Faces=100",
        "Faces=0",
        "Object=bad",
        "MeshCorners=Yes extra",
    ] {
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(&mut doc, &format!("ToSubD {args}"))
                .is_err(),
            "{args}"
        );
        assert_eq!(format!("{doc:?}"), before, "{args}");
    }
    assert!(
        ToSubDSelection::prepare(&doc, [ToSubDTarget::Face(id, 0)], ToSubDOptions::default())
            .is_err()
    );
    for change in ["geometry", "locked", "tolerance"] {
        let (mut doc, id) = scene();
        let prepared =
            ToSubDSelection::prepare(&doc, [ToSubDTarget::Whole(id)], ToSubDOptions::default())
                .unwrap();
        match change {
            "geometry" => {
                doc.replace_object_geometries([(
                    id,
                    Geometry::Point(Point3::try_new(1., 1., 1.).unwrap()),
                )])
                .unwrap();
            }
            "locked" => {
                doc.set_objects_locked([id], true).unwrap();
            }
            _ => {
                doc.set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-6).unwrap());
            }
        }
        let before = format!("{doc:?}");
        assert!(matches!(
            prepared.commit(&mut doc),
            Err(CommandError::ToSubDStale)
        ));
        assert_eq!(format!("{doc:?}"), before);
    }
}
#[test]
fn preferences_are_registry_scoped_independent_of_history_and_failed_parsing() {
    let registry = CommandRegistry::with_builtins();
    let mut prompt = registry.object_selection_prompt("ToSubD").unwrap().unwrap();
    assert_eq!(
        ToSubDOptions::from_prompt(&prompt).unwrap(),
        ToSubDOptions::default()
    );
    prompt.update_options("MeshCreases=Yes MeshCorners=Yes DeleteInput=Yes UseSurface=ControlPoints UseMesh=Location").unwrap();
    registry.accept_object_selection_options(&prompt).unwrap();
    let remembered = ToSubDOptions::from_prompt(&prompt).unwrap();
    assert!(
        registry
            .object_selection_prompt("ToSubD MeshCorners=garbage")
            .is_err()
    );
    let (mut doc, _) = scene();
    registry.execute(&mut doc, "ToSubD").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(
        ToSubDOptions::from_prompt(&registry.object_selection_prompt("ToSubD").unwrap().unwrap())
            .unwrap(),
        remembered
    );
    assert_eq!(
        ToSubDOptions::from_prompt(
            &CommandRegistry::with_builtins()
                .object_selection_prompt("ToSubD")
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        ToSubDOptions::default()
    );
}
