//! Native raw-mesh fixtures and channel comparisons shared by integration tests.
use serde_json::Value;
use viboceros_geometry::{MeshFace, MeshNgon, Point3, TriangleMesh, Vector3};

pub fn triples(value: &Value) -> Vec<[f64; 3]> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| std::array::from_fn(|i| v[i].as_f64().unwrap()))
        .collect()
}

pub fn mesh(v: &Value) -> TriangleMesh {
    let mesh = TriangleMesh::try_from_face_records(
        triples(&v["vertices"])
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        v["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let f = f
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                match f[..] {
                    [a, b, c] => MeshFace::Triangle([a, b, c]),
                    [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                    _ => panic!("invalid native face"),
                }
            })
            .collect(),
    )
    .unwrap()
    .try_with_ngons(
        v["ngons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| {
                let indices = |name: &str| {
                    g[name]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| i.as_u64().unwrap() as u32)
                        .collect()
                };
                MeshNgon::from_parts(indices("vertices"), indices("faces"))
            })
            .collect(),
    )
    .unwrap();
    let mesh = mesh
        .try_with_vertex_colors(
            v.get("colors")
                .and_then(Value::as_array)
                .filter(|c| !c.is_empty())
                .map(|cs| {
                    cs.iter()
                        .map(|c| {
                            [
                                c[0].as_u64().unwrap() as u8,
                                c[1].as_u64().unwrap() as u8,
                                c[2].as_u64().unwrap() as u8,
                                c[3].as_u64().unwrap_or(0) as u8,
                            ]
                        })
                        .collect()
                }),
        )
        .unwrap();
    mesh.try_with_texture_coordinates(v["uv"].as_array().filter(|v| !v.is_empty()).map(|v| {
        v.iter()
            .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
            .collect()
    }))
    .unwrap()
    .try_with_vertex_normals((!v["normals"].as_array().unwrap().is_empty()).then(|| {
        triples(&v["normals"])
            .into_iter()
            .map(|n| Vector3::try_from(n).unwrap())
            .collect()
    }))
    .unwrap()
}

pub fn near<const N: usize>(actual: &[[f64; N]], expected: &[[f64; N]]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(a, b)| a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 1e-6))
}

pub fn face_cycles(mesh: &TriangleMesh) -> Vec<Vec<u32>> {
    mesh.faces()
        .iter()
        .map(|f| {
            let mut cycle = f.indices().to_vec();
            let start = cycle.iter().enumerate().min_by_key(|(_, v)| *v).unwrap().0;
            cycle.rotate_left(start);
            cycle
        })
        .collect()
}

pub fn differences(actual: &TriangleMesh, expected: &Value) -> Vec<&'static str> {
    let mesh = mesh(expected);
    [
        (
            "vertices",
            near(
                &actual
                    .vertices()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>(),
                &triples(&expected["vertices"]),
            ),
        ),
        ("faces", face_cycles(actual) == face_cycles(&mesh)),
        (
            "UV",
            near(
                actual.texture_coordinates().unwrap_or_default(),
                mesh.texture_coordinates().unwrap_or_default(),
            ),
        ),
        (
            "normals",
            near(
                &actual
                    .vertex_normals()
                    .unwrap_or_default()
                    .iter()
                    .map(|n| n.to_array())
                    .collect::<Vec<_>>(),
                &triples(&expected["normals"]),
            ),
        ),
        ("colors", actual.vertex_colors() == mesh.vertex_colors()),
    ]
    .into_iter()
    .filter_map(|(name, passes)| (!passes).then_some(name))
    .collect()
}
