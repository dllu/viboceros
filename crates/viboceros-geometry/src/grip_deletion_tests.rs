//! Complete tensor, knot, weight, facet and n-gon checks against public Delete.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn corpus() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/observations/delete_grips.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}
fn scalar(v: &Value) -> f64 {
    v.as_f64().unwrap()
}
fn usize_value(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn point(v: &Value) -> Point3 {
    Point3::try_new(scalar(&v[0]), scalar(&v[1]), scalar(&v[2])).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| WeightedPoint3::try_new(point(&p["point"]), scalar(&p["weight"])).unwrap())
        .collect()
}
fn knots(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(scalar).collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        usize_value(&v["degree"]),
        controls(&v["control_points"]),
        knots(&v["knots"]),
    )
    .unwrap()
}
fn curve_value(c: &NurbsCurve) -> Value {
    json!({"degree":c.degree(),"control_points":c.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),"knots":c.knots(),"domain":[*c.domain().start(),*c.domain().end()]})
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        usize_value(&v["degree"][0]),
        usize_value(&v["degree"][1]),
        usize_value(&v["control_count"][0]),
        usize_value(&v["control_count"][1]),
        controls(&v["control_points"]),
        knots(&v["knots_u"]),
        knots(&v["knots_v"]),
    )
    .unwrap()
}
fn surface_value(s: &NurbsSurface) -> Value {
    json!({"degree":[s.degree_u(),s.degree_v()],"control_count":[s.control_point_count_u(),s.control_point_count_v()],
        "control_points":(0..s.control_point_count_v()).flat_map(|v|(0..s.control_point_count_u()).map(move|u|s.control_point(u,v).unwrap())).map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),
        "knots_u":s.knots_u(),"knots_v":s.knots_v(),"domain_u":[*s.domain_u().start(),*s.domain_u().end()],"domain_v":[*s.domain_v().start(),*s.domain_v().end()]})
}
fn index_list(v: &Value) -> Vec<u32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|i| usize_value(i) as u32)
        .collect()
}
fn mesh(v: &Value) -> TriangleMesh {
    let vertices = v["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect();
    let faces = v["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let p = index_list(v);
            match p.as_slice() {
                [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                _ => panic!("invalid face"),
            }
        })
        .collect();
    let colors = v["colors"].as_array().map(|a| {
        a.iter()
            .map(|p| {
                [
                    usize_value(&p[0]) as u8,
                    usize_value(&p[1]) as u8,
                    usize_value(&p[2]) as u8,
                    255 - usize_value(&p[3]) as u8,
                ]
            })
            .collect()
    });
    let ngons = v["ngons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| MeshNgon::from_parts(index_list(&g["vertices"]), index_list(&g["faces"])))
        .collect();
    TriangleMesh::try_from_face_records(vertices, faces)
        .unwrap()
        .try_with_vertex_colors(colors)
        .unwrap()
        .try_with_ngons(ngons)
        .unwrap()
}
fn mesh_value(m: &TriangleMesh) -> Value {
    json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices()).collect::<Vec<_>>(),"colors":m.vertex_colors().map(|v|v.iter().map(|c|[c[0],c[1],c[2],255-c[3]]).collect::<Vec<_>>()),"ngons":m.ngons().iter().map(|g|json!({"vertices":g.vertices(),"faces":g.faces()})).collect::<Vec<_>>()})
}
fn difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            ((a - b).abs() > 1e-8_f64.max(1e-12 * a.abs().max(b.abs())))
                .then(|| format!("{path}: {a} != {b}"))
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .find_map(|(i, (a, b))| difference(a, b, &format!("{path}/{i}"))),
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => a
            .iter()
            .find_map(|(k, a)| difference(a, &b[k], &format!("{path}/{k}"))),
        _ => (a != b).then(|| {
            format!(
                "{path}: {} != {}",
                a.to_string().chars().take(180).collect::<String>(),
                b.to_string().chars().take(180).collect::<String>()
            )
        }),
    }
}
fn check(family: &str) {
    let corpus = corpus();
    let mut errors = Vec::new();
    let mut checked = 0;
    let request: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/delete_grips.json"),
        )
        .unwrap(),
    )
    .unwrap();
    for row in corpus["results"].as_array().unwrap() {
        let operation = request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["id"] == row["id"])
            .unwrap();
        let initial = row["value"]["before"]["objects"].as_array().unwrap();
        let after = row["value"]["steps"][0]["after_script"]["objects"]
            .as_array()
            .unwrap();
        for owner in initial {
            if owner.get(family).is_none() {
                continue;
            }
            if family == "curve"
                && matches!(owner["kind"].as_str(), Some("PolyCurve" | "PolylineCurve"))
            {
                continue;
            }
            let selected = owner["grips"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|g| g["selected"] == true && g["hidden"] == false)
                .map(|g| usize_value(&g["index"]))
                .collect::<BTreeSet<_>>();
            if selected.is_empty() {
                continue;
            }
            let expected = after
                .iter()
                .find(|o| o["role"] == owner["role"])
                .map_or(Value::Null, |o| o[family].clone());
            let actual: Result<Value, GeometryError> = match family {
                "curve" => {
                    let source_index = owner["role"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("source")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    let c = curve(&owner[family]);
                    (if operation["sources"][source_index]["mode"] == "edit" {
                        c.try_delete_edit_grips(&selected)
                    } else {
                        c.try_delete_grips(&selected)
                    })
                    .map(|g| g.as_ref().map_or(Value::Null, curve_value))
                }
                "surface" => surface(&owner[family])
                    .try_delete_grips(&selected)
                    .map(|g| g.as_ref().map_or(Value::Null, surface_value)),
                "mesh" => {
                    if selected.len() == owner["grips"].as_array().unwrap().len() {
                        Ok(Value::Null)
                    } else {
                        mesh(&owner[family])
                            .dissolved_vertices(&selected.into_iter().collect::<Vec<_>>())
                            .map(|g| g.as_ref().map_or(Value::Null, mesh_value))
                    }
                }
                _ => unreachable!(),
            };
            let path = format!("{}:{}", row["id"], owner["role"]);
            match actual {
                Ok(actual) => {
                    if let Some(d) = difference(&actual, &expected, &path) {
                        errors.push(d);
                    }
                }
                Err(e) => errors.push(format!("{path}: {e}")),
            }
            checked += 1;
        }
    }
    assert!(checked > 30, "insufficient {family} witnesses");
    assert!(
        errors.is_empty(),
        "{family} differences ({}/{checked}):\n{}",
        errors.len(),
        errors.join("\n")
    );
}
#[test]
fn native_grip_deletion_curve_definitions() {
    check("curve");
}
#[test]
fn native_grip_deletion_surface_tensors() {
    check("surface");
}
#[test]
fn native_grip_deletion_mesh_facets_ngons_and_colors() {
    check("mesh");
}

