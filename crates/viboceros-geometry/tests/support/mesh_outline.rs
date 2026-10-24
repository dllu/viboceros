//! Shared frozen mesh inputs and oriented cyclic polyline comparisons.
use serde_json::Value;
use viboceros_geometry::{Frame3, MeshFace, Point3, Polyline3, Tolerance, TriangleMesh, Vector3};
pub fn point(value: &Value) -> Point3 {
    Point3::try_from(std::array::from_fn(|i| value[i].as_f64().unwrap())).unwrap()
}
pub fn vector(value: &Value) -> Vector3 {
    Vector3::try_from(std::array::from_fn(|i| value[i].as_f64().unwrap())).unwrap()
}
pub fn frame(value: &Value) -> Frame3 {
    Frame3::try_from_directions(
        point(&value["origin"]),
        vector(&value["x"]),
        vector(&value["y"]),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
pub fn mesh(value: &Value) -> TriangleMesh {
    let vertices = value["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect();
    let faces = value["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            if f.as_array().unwrap().len() == 3 {
                MeshFace::Triangle(std::array::from_fn(|i| f[i].as_u64().unwrap() as u32))
            } else {
                MeshFace::Quad(std::array::from_fn(|i| f[i].as_u64().unwrap() as u32))
            }
        })
        .collect();
    TriangleMesh::try_from_face_records(vertices, faces).unwrap()
}

fn simplify(mut vertices: Vec<Point3>, epsilon: f64) -> Vec<Point3> {
    assert!(vertices[0].distance_to(*vertices.last().unwrap()).unwrap() <= epsilon);
    vertices.pop();
    loop {
        let n = vertices.len();
        if n <= 3 {
            break;
        }
        let remove = (0..n).find(|&i| {
            let a = vertices[(i + n - 1) % n];
            let b = vertices[i];
            let c = vertices[(i + 1) % n];
            let ac = a.vector_to(c).unwrap();
            let t = a.vector_to(b).unwrap().dot(ac).unwrap() / ac.dot(ac).unwrap();
            (0. ..=1.).contains(&t)
                && a.translated(ac.scaled(t).unwrap())
                    .unwrap()
                    .distance_to(b)
                    .unwrap()
                    <= epsilon
        });
        match remove {
            Some(i) => {
                vertices.remove(i);
            }
            None => break,
        }
    }
    vertices
}

pub fn boundary_error(actual: &[Polyline3], expected: &Value, epsilon: f64) -> Option<f64> {
    let mut expected = expected
        .as_array()
        .unwrap()
        .iter()
        .map(|p| simplify(p.as_array().unwrap().iter().map(point).collect(), epsilon))
        .collect::<Vec<_>>();
    if actual.len() != expected.len() {
        return None;
    }
    let mut max_error = 0_f64;
    for poly in actual {
        assert!(poly.is_closed());
        let actual = simplify(poly.vertices().to_vec(), epsilon);
        let mut best = (f64::INFINITY, 0);
        for (index, native) in expected.iter().enumerate() {
            if actual.len() != native.len() {
                continue;
            }
            for start in 0..native.len() {
                let distance = actual
                    .iter()
                    .enumerate()
                    .map(|(i, p)| p.distance_to(native[(start + i) % native.len()]).unwrap())
                    .fold(0., f64::max);
                if distance < best.0 {
                    best = (distance, index);
                }
            }
        }
        if !best.0.is_finite() {
            return None;
        }
        max_error = max_error.max(best.0);
        expected.remove(best.1);
    }
    Some(max_error)
}

pub fn compare(actual: &[Polyline3], expected: &Value, epsilon: f64) -> f64 {
    let error =
        boundary_error(actual, expected, epsilon).expect("outline contour topology differs");
    assert!(error <= epsilon, "outline boundary differs by {error}");
    error
}
