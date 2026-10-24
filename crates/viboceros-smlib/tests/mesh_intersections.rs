#![cfg(feature = "native")]
use serde_json::Value;
use std::collections::BTreeSet;
use viboceros_geometry::{Point3, Tolerance};
use viboceros_smlib::mesh_intersections;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod reference;

fn input(name: &str) -> Vec<viboceros_geometry::TriangleMesh> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_intersect_discovery.json"
    ))
    .unwrap();
    let row = data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == name)
        .unwrap();
    row["value"]["input"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(reference::mesh)
        .collect()
}
fn unique_length(segments: &[[Point3; 2]]) -> f64 {
    let mut seen = BTreeSet::new();
    segments
        .iter()
        .filter(|[a, b]| {
            let key = |p: Point3| p.to_array().map(|v| if v == 0. { 0 } else { v.to_bits() });
            let (a, b) = (key(*a), key(*b));
            seen.insert(if a < b { [a, b] } else { [b, a] })
        })
        .map(|[a, b]| a.distance_to(*b).unwrap())
        .sum()
}

fn coplanar_segments(result: &viboceros_smlib::MeshIntersections) -> Vec<[Point3; 2]> {
    result
        .coplanar
        .iter()
        .flat_map(|p| p.vertices().windows(2))
        .map(|p| [p[0], p[1]])
        .collect()
}

#[test]
fn open_crossings_and_coplanar_boundaries_do_not_require_closed_solids() {
    for (name, crossing, coplanar) in [
        ("mesh_intersect_crossed_drop", 3., 0.),
        ("mesh_intersect_coplanar_polylines", 0., 8.),
        ("mesh_intersect_overlap_post", 11., 0.),
        ("mesh_intersect_disjoint_post", 0., 0.),
        ("mesh_intersect_contained_post", 0., 0.),
        ("mesh_intersect_point_touch_post", 0., 0.),
    ] {
        let meshes = input(name);
        let before = meshes.clone();
        let actual = mesh_intersections(&meshes[0], &meshes[1], Tolerance::DEFAULT).unwrap();
        assert!(
            (unique_length(&actual.transverse) - crossing).abs() < 1e-7,
            "{name}: {actual:?}"
        );
        assert!(
            (unique_length(&coplanar_segments(&actual)) - coplanar).abs() < 1e-7,
            "{name}: {actual:?}"
        );
        assert_eq!(meshes, before);
    }
}

#[test]
fn repeated_owned_intersections_preserve_input_mesh_storage() {
    let meshes = input("mesh_intersect_overlap_post");
    let before = meshes.clone();
    for _ in 0..24 {
        let actual = mesh_intersections(&meshes[0], &meshes[1], Tolerance::DEFAULT).unwrap();
        assert!((unique_length(&actual.transverse) - 11.).abs() < 1e-7);
        assert_eq!(meshes, before);
    }
}

