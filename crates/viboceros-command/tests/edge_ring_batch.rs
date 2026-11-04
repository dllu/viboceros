use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{MeshFace, Point3, TriangleMesh};
#[test]
fn many_isolated_seed_rings_reuse_source_topology_and_keep_pending_redo() {
    let mut doc = Document::default();
    let (mut points, mut faces) = (Vec::new(), Vec::new());
    for i in 0..2000 {
        let n = points.len() as u32;
        let x = i as f64 * 3.;
        points.extend(
            [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]].map(|p| Point3::try_from(p).unwrap()),
        );
        faces.push(MeshFace::Triangle([n, n + 1, n + 2]));
    }
    let id = doc
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new_faces(points, faces, doc.tolerance()).unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 40,50,60").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let source = doc.object(id).unwrap().clone();
    registry
        .execute(&mut doc, &format!("SelEdgeRing Edges=All Object={id}"))
        .unwrap();
    assert_eq!(registry.component_selection().len(), 6000);
    registry
        .execute(&mut doc, &format!("SelFaceLoop Edges=All Object={id}"))
        .unwrap();
    assert!(registry.component_selection().is_empty());
    assert_eq!(doc.object(id).unwrap(), &source);
    assert!(doc.can_redo());
}
