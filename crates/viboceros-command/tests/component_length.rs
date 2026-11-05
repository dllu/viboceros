use viboceros_command::{CommandRegistry, ComponentPick, ComponentSelectionKind as Kind};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{MeshFace, Point3, TriangleMesh};
#[test]
fn large_mesh_edge_sets_measure_once_per_owner_and_preserve_model_and_redo() {
    let n = 5000u32;
    let points = (0..=n)
        .flat_map(|i| {
            [
                Point3::try_new(i as f64, 0., 0.).unwrap(),
                Point3::try_new(i as f64, 1., 0.).unwrap(),
            ]
        })
        .collect();
    let faces = (0..n)
        .map(|i| MeshFace::Quad([2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1]))
        .collect();
    let mesh = TriangleMesh::try_from_face_records(points, faces).unwrap();
    let mut doc = Document::default();
    let owner = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 40,50,60").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry
        .component_selection()
        .replace(
            &doc,
            (0..3 * n + 1)
                .map(|i| ComponentPick {
                    object: owner,
                    index: i as usize,
                    kind: Kind::MeshEdge,
                })
                .collect(),
        )
        .unwrap();
    let source = doc.object(owner).unwrap().clone();
    let report = registry.execute(&mut doc, "Length").unwrap();
    assert_eq!(
        report,
        format!(
            "Measured {} curve(s): total length {}",
            3 * n + 1,
            3 * n + 1
        )
    );
    assert_eq!(doc.object(owner), Some(&source));
    assert!(registry.component_selection().is_empty());
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
}
