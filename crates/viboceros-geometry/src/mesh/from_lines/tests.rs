use super::*;
use serde_json::Value;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn public_line_network_captures_keep_geometry_and_face_counts_visible() {
    for (prefix, source) in [
        (
            "MESH_FROM_LINES_GEOMETRY",
            include_str!("../../../../../tools/rhino_oracle/observations/mesh_from_lines.json"),
        ),
        (
            "MESH_FROM_LINES_VARIANTS_GEOMETRY",
            include_str!(
                "../../../../../tools/rhino_oracle/observations/mesh_from_lines_variants.json"
            ),
        ),
        (
            "MESH_FROM_LINES_CONTROLS_GEOMETRY",
            include_str!(
                "../../../../../tools/rhino_oracle/observations/mesh_from_lines_controls.json"
            ),
        ),
        (
            "MESH_FROM_LINES_EDGES_GEOMETRY",
            include_str!(
                "../../../../../tools/rhino_oracle/observations/mesh_from_lines_edges.json"
            ),
        ),
    ] {
        replay(source, prefix);
    }
}
fn replay(source: &str, prefix: &str) {
    let response: Value = serde_json::from_str(source).unwrap();
    let mut failures = Vec::new();
    for row in response["results"].as_array().unwrap() {
        let v = &row["value"];
        let input = &v["input"];
        let lines = input["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| {
                LineSegment::try_new(
                    point(&line[0]),
                    point(&line[1]),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let actual = TriangleMesh::try_from_lines(
            &lines,
            input["max_sides"].as_u64().unwrap_or(4) as usize,
            Tolerance::try_new(input["sdk_tolerance"].as_f64().unwrap_or(1e-6), 1e-12, 1e-6)
                .unwrap(),
        );
        match actual {
            Err(error) => {
                println!(
                    "{prefix} {}",
                    serde_json::json!({"id":row["id"],"qualified":false,"error":error.to_string()})
                );
                failures.push(row["id"].clone());
            }
            Ok(actual) => {
                let expected = &v["sdk"];
                if expected.is_null() {
                    let qualified = actual.is_none();
                    println!(
                        "{prefix} {}",
                        serde_json::json!({"id":row["id"],"qualified":qualified,"native_empty":true})
                    );
                    if !qualified {
                        failures.push(row["id"].clone());
                    }
                    continue;
                }
                let Some(actual) = actual else {
                    println!(
                        "{prefix} {}",
                        serde_json::json!({"id":row["id"],"qualified":false,"local_empty":true})
                    );
                    failures.push(row["id"].clone());
                    continue;
                };
                let expected = TriangleMesh::try_new_faces(
                    expected["vertices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(point)
                        .collect(),
                    expected["faces"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| {
                            let indices = serde_json::from_value::<Vec<u32>>(f.clone()).unwrap();
                            match indices.len() {
                                3 => MeshFace::Triangle(indices.try_into().unwrap()),
                                _ => MeshFace::Quad(indices.try_into().unwrap()),
                            }
                        })
                        .collect(),
                    Tolerance::MESH_VALIDATION,
                )
                .unwrap();
                let error = distance(&actual, &expected).max(distance(&expected, &actual));
                let qualified = error < 1e-6
                    && actual.faces().len() == expected.faces().len()
                    && actual.vertices().len() == expected.vertices().len();
                println!(
                    "{prefix} {}",
                    serde_json::json!({"id":row["id"],"qualified":qualified,"maximum_bidirectional_sample_error":error,"actual_faces":actual.faces().len(),"native_faces":expected.faces().len(),"actual_vertices":actual.vertices().len(),"native_vertices":expected.vertices().len()})
                );
                if !qualified {
                    failures.push(row["id"].clone());
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "unqualified line networks: {failures:?}"
    );
}
fn distance(a: &TriangleMesh, b: &TriangleMesh) -> Real {
    let index = MeshProjectionIndex::new(b).unwrap();
    let mut maximum: Real = 0.;
    for face in a.triangles() {
        let [a, b, c] = face.map(|i| a.vertices()[i as usize].to_array());
        for i in 0..=8 {
            for j in 0..=8 - i {
                let u = i as Real / 8.;
                let v = j as Real / 8.;
                let p = Point3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                    (b[k] - a[k]).mul_add(u, (c[k] - a[k]).mul_add(v, a[k]))
                }))
                .unwrap();
                maximum = maximum.max(
                    p.distance_to(index.closest_point(p).unwrap().unwrap())
                        .unwrap(),
                );
            }
        }
    }
    maximum
}

#[test]
fn thousands_of_disconnected_regions_do_not_accumulate_quadratic_output_work() {
    let mut lines = Vec::new();
    for i in 0..5000 {
        let x = i as Real * 10.;
        let points =
            [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]].map(|p| Point3::try_from(p).unwrap());
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            lines.push(
                LineSegment::try_new(points[a], points[b], Tolerance::NUMERICAL_VALIDATION)
                    .unwrap(),
            );
        }
    }
    let mesh = TriangleMesh::try_from_lines(&lines, 4, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(mesh.faces().len(), 5000);
    assert_eq!(mesh.vertices().len(), 15000);
    assert!((mesh.area().unwrap() - 2500.).abs() < 1e-6);
}
#[test]
fn dense_networks_fail_with_a_resource_error_before_partial_mesh_admission() {
    let points = (0..100)
        .map(|x| {
            let x = x as Real;
            Point3::try_new(x, x * x, x * x * x).unwrap()
        })
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            lines.push(
                LineSegment::try_new(points[i], points[j], Tolerance::NUMERICAL_VALIDATION)
                    .unwrap(),
            );
        }
    }
    assert!(matches!(
        TriangleMesh::try_from_lines(&lines, 4, Tolerance::DEFAULT),
        Err(GeometryError::MeshLineNetworkLimit)
    ));
}

#[test]
fn a_long_ring_recovers_without_rewalking_every_smaller_root() {
    let count = 5000;
    let points = (0..count)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as Real / count as Real;
            Point3::try_new(100. * angle.cos(), 100. * angle.sin(), 0.).unwrap()
        })
        .collect::<Vec<_>>();
    let lines = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(count)
        .map(|(&a, &b)| LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION).unwrap())
        .collect::<Vec<_>>();
    let mesh = TriangleMesh::try_from_lines(&lines, count, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(mesh.vertices().len(), count);
    assert_eq!(mesh.faces().len(), count - 2);
    let expected = count as Real * 5000. * (std::f64::consts::TAU / count as Real).sin();
    assert!((mesh.area().unwrap() - expected).abs() < 1e-6);
}
