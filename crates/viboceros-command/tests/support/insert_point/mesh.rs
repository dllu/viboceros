//! Mesh correspondence and independent surface-policy diagnostics.
use super::*;
fn cycle(v: Vec<u32>) -> Vec<u32> {
    (0..v.len())
        .map(|i| v[i..].iter().chain(&v[..i]).copied().collect())
        .min()
        .unwrap()
}
fn cells(mesh: &TriangleMesh, map: &[u32]) -> Vec<Vec<u32>> {
    let mut out = mesh
        .faces()
        .iter()
        .map(|f| cycle(f.indices().iter().map(|&i| map[i as usize]).collect()))
        .collect::<Vec<_>>();
    out.sort();
    out
}
fn regions(mesh: &TriangleMesh, map: &[u32]) -> Vec<(Vec<u32>, usize)> {
    let mut used = vec![false; mesh.face_count()];
    let mut out = Vec::new();
    for n in mesh.ngons() {
        for &f in n.faces() {
            assert!(!used[f as usize]);
            used[f as usize] = true;
        }
        out.push((
            cycle(n.vertices().iter().map(|&i| map[i as usize]).collect()),
            n.faces().len(),
        ));
    }
    for (i, f) in mesh.faces().iter().enumerate() {
        if !used[i] {
            out.push((
                cycle(f.indices().iter().map(|&i| map[i as usize]).collect()),
                1,
            ));
        }
    }
    out.sort();
    out
}
fn facets(mesh: &TriangleMesh, map: &[u32]) -> Vec<Vec<u32>> {
    let mut out = mesh
        .triangles()
        .iter()
        .map(|t| cycle(t.iter().map(|&i| map[i as usize]).collect()))
        .collect::<Vec<_>>();
    out.sort();
    out
}
fn overlays(mesh: &TriangleMesh, map: &[u32]) -> Vec<(Vec<u32>, usize)> {
    let mut out = mesh
        .ngons()
        .iter()
        .map(|n| {
            (
                cycle(n.vertices().iter().map(|&i| map[i as usize]).collect()),
                n.faces().len(),
            )
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}
pub fn compare_mesh(actual: &TriangleMesh, observed: &Value) -> Value {
    let result = compare_mesh_diagnostic(actual, observed);
    assert!(result["maximum_error"].as_f64().unwrap() <= 1e-6);
    assert_eq!(
        result["logical_regions"], true,
        "oriented logical polygon regions differ"
    );
    assert_eq!(
        result["ngon_overlays_correspond"], true,
        "explicit n-gon overlays differ"
    );
    result
}
pub fn compare_mesh_diagnostic(actual: &TriangleMesh, observed: &Value) -> Value {
    let mut observed = observed.clone();
    if let Some(colors) = observed["colors"].as_array_mut() {
        for c in colors {
            c[3] = json!(255 - c[3].as_u64().unwrap());
        }
    }
    let expected = mesh_native::mesh(&observed);
    assert_eq!(actual.vertices().len(), expected.vertices().len());
    assert_eq!(actual.face_count(), expected.face_count());
    let mut used = vec![false; expected.vertices().len()];
    let mut map = Vec::new();
    let mut maximum = 0f64;
    for (i, &p) in actual.vertices().iter().enumerate() {
        let (j, d) = expected
            .vertices()
            .iter()
            .enumerate()
            .filter(|(j, _)| !used[*j])
            .filter(|(j, _)| {
                actual.vertex_colors().map(|c| c[i]) == expected.vertex_colors().map(|c| c[*j])
                    && observed["uv"].as_array().is_none_or(|uv| {
                        actual.texture_coordinates().is_some_and(|a| {
                            (0..2).all(|k| (a[i][k] - uv[*j][k].as_f64().unwrap()).abs() <= 1e-7)
                        })
                    })
                    && observed["vertex_normals"].as_array().is_none_or(|n| {
                        actual.vertex_normals().is_some_and(|a| {
                            (0..3).all(|k| {
                                (a[i].to_array()[k] - n[*j][k].as_f64().unwrap()).abs() <= 1e-7
                            })
                        })
                    })
            })
            .map(|(j, &q)| (j, p.distance_to(q).unwrap()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(d <= 1e-6);
        used[j] = true;
        map.push(j as u32);
        maximum = maximum.max(d);
        assert_eq!(
            actual.vertex_colors().map(|c| c[i]),
            expected.vertex_colors().map(|c| c[j])
        );
        if let Some(uv) = observed["uv"].as_array() {
            for (actual, expected) in actual.texture_coordinates().unwrap()[i]
                .iter()
                .zip(uv[j].as_array().unwrap())
            {
                assert!((actual - expected.as_f64().unwrap()).abs() <= 1e-7);
            }
        }
        if let Some(normals) = observed["vertex_normals"].as_array() {
            for (actual, expected) in actual.vertex_normals().unwrap()[i]
                .to_array()
                .iter()
                .zip(normals[j].as_array().unwrap())
            {
                assert!((actual - expected.as_f64().unwrap()).abs() <= 1e-7);
            }
        }
    }
    let identity = (0..expected.vertices().len() as u32).collect::<Vec<_>>();
    let logical = regions(actual, &map) == regions(&expected, &identity);
    let ngon_overlays = overlays(actual, &map) == overlays(&expected, &identity);
    let pieces = cells(actual, &map) == cells(&expected, &identity);
    let stored_facets = facets(actual, &map) == facets(&expected, &identity);
    let sampled = sampled_mesh_error(actual, &expected, false);
    let fixed_error = sampled_mesh_error(actual, &expected, true);
    json!({"qualified":logical && ngon_overlays && sampled <= 1e-6 && fixed_error <= 1e-6,"logical_regions":logical,"ngon_overlays_correspond":ngon_overlays,"ngons":actual.ngons().len(),"channels":true,"vertices":actual.vertices().len(),"faces":actual.face_count(),"maximum_error":maximum.max(sampled),"maximum_fixed_facet_sample_error":fixed_error,"fixed_facet_surface_qualified":fixed_error <= 1e-6,"stored_pieces_correspond":pieces,"stored_facets_correspond":stored_facets,"actual_cells":cells(actual,&map),"native_cells":cells(&expected,&identity)})
}
fn sampled_mesh_error(actual: &TriangleMesh, expected: &TriangleMesh, fixed: bool) -> f64 {
    let mut sampled = 0f64;
    for (a, b) in [(actual, expected), (expected, actual)] {
        let triangulated = |mesh: &TriangleMesh| {
            if fixed {
                TriangleMesh::try_from_face_records(
                    mesh.vertices().to_vec(),
                    mesh.triangles()
                        .iter()
                        .copied()
                        .map(viboceros_geometry::MeshFace::Triangle)
                        .collect(),
                )
                .unwrap()
            } else {
                mesh.triangulate_quads(Tolerance::MESH_VALIDATION)
                    .unwrap()
                    .0
            }
        };
        let index = MeshProjectionIndex::new_owned(triangulated(b)).unwrap();
        let triangulated = triangulated(a);
        for t in triangulated.triangles() {
            let p = t.map(|i| triangulated.vertices()[i as usize]);
            for weights in [[1. / 3.; 3], [0.5, 0.25, 0.25], [0.25, 0.5, 0.25]] {
                let point = Point3::try_from(std::array::from_fn(|k| {
                    (0..3).map(|i| p[i].to_array()[k] * weights[i]).sum()
                }))
                .unwrap();
                sampled = sampled.max(
                    point
                        .distance_to(index.closest_point(point).unwrap().unwrap())
                        .unwrap(),
                );
            }
        }
    }
    sampled
}
