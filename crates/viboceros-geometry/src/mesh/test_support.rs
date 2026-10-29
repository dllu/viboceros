//! Shared exact topology lookup for mesh regression tests.
use super::*;

pub(super) fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

pub(super) fn topology_edge_index_between(
    mesh: &TriangleMesh,
    first: Point3,
    second: Point3,
) -> usize {
    let data = mesh.topology_data();
    data.edges
        .keys()
        .position(|&(a, b)| {
            (data.topological_points[a] == first && data.topological_points[b] == second)
                || (data.topological_points[a] == second && data.topological_points[b] == first)
        })
        .expect("test topology edge exists")
}
