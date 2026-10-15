use super::*;
use viboceros_geometry::MeshFace;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn source(all_bad: bool) -> Geometry {
    let vertices = vec![
        p(0., 0., 0.),
        p(2., 0., 0.),
        p(0., 2., 0.),
        p(0., 0., 0.),
        p(3., 0., 0.),
        p(4., 0., 0.),
        p(5., 0., 0.),
    ];
    let faces = if all_bad {
        vec![MeshFace::Triangle([4, 5, 6])]
    } else {
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 5, 6])]
    };
    Geometry::Mesh(TriangleMesh::try_from_face_records(vertices, faces).unwrap())
}
#[test]
fn native_cleanup_stages_replacements_and_empty_deletion_in_one_history_entry() {
    let mut d = Document::default();
    let r = CommandRegistry::with_builtins();
    let repaired = d
        .add_geometry_with_attributes(
            source(false),
            ObjectAttributes::on_layer(d.current_layer_id()).with_name("repair"),
        )
        .unwrap();
    let deleted = d.add_geometry(source(true)).unwrap();
    let group = d.add_group(None, [repaired, deleted]).unwrap();
    d.select_objects_direct([repaired, deleted], SelectionMode::Replace)
        .unwrap();
    let originals = d.objects().cloned().collect::<Vec<_>>();
    let text = r.execute(&mut d, "CullDegenerateMeshFaces").unwrap();
    assert!(
        text.starts_with("Removed 2 degenerate face(s), repaired 1 quad(s)"),
        "{text}"
    );
    assert!(d.object(deleted).is_none());
    assert_eq!(
        d.object(repaired).unwrap().attributes(),
        originals[0].attributes()
    );
    assert!(
        matches!(d.object(repaired).unwrap().geometry(),Geometry::Mesh(m) if m.face_count()==1 && (m.area().unwrap()-2.).abs()<1e-12)
    );
    assert!(d.is_selected(repaired));
    assert_eq!(
        d.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![repaired]
    );
    assert_eq!(d.undo_label(), Some("CullDegenerateMeshFaces"));
    let final_object = d.object(repaired).unwrap().clone();
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), originals);
    assert!(d.is_selected(repaired) && d.is_selected(deleted));
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.object(repaired), Some(&final_object));
    assert!(d.object(deleted).is_none());
}
#[test]
fn noops_and_later_staging_failure_preserve_redo_and_every_source() {
    let mut d = Document::default();
    let r = CommandRegistry::with_builtins();
    let good = d
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)],
                vec![[0, 1, 2]],
                d.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let bad = d.add_geometry(source(false)).unwrap();
    let point = d.add_geometry(Geometry::Point(p(4., 5., 6.))).unwrap();
    d.add_geometry(Geometry::Point(p(9., 9., 9.))).unwrap();
    d.undo().unwrap();
    d.select_objects_direct([good], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "CullDegenerateMeshFaces")
            .unwrap()
            .contains("0 mesh(es) changed")
    );
    assert_eq!(format!("{d:?}"), before);
    d.select_objects_direct([bad, point], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "CullDegenerateMeshFaces").is_err());
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
    assert!(r.execute(&mut d, "CullDegenerateMeshFaces Extra").is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[test]
fn prepared_cleanup_checks_source_identity_but_is_independent_of_document_tolerance() {
    let mut d = Document::default();
    let id = d.add_geometry(source(false)).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let selection = DegenerateFaceSelection::prepare(&d).unwrap();
    d.set_tolerance(Tolerance::try_new(1e-3, 1e-8, 1e-5).unwrap());
    selection.commit(&mut d).unwrap();
    d.undo().unwrap();
    let selection = DegenerateFaceSelection::prepare(&d).unwrap();
    d.replace_object_geometries([(id, source(true))]).unwrap();
    let before = format!("{d:?}");
    assert!(selection.commit(&mut d).is_err());
    assert_eq!(format!("{d:?}"), before);
}
#[test]
fn check_to_repair_workflow_and_mesh_only_getter_use_shared_commands() {
    let mut d = Document::default();
    let r = CommandRegistry::with_builtins();
    let id = d.add_geometry(source(false)).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert!(r.execute(&mut d, "Check").unwrap().contains("1 invalid"));
    r.execute(&mut d, "CullDegenerateMeshFaces").unwrap();
    assert!(
        r.execute(&mut d, "Check")
            .unwrap()
            .contains("1 valid, 0 invalid")
    );
    let prompt = r
        .object_selection_prompt("CullDegenerateMeshFaces")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Mesh);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::ConfirmAfterSelection
    );
}
