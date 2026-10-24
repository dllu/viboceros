#![cfg(feature = "native")]
use serde_json::{Value, json};
use viboceros_geometry::{MeshFace, Point3, Tolerance, TriangleMesh};
use viboceros_smlib::{BooleanOperation, mesh_boolean};

#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod reference;

#[path = "support/mesh_boolean.rs"]
mod coverage;
use coverage::sampled_distance;

#[test]
fn recorded_native_mesh_boolean_geometry() {
    let mut rows = Vec::new();
    for text in [
        include_str!("../../../tools/rhino_oracle/observations/mesh_boolean_sdk.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_boolean_controls.json"),
    ] {
        let capture: Value = serde_json::from_str(text).unwrap();
        rows.extend(capture["results"].as_array().unwrap().iter().cloned());
    }
    let mut errors = Vec::new();
    let mut matched = 0;
    for row in &rows {
        let value = &row["value"];
        let input = &value["input"];
        let a = reference::mesh(&input["meshes"][0]);
        let b = reference::mesh(&input["meshes"][1]);
        let before = (a.clone(), b.clone());
        let operation = match input["operation"].as_str().unwrap() {
            "union" => BooleanOperation::Union,
            "intersection" => BooleanOperation::Intersection,
            "difference" => BooleanOperation::Difference,
            _ => unreachable!(),
        };
        let expected = value["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .map(reference::mesh)
            .collect::<Vec<_>>();
        let (actual, error) = match mesh_boolean(&a, &b, operation, Tolerance::DEFAULT) {
            Ok(mesh) => (mesh.into_iter().collect::<Vec<_>>(), None),
            Err(error) => {
                errors.push(row["id"].clone());
                (vec![], Some(error.to_string()))
            }
        };
        assert_eq!((&a, &b), (&before.0, &before.1));
        let deviation = error
            .is_none()
            .then(|| sampled_distance(&actual, &expected))
            .flatten();
        let name = row["id"].as_str().unwrap();
        let gap = name.starts_with("identical_")
            || (name.starts_with("face_touch_") && name != "face_touch_union")
            || (name.starts_with("coincident_") && name != "coincident_face_touch_union");
        assert_eq!(deviation.is_some_and(|e| e <= 1e-7), !gap, "{name}");
        matched += usize::from(!gap);
        eprintln!(
            "MESH_BOOLEAN_SDK {}",
            json!({"case":row["id"],"kernel_error":error,
                "maximum_sampled_boundary_deviation":deviation,
                "area":actual.iter().map(|m| m.area().unwrap()).sum::<f64>(),
                "signed_volume":actual.iter().map(|m| m.signed_volume().unwrap()).sum::<f64>(),
                "source_unchanged":a==before.0 && b==before.1,
                "meshes":actual.iter().map(|m| json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.triangles()})).collect::<Vec<_>>()
            })
        );
    }
    assert!(
        errors.is_empty(),
        "native polygon Boolean failures: {errors:?}"
    );
    assert_eq!((rows.len(), matched), (48, 26));
}

fn box_mesh(origin: [f64; 3]) -> TriangleMesh {
    let vertices = [0., 2.]
        .into_iter()
        .flat_map(|z| {
            [0., 3.].into_iter().flat_map(move |y| {
                [0., 4.].into_iter().map(move |x| {
                    Point3::try_new(origin[0] + x, origin[1] + y, origin[2] + z).unwrap()
                })
            })
        })
        .collect();
    let faces = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ]
    .into_iter()
    .map(MeshFace::Quad)
    .collect();
    TriangleMesh::try_from_face_records(vertices, faces).unwrap()
}

#[test]
fn native_polygon_booleans_have_expected_boundaries_and_preserve_sources() {
    let a = box_mesh([0., 0., 0.])
        .try_with_texture_coordinates(Some(vec![[0.25, 0.75]; 8]))
        .unwrap()
        .try_with_vertex_colors(Some(vec![[13, 27, 53, 255]; 8]))
        .unwrap();
    let b = box_mesh([2., 1., 0.5]);
    let before = (a.clone(), b.clone());
    for (operation, volume, area) in [
        (BooleanOperation::Union, 42., 84.),
        (BooleanOperation::Intersection, 6., 20.),
        (BooleanOperation::Difference, 18., 52.),
    ] {
        let result = mesh_boolean(&a, &b, operation, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(
            result.topology().is_solid(),
            "{operation:?}: {:?}",
            result.topology()
        );
        assert!(
            (result.signed_volume().unwrap() - volume).abs() < 1e-8,
            "{operation:?}"
        );
        assert!(
            (result.area().unwrap() - area).abs() < 1e-8,
            "{operation:?}"
        );
        assert_eq!((&a, &b), (&before.0, &before.1));
    }
}

#[test]
fn through_holes_and_coincident_volumes_keep_set_operation_semantics() {
    let a = box_mesh([0., 0., 0.]);
    let template = box_mesh([0., 0., 0.]);
    let cutter = TriangleMesh::try_from_face_records(
        template
            .vertices()
            .iter()
            .map(|p| Point3::try_new(1. + p.x() / 4., 1. + p.y() / 3., -1. + p.z() * 2.).unwrap())
            .collect(),
        template.faces().to_vec(),
    )
    .unwrap();
    let result = mesh_boolean(
        &a,
        &cutter,
        BooleanOperation::Difference,
        Tolerance::DEFAULT,
    )
    .unwrap()
    .unwrap();
    assert!(result.topology().is_solid());
    assert!((result.signed_volume().unwrap() - 22.).abs() < 1e-8);
    assert!((result.area().unwrap() - 58.).abs() < 1e-8);
    for operation in [BooleanOperation::Union, BooleanOperation::Intersection] {
        let result = mesh_boolean(&a, &a, operation, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!((result.signed_volume().unwrap() - 24.).abs() < 1e-8);
        assert!((result.area().unwrap() - 52.).abs() < 1e-8);
    }
}

#[test]
fn empty_results_and_repeated_native_ownership_are_handled() {
    let a = box_mesh([0., 0., 0.]);
    let b = box_mesh([8., 0., 0.]);
    for _ in 0..3 {
        assert!(
            mesh_boolean(&a, &b, BooleanOperation::Intersection, Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
        assert!(
            mesh_boolean(&a, &a, BooleanOperation::Difference, Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
        let result = mesh_boolean(&a, &b, BooleanOperation::Union, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!((result.signed_volume().unwrap() - 48.).abs() < 1e-8);
    }
}
