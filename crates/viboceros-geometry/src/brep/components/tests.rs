use super::*;
use crate::{Frame3, NurbsSurface, Point3, Tolerance, TriangleMesh, Vector3};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn cube() -> Brep {
    Brep::try_box(frame(), [[-1., 1.]; 3], Tolerance::DEFAULT).unwrap()
}

#[test]
fn connectivity_ignores_sense_and_spatial_coincidence_and_preserves_all_geometry() {
    let b = cube();
    assert_eq!(b.edge_connected_face_components(), [vec![0, 1, 2, 3, 4, 5]]);
    let combined = Brep::try_combine(vec![b.clone(), b.reversed()], Tolerance::DEFAULT).unwrap();
    let before = combined.clone();
    assert_eq!(
        combined.edge_connected_face_components(),
        [vec![0, 1, 2, 3, 4, 5], vec![6, 7, 8, 9, 10, 11]]
    );
    assert_eq!(combined, before);
    // Interleave the face table: neither component ordering nor membership may
    // depend on discovery's union representative or on adjacent table slots.
    let faces = (0..6)
        .flat_map(|i| [combined.faces[i].clone(), combined.faces[i + 6].clone()])
        .collect();
    let interleaved =
        Brep::try_new(combined.vertices, combined.edges, faces, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        interleaved.edge_connected_face_components(),
        [vec![0, 2, 4, 6, 8, 10], vec![1, 3, 5, 7, 9, 11]]
    );
}

#[test]
fn nonmanifold_edges_connect_faces_but_a_shared_vertex_does_not() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(0., 2., 0.),
            point(0., -2., 0.),
            point(0., 0., 2.),
            point(-2., 0., 0.),
            point(0., 0., -2.),
        ],
        vec![[0, 1, 2], [1, 0, 3], [0, 1, 4], [0, 5, 6]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for trimmed in [false, true] {
        let brep = Brep::try_from_mesh(&mesh, trimmed, Tolerance::DEFAULT).unwrap();
        assert!(!brep.is_manifold());
        assert_eq!(
            brep.edge_connected_face_components(),
            [vec![0, 1, 2], vec![3]]
        );
    }
}

#[test]
fn seams_and_poles_do_not_lose_or_duplicate_single_face_components() {
    let mut parts = Vec::new();
    for surface in [
        NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        NurbsSurface::try_cylinder(frame(), 2., 0., 3.).unwrap(),
    ] {
        let b = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(b.edge_connected_face_components(), [vec![0]]);
        parts.push(b);
    }
    let brep = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
    assert_eq!(brep.edge_connected_face_components(), [vec![0], vec![1]]);
}

#[test]
fn every_cube_face_subset_matches_an_independent_shared_edge_graph() {
    let box_brep = cube();
    for mask in 1_u8..64 {
        let faces = (0..6).filter(|&i| mask & (1 << i) != 0).collect::<Vec<_>>();
        let brep = box_brep.sub_brep(&faces, Tolerance::DEFAULT).unwrap();
        let edges = brep
            .faces
            .iter()
            .map(|f| {
                f.loops
                    .iter()
                    .flat_map(|l| &l.trims)
                    .filter_map(|t| t.edge)
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .collect::<Vec<_>>();
        let n = edges.len();
        let mut reachable = vec![vec![false; n]; n];
        for i in 0..n {
            for j in 0..n {
                reachable[i][j] = i == j || !edges[i].is_disjoint(&edges[j]);
            }
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    reachable[i][j] |= reachable[i][k] && reachable[k][j];
                }
            }
        }
        let mut expected = Vec::new();
        for (i, row) in reachable.iter().enumerate() {
            if !row[..i].iter().any(|&connected| connected) {
                expected.push(
                    row.iter()
                        .enumerate()
                        .skip(i)
                        .filter_map(|(j, &connected)| connected.then_some(j))
                        .collect::<Vec<_>>(),
                );
            }
        }
        assert_eq!(
            brep.edge_connected_face_components(),
            expected,
            "mask={mask}"
        );
    }
}

#[test]
fn component_extraction_preserves_geometry_sense_poles_and_shared_vertex_tolerances() {
    let tolerance = Tolerance::DEFAULT;
    let shared_vertex = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(0., 2., 0.),
            point(-2., 0., 0.),
            point(0., 0., -2.),
        ],
        vec![[0, 1, 2], [0, 3, 4]],
        tolerance,
    )
    .unwrap();
    let source = Brep::try_combine(
        vec![
            cube(),
            cube().reversed(),
            Brep::try_surface_face(NurbsSurface::try_sphere(frame(), 2.).unwrap(), tolerance)
                .unwrap(),
            Brep::try_surface_face(
                NurbsSurface::try_cylinder(frame(), 2., 0., 3.).unwrap(),
                tolerance,
            )
            .unwrap(),
            Brep::try_from_mesh(&shared_vertex, true, tolerance).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let before = source.clone();
    let partition = source.edge_connected_face_components();
    assert_eq!(partition.len(), 6);
    let results = source
        .clone()
        .into_edge_connected_components(tolerance)
        .unwrap();
    for (result, faces) in results.iter().zip(&partition) {
        assert_eq!(result, &source.sub_brep(faces, tolerance).unwrap());
    }
    assert_eq!(source, before);
    assert_eq!(
        cube().into_edge_connected_components(tolerance).unwrap(),
        vec![cube()]
    );
}

#[test]
fn component_extraction_compacts_many_independent_sheets_in_source_order() {
    let tolerance = Tolerance::DEFAULT;
    let parts = (0..1024)
        .map(|i| {
            let x = i as f64 * 3.;
            let mesh = TriangleMesh::try_new(
                vec![point(x, 0., 0.), point(x + 1., 0., 0.), point(x, 1., 0.)],
                vec![[0, 1, 2]],
                tolerance,
            )
            .unwrap();
            Brep::try_from_mesh(&mesh, true, tolerance).unwrap()
        })
        .collect::<Vec<_>>();
    let combined = Brep::try_combine(parts.clone(), tolerance).unwrap();
    assert_eq!(
        combined.into_edge_connected_components(tolerance).unwrap(),
        parts
    );
}
