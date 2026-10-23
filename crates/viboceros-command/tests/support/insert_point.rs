//! Point witnesses, lifecycle evidence and explicit mesh representation comparisons.
use super::{mesh_native, native, subd_native};
pub const CELL_GAPS: [&str; 3] = [
    "insert_point_full_mesh_adjacent",
    "insert_point_full_mesh_same_edge",
    "insert_point_full_mesh_strip_mid",
];
pub const NGON_CELL_GAPS: [&str; 1] = ["insert_point_ngon_two_overlays_shared"];
use serde_json::{Value, json};
use viboceros_command::insert_point::{InsertAction, InsertSession};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    LineSegment, MeshProjectionIndex, Point3, PolygonEdgePoint, Tolerance, TriangleMesh, Vector3,
};
pub fn rows() -> Vec<Value> {
    serde_json::from_str::<Value>(include_str!(
        "../../../../tools/rhino_oracle/observations/insert_point_full.json"
    ))
    .unwrap()["results"]
        .as_array()
        .unwrap()
        .clone()
}
pub fn actions(v: &Value) -> Vec<InsertAction> {
    v["input"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, a)| {
            if a.is_object() {
                let click = v["input_trace"][0]["clicks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["action"] == index)
                    .unwrap();
                InsertAction::Point(native::point(&click["projected_point"]))
            } else {
                match a.as_str().unwrap() {
                    "Undo" => InsertAction::Undo,
                    "Enter" => InsertAction::Finish,
                    "Cancel" => InsertAction::Cancel,
                    _ => panic!(),
                }
            }
        })
        .collect()
}
pub fn script(v: &Value) -> String {
    std::iter::once("InsertPoint".to_owned())
        .chain(actions(v).into_iter().map(|a| match a {
            InsertAction::Point(p) => p.to_array().map(|v| v.to_string()).join(","),
            InsertAction::Undo => "Undo".into(),
            InsertAction::Finish => "Enter".into(),
            InsertAction::Cancel => "Cancel".into(),
            _ => panic!(),
        }))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn stage(session: &mut InsertSession, doc: &Document, v: &Value) -> bool {
    for action in actions(v) {
        match action {
            InsertAction::Point(p) => {
                let pick = pick_in_top_view(session, p);
                session
                    .insert(
                        doc,
                        pick,
                        Some(
                            Vector3::try_new(0., 0., 1.)
                                .unwrap()
                                .normalized_nonzero()
                                .unwrap(),
                        ),
                    )
                    .unwrap();
            }
            InsertAction::Undo => {
                session.undo_point(doc).unwrap();
            }
            InsertAction::Cancel => return false,
            InsertAction::Finish => break,
            _ => panic!(),
        }
    }
    true
}
pub fn pick_in_top_view(session: &InsertSession, point: Point3) -> PolygonEdgePoint {
    let flat = |p: Point3| Point3::try_new(p.x(), p.y(), 0.).unwrap();
    let point = flat(point);
    session
        .source_edges()
        .iter()
        .enumerate()
        .filter_map(|(edge, line)| {
            let projected = LineSegment::try_new(
                flat(line.start()),
                flat(line.end()),
                Tolerance::MESH_VALIDATION,
            )
            .ok()?;
            let parameter = projected
                .closest_parameter(point, Tolerance::MESH_VALIDATION)
                .unwrap();
            Some((
                projected
                    .point_at(parameter)
                    .unwrap()
                    .distance_to(point)
                    .unwrap(),
                PolygonEdgePoint { edge, parameter },
            ))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
        .1
}
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
    let mut sampled = 0f64;
    for (a, b) in [(actual, &expected), (&expected, actual)] {
        let index = MeshProjectionIndex::new(b).unwrap();
        let (triangulated, _) = a.triangulate_quads(Tolerance::MESH_VALIDATION).unwrap();
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
    assert!(sampled <= 1e-6);
    json!({"qualified":logical && ngon_overlays,"logical_regions":logical,"ngon_overlays_correspond":ngon_overlays,"ngons":actual.ngons().len(),"channels":true,"vertices":actual.vertices().len(),"faces":actual.face_count(),"maximum_error":maximum.max(sampled),"stored_pieces_correspond":pieces,"stored_facets_correspond":stored_facets,"actual_cells":cells(actual,&map),"native_cells":cells(&expected,&identity)})
}
pub fn check(doc: &Document, ids: &[ObjectId], rows: &Value, selection: bool) -> Value {
    native::check_metadata(doc, rows, selection);
    let target = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == 0)
        .unwrap();
    match doc.object(ids[0]).unwrap().geometry() {
        Geometry::Mesh(mesh) => compare_mesh(mesh, &target["mesh"]),
        Geometry::SubD(s) => {
            let result = subd_native::compare(s, &target["subd"]);
            json!({"qualified":true,"logical_regions":true,"channels":true,"vertices":s.control_points().len(),"faces":s.faces().len(),"maximum_error":result["maximum_control_or_limit_point_error"].as_f64().unwrap().max(result["maximum_sampled_surface_error"].as_f64().unwrap()),"stored_pieces_correspond":true,"stored_facets_correspond":true})
        }
        _ => panic!(),
    }
}
pub fn assert_known_cell_gap(id: &str, report: &Value) {
    assert_eq!(
        report["stored_pieces_correspond"],
        !CELL_GAPS.contains(&id),
        "native mesh cell gap set changed: {id}"
    );
    assert_eq!(
        report["stored_facets_correspond"],
        !CELL_GAPS.contains(&id),
        "native mesh facet gap set changed: {id}"
    );
}
pub fn assert_ngon_cell_gap(id: &str, report: &Value) {
    for flag in ["stored_pieces_correspond", "stored_facets_correspond"] {
        assert_eq!(
            report[flag],
            !NGON_CELL_GAPS.contains(&id),
            "n-gon cell gap changed: {id}"
        );
    }
}
