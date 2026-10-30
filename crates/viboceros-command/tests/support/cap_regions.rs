#![allow(dead_code)]
//! Independent source archives and full Cap scene/history comparisons.
#[path = "cap_regions_surface.rs"]
mod planar;
use super::{mesh_native, scene, surface};
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::Tolerance;

pub fn compare(doc: &Document, ids: &[ObjectId], rows: &Value, selection: bool) -> Value {
    let mut maximum = 0f64;
    let mut counts = true;
    let mut planar_match = true;
    let mut topology = true;
    let mut metadata = true;
    let mut ngons = true;
    let mut normals = Some(true);
    assert_eq!(doc.objects().len(), rows.as_array().unwrap().len());
    for row in rows.as_array().unwrap() {
        let object = if let Some(i) = row["source"].as_u64() {
            doc.object(ids[i as usize]).unwrap()
        } else if row.get("point").is_some() {
            doc.objects()
                .find(|o| {
                    o.attributes().name() == row["name"].as_str()
                        && matches!(o.geometry(), Geometry::Point(_))
                })
                .unwrap()
        } else {
            let outputs = doc
                .objects()
                .filter(|o| {
                    !ids.contains(&o.id())
                        && matches!(o.geometry(), Geometry::Mesh(_))
                        && o.attributes().name() == row["name"].as_str()
                })
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), 1);
            outputs[0]
        };
        let attrs = object.attributes();
        let color = attrs.object_color();
        metadata &= json!([color.red, color.green, color.blue]) == row["color"]
            && attrs.color_source() as u8 == row["color_source"].as_u64().unwrap() as u8
            && attrs.name() == row["name"].as_str()
            && doc.layer(attrs.layer_id()).unwrap().name() == row["layer"].as_str().unwrap()
            && json!(
                object
                    .group_ids()
                    .iter()
                    .map(|&g| doc.group(g).unwrap().name().unwrap())
                    .collect::<Vec<_>>()
            ) == row["groups"]
            && (!selection || doc.is_selected(object.id()) == row["selected"].as_bool().unwrap());
        if let Geometry::Mesh(actual) = object.geometry() {
            let expected = mesh_native::mesh(&row["mesh"]);
            let boundaries = |mesh: &viboceros_geometry::TriangleMesh| {
                let mut out = mesh
                    .ngons()
                    .iter()
                    .map(|n| {
                        let v = n
                            .vertices()
                            .iter()
                            .map(|&i| {
                                mesh.vertices()[i as usize]
                                    .to_array()
                                    .map(|x| if x == 0. { 0 } else { x.to_bits() })
                            })
                            .collect::<Vec<_>>();
                        (0..v.len())
                            .map(|i| v[i..].iter().chain(&v[..i]).copied().collect::<Vec<_>>())
                            .min()
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                out.sort();
                out
            };
            ngons &= boundaries(actual) == boundaries(&expected);
            let a = planar::signature(actual);
            let b = planar::signature(&expected);
            assert!(
                a.is_some() && b.is_some(),
                "axis-aligned certificate unavailable"
            );
            planar_match &= a == b;
            maximum = maximum
                .max(surface::directed(actual, &expected))
                .max(surface::directed(&expected, actual));
            let same_counts = actual.vertices().len() == expected.vertices().len()
                && actual.face_count() == expected.face_count();
            counts &= same_counts;
            if !same_counts {
                normals = None;
            } else if let Some(matches) = &mut normals {
                let native_normals = row["mesh"]["vertex_normals"].as_array().unwrap();
                if let Some(actual_normals) = actual.vertex_normals() {
                    if native_normals.len() != expected.vertices().len() {
                        *matches = false;
                    } else {
                        let graph = actual
                            .vertices()
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                expected
                                    .vertices()
                                    .iter()
                                    .enumerate()
                                    .filter(|(j, q)| {
                                        p.distance_to(**q).unwrap() <= 1e-6
                                            && (0..3).all(|k| {
                                                (actual_normals[i].to_array()[k]
                                                    - native_normals[*j][k].as_f64().unwrap())
                                                .abs()
                                                    <= 1e-6
                                            })
                                    })
                                    .map(|(j, _)| j)
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();
                        fn augment(
                            a: usize,
                            graph: &[Vec<usize>],
                            matched: &mut [Option<usize>],
                            seen: &mut [bool],
                        ) -> bool {
                            for &b in &graph[a] {
                                if seen[b] {
                                    continue;
                                }
                                seen[b] = true;
                                if matched[b].is_none()
                                    || augment(matched[b].unwrap(), graph, matched, seen)
                                {
                                    matched[b] = Some(a);
                                    return true;
                                }
                            }
                            false
                        }
                        let count = expected.vertices().len();
                        let mut matched = vec![None; count];
                        *matches &= (0..graph.len())
                            .all(|i| augment(i, &graph, &mut matched, &mut vec![false; count]));
                    }
                } else {
                    *matches &= native_normals.is_empty();
                }
            }
            topology &= actual.topology().is_solid() == row["mesh"]["solid"].as_bool().unwrap()
                && actual.topology().boundary_edge_count()
                    == row["mesh"]["boundary_edges"].as_u64().unwrap() as usize;
        } else if let Geometry::Point(point) = object.geometry() {
            assert_eq!(*point, scene::point(&row["point"]));
        }
    }
    json!({"sample_error":maximum,"sample_surface":maximum<=1e-6,"boundary_certificate":planar_match,"counts":counts,"topology":topology,"metadata":metadata,"ngon_boundaries":ngons,"normals":normals})
}

