//! Decode raw public SDK mesh snapshots, retaining stored vertex channels.
use serde_json::Value;
use viboceros_geometry::{MeshFace, MeshNgon, Point3, TriangleMesh, Vector3};

pub fn mesh(v: &Value, drop_invalid: bool) -> (TriangleMesh, Vec<usize>) {
    let array = |key: &str| v[key].as_array().filter(|a| !a.is_empty());
    let base = TriangleMesh::try_from_face_records(
        serde_json::from_value::<Vec<[f64; 3]>>(v["vertices"].clone())
            .unwrap()
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        v["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let indices: Vec<u32> = serde_json::from_value(f.clone()).unwrap();
                match *indices.as_slice() {
                    [a, b, c] => MeshFace::Triangle([a, b, c]),
                    [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                    _ => panic!(),
                }
            })
            .collect(),
    )
    .unwrap()
    .try_with_vertex_colors(
        array("colors").map(|a| serde_json::from_value(Value::Array(a.clone())).unwrap()),
    )
    .unwrap()
    .try_with_texture_coordinates(
        array("uv").map(|a| serde_json::from_value(Value::Array(a.clone())).unwrap()),
    )
    .unwrap()
    .try_with_vertex_normals(array("normals").map(|a| {
        a.iter()
            .map(|n| {
                Vector3::try_from(serde_json::from_value::<[f64; 3]>(n.clone()).unwrap()).unwrap()
            })
            .collect()
    }))
    .unwrap();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for (index, g) in v["ngons"].as_array().unwrap().iter().enumerate() {
        if g["vertices"].as_array().unwrap().is_empty() && g["faces"].as_array().unwrap().is_empty()
        {
            continue; // Native n-gon tables can retain empty removed slots.
        }
        let ngon = MeshNgon::from_parts(
            serde_json::from_value(g["vertices"].clone()).unwrap(),
            serde_json::from_value(g["faces"].clone()).unwrap(),
        );
        if base.clone().try_with_ngons(vec![ngon.clone()]).is_err() {
            assert!(drop_invalid);
            invalid.push(index);
        } else {
            valid.push(ngon);
        }
    }
    (base.try_with_ngons(valid).unwrap(), invalid)
}