#[test]
fn coplanar_region_classification_is_independent_of_coordinate_plane() {
    let originals = input("mesh_intersect_coplanar_polylines");
    for rotation in 0..3 {
        let meshes = originals
            .iter()
            .map(|m| {
                let vertices = m
                    .vertices()
                    .iter()
                    .map(|p| {
                        let a = p.to_array();
                        Point3::try_from([
                            a[rotation],
                            a[(rotation + 1) % 3],
                            a[(rotation + 2) % 3],
                        ])
                        .unwrap()
                    })
                    .collect();
                viboceros_geometry::TriangleMesh::try_from_face_records(
                    vertices,
                    m.faces().to_vec(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let output = mesh_intersections(&meshes[0], &meshes[1], Tolerance::DEFAULT).unwrap();
        assert!(
            output.transverse.is_empty(),
            "rotation {rotation}: {output:?}"
        );
        assert!((unique_length(&coplanar_segments(&output)) - 8.).abs() < 1e-7);
    }
}

#[test]
fn contained_coplanar_patch_does_not_fill_the_surrounding_faces_hole() {
    use viboceros_geometry::{MeshFace, TriangleMesh};
    let outer = input("mesh_intersect_coplanar_polylines").remove(0);
    let vertices = [
        [1.1, 1.8, 0.],
        [1.6, 1.8, 0.],
        [1.6, 2.3, 0.],
        [1.1, 2.3, 0.],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let inner =
        TriangleMesh::try_from_face_records(vertices, vec![MeshFace::Quad([0, 1, 2, 3])]).unwrap();
    let result = mesh_intersections(&outer, &inner, Tolerance::DEFAULT).unwrap();
    assert!(result.transverse.is_empty(), "{result:?}");
    assert!(
        (unique_length(&coplanar_segments(&result)) - 2.).abs() < 1e-7,
        "{result:?}"
    );
}

#[test]
fn coplanar_coverage_preserves_holes_and_ignores_face_winding() {
    use viboceros_geometry::{MeshFace, TriangleMesh};
    let mesh = |points: &[[f64; 3]], faces| {
        TriangleMesh::try_from_face_records(
            points
                .iter()
                .copied()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            faces,
        )
        .unwrap()
    };
    let outer = mesh(
        &[[-1., -1., 0.], [5., -1., 0.], [5., 5., 0.], [-1., 5., 0.]],
        vec![MeshFace::Quad([0, 1, 2, 3])],
    );
    let points = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 4., 0.],
        [0., 4., 0.],
        [1., 1., 0.],
        [3., 1., 0.],
        [3., 3., 0.],
        [1., 3., 0.],
    ];
    for reverse in [false, true] {
        let faces = [[0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]]
            .into_iter()
            .map(|mut face| {
                if reverse {
                    face.reverse();
                }
                MeshFace::Quad(face)
            })
            .collect();
        let ring = mesh(&points, faces);
        for (a, b) in [(&outer, &ring), (&ring, &outer)] {
            let result = mesh_intersections(a, b, Tolerance::DEFAULT).unwrap();
            assert!(
                result.transverse.is_empty(),
                "unwanted internal line: {result:?}"
            );
            assert!(
                (unique_length(&coplanar_segments(&result)) - 24.).abs() < 1e-7,
                "{result:?}"
            );
            for segment in coplanar_segments(&result) {
                let [a, b] = segment.map(Point3::to_array);
                let midpoint = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.];
                let on_outer = midpoint
                    .iter()
                    .any(|v| v.abs() < 1e-7 || (*v - 4.).abs() < 1e-7);
                let on_inner = midpoint.iter().all(|v| *v >= 1. - 1e-7 && *v <= 3. + 1e-7)
                    && midpoint
                        .iter()
                        .any(|v| (*v - 1.).abs() < 1e-7 || (*v - 3.).abs() < 1e-7);
                assert!(on_outer || on_inner, "spurious internal edge: {segment:?}");
            }
        }
    }
}

#[test]
fn partial_coplanar_edge_contacts_survive_without_shared_area() {
    use viboceros_geometry::{MeshFace, TriangleMesh};
    let outer = input("mesh_intersect_coplanar_polylines").remove(0);
    let adjacent = TriangleMesh::try_from_face_records(
        [[4., 1., 0.], [6., 1., 0.], [6., 2., 0.], [4., 2., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap();
    for (a, b) in [(&outer, &adjacent), (&adjacent, &outer)] {
        let result = mesh_intersections(a, b, Tolerance::DEFAULT).unwrap();
        assert!(result.coplanar.is_empty(), "{result:?}");
        assert!(result.transverse.is_empty(), "{result:?}");
        assert!(
            (unique_length(&result.tangential) - 1.).abs() < 1e-7,
            "{result:?}"
        );
    }
    let result = mesh_intersections(&outer, &outer, Tolerance::DEFAULT).unwrap();
    assert!(
        (unique_length(&coplanar_segments(&result)) - 14.).abs() < 1e-7,
        "{result:?}"
    );
    assert!(
        (unique_length(&result.transverse) - 5.).abs() < 1e-7,
        "{result:?}"
    );
}

#[test]
fn crossings_and_pure_boundary_contacts_follow_public_sdk_classification() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_intersect_mixed.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        let value = &row["value"];
        let meshes = value["input"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(reference::mesh)
            .collect::<Vec<_>>();
        let before = meshes.clone();
        let actual = mesh_intersections(&meshes[0], &meshes[1], Tolerance::DEFAULT).unwrap();
        let sdk = &value["intersection_sdk"]["modern"];
        let length = |paths: &Value| {
            paths
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p.as_array().unwrap().windows(2))
                .map(|p| {
                    reference::point(&p[0])
                        .distance_to(reference::point(&p[1]))
                        .unwrap()
                })
                .sum::<f64>()
        };
        assert!(sdk["success"] == true, "{sdk:?}");
        // The public API truncates half a unit at this coplanar/crossing
        // junction. Independent triangle pairs retain the complete line.
        let omitted = if matches!(
            row["id"].as_str().unwrap(),
            "mesh_intersect_mixed_corner" | "mesh_intersect_mixed_curtain"
        ) {
            0.5
        } else {
            0.
        };
        assert!(
            (unique_length(&actual.transverse) - length(&sdk["polylines"]) - omitted).abs() < 1e-7,
            "{}: {actual:?}",
            row["id"]
        );
        assert_eq!(
            actual.coplanar.len(),
            sdk["coplanar"].as_array().unwrap().len(),
            "{}",
            row["id"]
        );
        assert!(
            (unique_length(&coplanar_segments(&actual)) - length(&sdk["coplanar"])).abs() < 1e-7,
            "{}: {actual:?}",
            row["id"]
        );
        assert_eq!(meshes, before);
    }
    let meshes = input("mesh_intersect_edge_touch_post");
    let actual = mesh_intersections(&meshes[0], &meshes[1], Tolerance::DEFAULT).unwrap();
    assert!(actual.transverse.is_empty(), "{actual:?}");
    assert!(
        (unique_length(&actual.tangential) - 2.).abs() < 1e-7,
        "{actual:?}"
    );
}

#[test]
fn crossing_coverage_survives_coplanar_face_order_and_operand_swap() {
    use viboceros_geometry::TriangleMesh;
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_intersect_mixed.json"
    ))
    .unwrap();
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "mesh_intersect_mixed_two_regions")
        .unwrap()["value"];
    let a = reference::mesh(&value["input"]["sources"][0]);
    let b = reference::mesh(&value["input"]["sources"][1]);
    let mut faces = b.faces().to_vec();
    faces.reverse();
    let reordered = TriangleMesh::try_from_face_records(b.vertices().to_vec(), faces).unwrap();
    for b in [&b, &reordered] {
        for (a, b) in [(&a, b), (b, &a)] {
            let result = mesh_intersections(a, b, Tolerance::DEFAULT).unwrap();
            assert!(
                (unique_length(&result.transverse) - 4.).abs() < 1e-7,
                "{result:?}"
            );
            assert_eq!(result.coplanar.len(), 2);
            for point in result.transverse.iter().flatten() {
                assert!((point.y() - 1.8).abs() < 1e-7 && point.z().abs() < 1e-7);
            }
        }
    }
}