pub fn rows() -> Vec<(&'static str, Value)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    let mut rows = Vec::new();
    for (family, count) in [
        ("mesh_cap_regions", 14),
        ("mesh_cap_regions_groups", 6),
        ("mesh_cap_regions_noop", 2),
    ] {
        let value: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(value["results"].as_array().unwrap().len(), count);
        rows.extend(
            value["results"]
                .as_array()
                .unwrap()
                .iter()
                .cloned()
                .map(|r| (family, r)),
        );
    }
    rows
}

pub fn setup(row: &Value, family: &str) -> (Document, Vec<ObjectId>) {
    let (doc, ids) = scene::setup_from_family_with_tolerance(
        row,
        family,
        Some(Tolerance::try_new(1e-9, 1e-12, 1e-15).unwrap()),
    );
    for (index, id) in ids.iter().enumerate() {
        let Geometry::Mesh(mesh) = doc.object(*id).unwrap().geometry() else {
            panic!()
        };
        let source = row["value"]["before"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == index)
            .unwrap();
        assert_eq!(
            json!(
                mesh.vertices()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>()
            ),
            source["mesh"]["vertices"],
            "exact source {}",
            row["id"]
        );
        assert_eq!(
            json!(mesh.faces().iter().map(|f| f.indices()).collect::<Vec<_>>()),
            source["mesh"]["faces"]
        );
    }
    (doc, ids)
}

pub fn command(value: &Value) -> String {
    format!(
        "Cap {}",
        value["input"]["command_arguments"]
            .as_str()
            .unwrap_or_default()
            .replace('_', "")
    )
}

pub fn assert_diagnostics(diagnostics: &[Value]) {
    assert_eq!(diagnostics.len(), 22);
    let gaps: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/mesh_cap_regions_retained_gaps.json"
    ))
    .unwrap();
    for phase in ["after", "after_script", "redo"] {
        for field in [
            "boundary_certificate",
            "counts",
            "topology",
            "sample_surface",
        ] {
            let actual = diagnostics
                .iter()
                .filter(|r| r[phase][field] != true)
                .map(|r| r["id"].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                json!(actual),
                gaps[field],
                "unexplained Cap change: {phase}/{field}"
            );
        }
    }
    for phase in ["after", "after_script", "redo"] {
        for (field, predicate) in [("normals", false), ("uncompared_normals", true)] {
            let actual = diagnostics
                .iter()
                .filter(|r| {
                    if predicate {
                        r[phase]["normals"].is_null()
                    } else {
                        r[phase]["normals"] == false
                    }
                })
                .map(|r| r["id"].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                json!(actual),
                gaps[field],
                "unexplained Cap normal change: {phase}/{field}"
            );
        }
    }
    for r in diagnostics {
        for phase in ["before", "after", "after_script", "undo", "redo"] {
            assert_eq!(r[phase]["metadata"], true, "{}/{}", r["id"], phase);
            assert_eq!(r[phase]["ngon_boundaries"], true, "{}/{}", r["id"], phase);
        }
        for phase in ["before", "undo"] {
            for field in ["boundary_certificate", "counts", "topology", "normals"] {
                assert_eq!(r[phase][field], true);
            }
        }
    }
}