#[test]
fn native_cyclic_grip_deletion_sequences() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/observations");
    let mut failures = Vec::new();
    let mut count = 0;
    for name in [
        "delete_grips_sequences.json",
        "delete_grips_sequences_extra.json",
    ] {
        let corpus: Value =
            serde_json::from_str(&std::fs::read_to_string(folder.join(name)).unwrap()).unwrap();
        for row in corpus["results"].as_array().unwrap() {
            let mut current = curve(&row["value"]["before"]["curve"]);
            for (i, step) in row["value"]["steps"].as_array().unwrap().iter().enumerate() {
                let picks = step["selected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(usize_value)
                    .collect();
                match current.try_delete_grips(&picks) {
                    Ok(Some(next)) => {
                        if let Some(d) = difference(
                            &curve_value(&next),
                            &step["after_script"]["curve"],
                            &format!("{}/{i}", row["id"]),
                        ) {
                            failures.push(d);
                        }
                        current = next;
                    }
                    other => {
                        failures.push(format!("{}: {other:?}", row["id"]));
                        break;
                    }
                }
            }
        }
        count += corpus["results"].as_array().unwrap().len();
    }
    assert_eq!(count, 115);
    assert!(
        failures.is_empty(),
        "cyclic differences:\n{}",
        failures.join("\n")
    );
}
