#![cfg(feature = "native")]
use serde_json::{Value, json};
use viboceros_geometry::{Point3, Tolerance, TriangleMesh};
use viboceros_smlib::{mesh_split, mesh_split_with_face_sources};

#[test]
fn face_lineage_survives_multiple_imprints_triangulation_and_component_copies() {
    let row = rows("discovery")
        .into_iter()
        .find(|r| r["id"] == "mesh_split_two_cutters")
        .unwrap();
    let inputs = inputs(&row);
    let source = &inputs[0];
    let parts = mesh_split_with_face_sources(
        source,
        &inputs[1..].iter().collect::<Vec<_>>(),
        true,
        Tolerance::try_new(1e-10, 1e-12, 1e-6).unwrap(),
    )
    .unwrap();
    assert_eq!(parts.len(), 4);
    let mut covered = vec![0.; source.faces().len()];
    for part in parts {
        assert_eq!(part.source_faces.len(), part.mesh.faces().len());
        for (&face, &parent) in part.mesh.faces().iter().zip(&part.source_faces) {
            let original = source.faces()[parent].indices();
            let [a, b, c] = [0, 1, 2].map(|i| source.vertices()[original[i] as usize]);
            let normal = a
                .vector_to(b)
                .unwrap()
                .cross(a.vector_to(c).unwrap())
                .unwrap()
                .normalized_nonzero()
                .unwrap()
                .as_vector();
            for &v in face.indices() {
                let p = part.mesh.vertices()[v as usize];
                assert!(a.vector_to(p).unwrap().dot(normal).unwrap().abs() < 1e-7);
            }
            covered[parent] +=
                TriangleMesh::try_from_face_records(part.mesh.vertices().to_vec(), vec![face])
                    .unwrap()
                    .area()
                    .unwrap();
        }
    }
    for (i, &area) in covered.iter().enumerate() {
        let expected = TriangleMesh::try_from_face_records(
            source.vertices().to_vec(),
            vec![source.faces()[i]],
        )
        .unwrap()
        .area()
        .unwrap();
        assert!((area - expected).abs() < 1e-7, "source face {i}");
    }
}
#[path = "support/mesh_boolean.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod reference;

fn rows(family: &str) -> Vec<Value> {
    let text = match family {
        "discovery" => {
            include_str!("../../../tools/rhino_oracle/observations/mesh_split_discovery.json")
        }
        "controls" => {
            include_str!("../../../tools/rhino_oracle/observations/mesh_split_controls.json")
        }
        _ => unreachable!(),
    };
    serde_json::from_str::<Value>(text).unwrap()["results"]
        .as_array()
        .unwrap()
        .clone()
}
fn inputs(row: &Value) -> Vec<TriangleMesh> {
    row["value"]["input"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(reference::mesh)
        .collect()
}
fn mesh_value(mesh: &TriangleMesh) -> Value {
    json!({"vertices":mesh.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":mesh.triangles()})
}
// Open-part volume depends on the integration origin. Use the same origin
// for every part, so their oriented integrals remain additive.
fn origin_volume(mesh: &TriangleMesh) -> f64 {
    mesh.triangles()
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.vertices()[i as usize].to_array());
            (a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.
        })
        .sum()
}
fn oriented_area(mesh: &TriangleMesh) -> [f64; 3] {
    let mut sum = [0.; 3];
    for t in mesh.triangles() {
        let [a, b, c] = t.map(|i| mesh.vertices()[i as usize]);
        let n = a
            .vector_to(b)
            .unwrap()
            .cross(a.vector_to(c).unwrap())
            .unwrap()
            .to_array();
        for axis in 0..3 {
            sum[axis] += n[axis] * 0.5;
        }
    }
    sum
}

fn same_part(a: &TriangleMesh, b: &TriangleMesh) -> bool {
    (a.area().unwrap() - b.area().unwrap()).abs() < 1e-7
        && (origin_volume(a) - origin_volume(b)).abs() < 1e-7
        && (0..3).all(|i| (oriented_area(a)[i] - oriented_area(b)[i]).abs() < 1e-7)
        && comparison::sampled_distance(std::slice::from_ref(a), std::slice::from_ref(b))
            .is_some_and(|d| d < 1e-7)
}
fn matched(actual: &[TriangleMesh], expected: &[TriangleMesh]) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut used = vec![false; expected.len()];
    actual.iter().all(|a| {
        if let Some(index) = expected
            .iter()
            .enumerate()
            .position(|(i, b)| !used[i] && same_part(a, b))
        {
            used[index] = true;
            true
        } else {
            false
        }
    })
}

