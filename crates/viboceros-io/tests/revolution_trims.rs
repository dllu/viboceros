use std::path::Path;
use viboceros_geometry::SurfaceIso;
use viboceros_io::{ThreeDmGeometry, read_3dm_file_for_document};

#[test]
fn opens_rhino_sphere_cut_by_an_oblique_plane_with_saved_render_mesh() {
    // In Rhino 8, trim a radius-3 sphere at the origin with the plane
    // through (0, 0, 0.7) with normal (0.3, 0.1, 1), at tolerance 0.0001.
    // The archive contains the resulting revolution face and its render mesh.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/revolution-trims/oblique-trim-sphere.3dm");
    let mut model = read_3dm_file_for_document(path).unwrap();
    assert_eq!(model.unsupported_object_count(), 0);
    assert_eq!(model.objects.len(), 1);
    let object = &model.objects[0];
    let id = object.id;
    let ThreeDmGeometry::Brep(brep) = &object.geometry else {
        panic!("the archive object must remain editable as a B-rep");
    };
    assert_eq!((brep.vertices().len(), brep.edges().len()), (2, 2));
    let [face] = brep.faces() else {
        panic!("the cut sphere should have one face");
    };
    let [cut] = face.loops()[0]
        .trims()
        .iter()
        .filter(|trim| trim.iso() == SurfaceIso::NotIso)
        .collect::<Vec<_>>()[..]
    else {
        panic!("expected one non-isoparametric trim");
    };
    for (start, end) in cut.curve().spans() {
        for fraction in [0.0, 0.5, 1.0] {
            let uv = cut
                .curve()
                .evaluate(start + (end - start) * fraction)
                .unwrap();
            let point = face.surface().evaluate(uv.x(), uv.y()).unwrap();
            let plane_error = (0.3 * point.x() + 0.1 * point.y() + point.z() - 0.7).abs();
            assert!(plane_error < 0.0002, "{plane_error}");
        }
    }

    let (mesh, sources) = model.take_render_meshes().remove(&id).unwrap();
    assert!(!sources.is_empty());
    assert_eq!(sources.len(), mesh.face_count());
    assert!(sources.iter().all(|&source| source == 0));
    assert!(mesh.vertex_normals().is_some());
}
