//! Sampled surface coverage comparison through the shared mesh BVH.
use viboceros_geometry::{MeshProjectionIndex, Point3, TriangleMesh};

pub fn sampled_distance(first: &[TriangleMesh], second: &[TriangleMesh]) -> Option<f64> {
    if first.is_empty() || second.is_empty() {
        return (first.is_empty() == second.is_empty()).then_some(0.);
    }
    fn directed(source: &[TriangleMesh], target: &[TriangleMesh]) -> f64 {
        let indices = target
            .iter()
            .map(|m| MeshProjectionIndex::new(m).unwrap())
            .collect::<Vec<_>>();
        let mut distance = 0_f64;
        for mesh in source {
            for triangle in mesh.triangles() {
                let points = triangle.map(|i| mesh.vertices()[i as usize].to_array());
                for weights in [
                    [1., 0., 0.],
                    [0., 1., 0.],
                    [0., 0., 1.],
                    [0.5, 0.5, 0.],
                    [0., 0.5, 0.5],
                    [0.5, 0., 0.5],
                    [1. / 3.; 3],
                ] {
                    let p = Point3::try_from(std::array::from_fn(|axis| {
                        (0..3).map(|i| weights[i] * points[i][axis]).sum()
                    }))
                    .unwrap();
                    let error = indices
                        .iter()
                        .map(|index| {
                            p.distance_to(index.closest_point(p).unwrap().unwrap())
                                .unwrap()
                        })
                        .fold(f64::INFINITY, f64::min);
                    distance = distance.max(error);
                }
            }
        }
        distance
    }
    Some(directed(first, second).max(directed(second, first)))
}
