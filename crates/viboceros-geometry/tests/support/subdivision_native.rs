//! Shared native mesh reconstruction and oriented piece/facet correspondence.
use serde_json::{Value, json};
use viboceros_geometry::{MeshFace, MeshNgon, Point3, TriangleMesh};

pub fn point(p: &Value) -> Point3 {
    Point3::try_new(
        p[0].as_f64().unwrap(),
        p[1].as_f64().unwrap(),
        p[2].as_f64().unwrap(),
    )
    .unwrap()
}
pub fn indices(v: &Value) -> Vec<u32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_u64().unwrap() as u32)
        .collect()
}
pub fn mesh(v: &Value) -> TriangleMesh {
    let points = v["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect();
    let faces = v["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let f = indices(f);
            match f[..] {
                [a, b, c] => MeshFace::Triangle([a, b, c]),
                [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                _ => panic!("invalid polygon"),
            }
        })
        .collect();
    let colors = v["colors"].as_array().map(|a| {
        a.iter()
            .map(|c| std::array::from_fn(|i| c[i].as_u64().unwrap() as u8))
            .collect()
    });
    let ngons = v["ngons"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|n| MeshNgon::from_parts(indices(&n["vertices"]), indices(&n["faces"])))
                .collect()
        })
        .unwrap_or_default();
    TriangleMesh::try_from_face_records(points, faces)
        .unwrap()
        .try_with_vertex_colors(colors)
        .unwrap()
        .try_with_ngons(ngons)
        .unwrap()
}
fn cycle(v: Vec<u32>) -> Vec<u32> {
    (0..v.len())
        .map(|i| v[i..].iter().chain(&v[..i]).copied().collect::<Vec<_>>())
        .min()
        .unwrap()
}
fn regions(mesh: &TriangleMesh) -> Vec<(Vec<u32>, Vec<usize>)> {
    let mut used = vec![false; mesh.face_count()];
    let mut out = Vec::new();
    for n in mesh.ngons() {
        for &f in n.faces() {
            used[f as usize] = true;
        }
        out.push((
            cycle(n.vertices().to_vec()),
            n.faces().iter().map(|&i| i as usize).collect(),
        ));
    }
    for (i, f) in mesh.faces().iter().enumerate() {
        if !used[i] {
            out.push((cycle(f.indices().to_vec()), vec![i]));
        }
    }
    out
}
pub fn compare_mesh(actual: &TriangleMesh, native: &Value, original: usize) -> Value {
    let expected = mesh(native);
    assert_eq!(actual.vertices().len(), expected.vertices().len());
    assert_eq!(actual.face_count(), expected.face_count());
    assert_eq!(actual.ngons().len(), expected.ngons().len());
    assert_eq!(
        actual.vertex_colors().is_some(),
        expected.vertex_colors().is_some()
    );
    let a = regions(actual);
    let b = regions(&expected);
    let mut used = vec![false; expected.vertices().len()];
    let mut permutation = Vec::new();
    let mut error = 0_f64;
    // Refine common coordinate/color classes through raw-edge adjacency. Fixed
    // original vertex IDs disambiguate seams even after repeated subdivision.
    fn adjacency(mesh: &TriangleMesh) -> Vec<std::collections::BTreeSet<usize>> {
        let mut graph = vec![std::collections::BTreeSet::new(); mesh.vertices().len()];
        for f in mesh.faces() {
            let v = f.indices();
            for i in 0..v.len() {
                let a = v[i] as usize;
                let b = v[(i + 1) % v.len()] as usize;
                graph[a].insert(b);
                graph[b].insert(a);
            }
        }
        graph
    }
    let mut initial = std::collections::BTreeMap::new();
    let mut labels = Vec::new();
    for mesh in [actual, &expected] {
        labels.push(
            mesh.vertices()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let key = (
                        (i < original).then_some(i),
                        p.to_array().map(|c| (c / 1e-6).round() as i64),
                        mesh.vertex_colors().map(|c| c[i]),
                    );
                    let n = initial.len();
                    *initial.entry(key).or_insert(n)
                })
                .collect::<Vec<_>>(),
        );
    }
    let graphs = [adjacency(actual), adjacency(&expected)];
    for _ in 0..actual.vertices().len() {
        let mut classes = std::collections::BTreeMap::new();
        let mut next = Vec::new();
        for side in 0..2 {
            next.push(
                (0..labels[side].len())
                    .map(|i| {
                        let mut neighbors = graphs[side][i]
                            .iter()
                            .map(|&j| labels[side][j])
                            .collect::<Vec<_>>();
                        neighbors.sort();
                        let key = (labels[side][i], neighbors);
                        let n = classes.len();
                        *classes.entry(key).or_insert(n)
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if next == labels {
            break;
        }
        labels = next;
    }
    for (i, &p) in actual.vertices().iter().enumerate() {
        let color = actual.vertex_colors().map(|c| c[i]);
        let (j, d) = if i < original {
            (i, p.distance_to(expected.vertices()[i]).unwrap())
        } else {
            expected
                .vertices()
                .iter()
                .enumerate()
                .filter(|(j, _)| {
                    !used[*j]
                        && color == expected.vertex_colors().map(|c| c[*j])
                        && labels[0][i] == labels[1][*j]
                })
                .map(|(j, q)| (j, p.distance_to(*q).unwrap()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
        };
        assert_eq!(color, expected.vertex_colors().map(|c| c[j]));
        used[j] = true;
        error = error.max(d);
        permutation.push(j as u32);
    }
    assert!(error <= 1e-6, "mesh vertex error {error}");
    let mut ar = a
        .iter()
        .map(|(v, faces)| {
            let mut members = faces
                .iter()
                .map(|&face| {
                    cycle(
                        actual.faces()[face]
                            .indices()
                            .iter()
                            .map(|&i| permutation[i as usize])
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            members.sort();
            (
                cycle(v.iter().map(|&i| permutation[i as usize]).collect()),
                members,
            )
        })
        .collect::<Vec<_>>();
    let mut br = b
        .iter()
        .map(|(v, faces)| {
            let mut members = faces
                .iter()
                .map(|&face| cycle(expected.faces()[face].indices().to_vec()))
                .collect::<Vec<_>>();
            members.sort();
            (v.clone(), members)
        })
        .collect::<Vec<_>>();
    ar.sort();
    br.sort();
    assert_eq!(ar, br, "logical polygon regions differ");
    let mut actual_pieces = actual
        .faces()
        .iter()
        .map(|f| {
            cycle(
                f.indices()
                    .iter()
                    .map(|&i| permutation[i as usize])
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let mut native_pieces = expected
        .faces()
        .iter()
        .map(|f| cycle(f.indices().to_vec()))
        .collect::<Vec<_>>();
    actual_pieces.sort();
    native_pieces.sort();
    assert_eq!(
        actual_pieces, native_pieces,
        "oriented stored pieces differ"
    );
    let mut actual_triangles = actual
        .triangles()
        .iter()
        .map(|t| cycle(t.iter().map(|&i| permutation[i as usize]).collect()))
        .collect::<Vec<_>>();
    let mut native_triangles = expected
        .triangles()
        .iter()
        .map(|t| cycle(t.to_vec()))
        .collect::<Vec<_>>();
    actual_triangles.sort();
    native_triangles.sort();
    assert_eq!(
        actual_triangles, native_triangles,
        "stored quad diagonals differ"
    );
    json!({"qualified":true,"vertices":actual.vertices().len(),"faces":actual.face_count(),"ngons":actual.ngons().len(),"maximum_vertex_error":error,"stored_pieces_correspond":true,"triangle_facets_correspond":true,"raw_seams_colors_and_regions":true})
}