#[test]
fn open_mesh_partitions_replay_native_command_geometry_and_public_sdk() {
    let mut gaps = Vec::new();
    let mut count = 0;
    for family in ["discovery", "controls"] {
        for row in rows(family) {
            let name = row["id"].as_str().unwrap();
            let value = &row["value"];
            let input = &value["input"];
            if input["cancel"] == true
                || input["cancel_second"] == true
                || input["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s["kind"] != "mesh")
            {
                continue;
            }
            let meshes = inputs(&row);
            let before = meshes.clone();
            let cutters = input["second"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| &meshes[i.as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            let tolerance = Tolerance::try_new(
                value["document_tolerance"].as_f64().unwrap() * 1e-4,
                1e-12,
                1e-6,
            )
            .unwrap();
            let mut actual = Vec::new();
            let mut error = None;
            for target in input["first"].as_array().unwrap() {
                match mesh_split(
                    &meshes[target.as_u64().unwrap() as usize],
                    &cutters,
                    input["split_coplanar"] != false,
                    tolerance,
                ) {
                    Ok(parts) if family == "controls" || parts.len() > 1 => actual.extend(parts),
                    Ok(_) => (),
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                }
            }
            assert_eq!(meshes, before, "{name} changed a source");
            let expected = if family == "controls" {
                value["split_sdk"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|target| target["parts"].as_array().unwrap())
                    .map(reference::mesh)
                    .collect::<Vec<_>>()
            } else {
                value["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|o| o["role"] == "output")
                    .map(|o| reference::mesh(&o["mesh"]))
                    .collect::<Vec<_>>()
            };
            let distance = comparison::sampled_distance(&actual, &expected);
            let matched = error.is_none() && matched(&actual, &expected);
            println!(
                "MESH_SPLIT_KERNEL {}",
                json!({"id":name,"family":family,"error":error,"matched":matched,
                "parts":actual.iter().map(mesh_value).collect::<Vec<_>>(),
                "maximum_sampled_boundary_deviation":distance,"area":actual.iter().map(|m|m.area().unwrap()).sum::<f64>(),
                "signed_volume":actual.iter().map(origin_volume).sum::<f64>(),"oriented_area":(0..3).map(|i|actual.iter().map(|m|oriented_area(m)[i]).sum::<f64>()).collect::<Vec<_>>(),"sources_unchanged":meshes==before})
            );
            if !matched {
                gaps.push(name.to_owned());
            }
            count += 1;
        }
    }
    assert_eq!(count, 30);
    assert_eq!(
        gaps,
        [
            "mesh_split_controls_mixed_cutter",
            "mesh_split_controls_three_cutters"
        ]
    );
}

#[test]
fn repeated_partitions_remain_open_and_preserve_surface_area_and_input_storage() {
    let row = rows("discovery")
        .into_iter()
        .find(|r| r["id"] == "mesh_split_two_cutters")
        .unwrap();
    let meshes = inputs(&row);
    let before = meshes.clone();
    for _ in 0..16 {
        let parts = mesh_split(
            &meshes[0],
            &[&meshes[1], &meshes[2]],
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(parts.len(), 4);
        assert!(
            (parts.iter().map(|p| p.area().unwrap()).sum::<f64>() - meshes[0].area().unwrap())
                .abs()
                < 1e-7
        );
        assert!(
            (parts.iter().map(origin_volume).sum::<f64>() - origin_volume(&meshes[0])).abs() < 1e-7
        );
        assert!(parts.iter().all(|p| !p.topology().is_closed()));
        assert_eq!(meshes, before);
    }
}

#[test]
fn late_invalid_cutters_do_not_change_valid_meshes() {
    let row = rows("discovery").remove(0);
    let meshes = inputs(&row);
    let before = meshes.clone();
    let bad = TriangleMesh::try_from_face_records(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![viboceros_geometry::MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    assert!(mesh_split(&meshes[0], &[&meshes[1], &bad], true, Tolerance::DEFAULT).is_err());
    assert_eq!(meshes, before);
}

#[test]
fn three_crossing_cutters_preserve_the_complete_source_surface() {
    let row = rows("controls")
        .into_iter()
        .find(|r| r["id"] == "mesh_split_controls_three_cutters")
        .unwrap();
    let meshes = inputs(&row);
    let parts = mesh_split(
        &meshes[0],
        &meshes[1..].iter().collect::<Vec<_>>(),
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(parts.len(), 8);
    assert!((parts.iter().map(|m| m.area().unwrap()).sum::<f64>() - 52.).abs() < 1e-7);
    assert!((parts.iter().map(origin_volume).sum::<f64>() - 24.).abs() < 1e-7);
}
