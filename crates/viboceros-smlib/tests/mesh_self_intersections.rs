#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{Point3, Tolerance, TriangleMesh};
use viboceros_smlib::mesh_self_intersections;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod reference;

fn input(name: &str) -> TriangleMesh {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_self_intersect_discovery.json"
    ))
    .unwrap();
    let row = data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == format!("mesh_self_intersect_{name}_post"))
        .unwrap();
    reference::mesh(&row["value"]["input"]["sources"][0])
}
fn length(lines: &[[Point3; 2]]) -> f64 {
    let mut unique = std::collections::BTreeSet::new();
    lines
        .iter()
        .filter(|[a, b]| {
            let key = |p: Point3| p.to_array().map(|v| if v == 0. { 0 } else { v.to_bits() });
            let (a, b) = (key(*a), key(*b));
            unique.insert(if a < b { [a, b] } else { [b, a] })
        })
        .map(|[a, b]| a.distance_to(*b).unwrap())
        .sum()
}
fn measures(mesh: &TriangleMesh) -> (f64, f64) {
    let result = mesh_self_intersections(mesh, Tolerance::DEFAULT).unwrap();
    let lines = result
        .transverse
        .into_iter()
        .chain(result.tangential)
        .collect::<Vec<_>>();
    let contours = result
        .coplanar
        .iter()
        .flat_map(|p| p.vertices().windows(2))
        .map(|s| [s[0], s[1]])
        .collect::<Vec<_>>();
    (length(&lines), length(&contours))
}

#[test]
fn crossings_and_overlaps_exclude_ordinary_adjacency_and_internal_seams() {
    for (name, line, area) in [
        ("flat", 0., 0.),
        ("box", 0., 0.),
        ("adjacent", 0., 0.),
        ("crossing", 3., 0.),
        ("shared_vertex", 8_f64.sqrt(), 0.),
        ("coplanar", 0., 8.),
        ("identical_faces", 0., 14.),
        ("folded", 0., 2. + 2. * 2_f64.sqrt()),
    ] {
        let mesh = input(name);
        let before = mesh.clone();
        let (actual_line, actual_area) = measures(&mesh);
        assert!(
            (actual_line - line).abs() < 1e-7 && (actual_area - area).abs() < 1e-7,
            "{name}: {actual_line}, {actual_area}"
        );
        assert_eq!(mesh, before);
    }
}

#[test]
fn self_intersections_preserve_input_and_are_covariant_under_face_order_and_rotation() {
    for name in ["crossing", "coplanar"] {
        let original = input(name);
        let expected = measures(&original);
        let mut faces = original.faces().to_vec();
        faces.reverse();
        for rotation in 0..3 {
            let vertices = original
                .vertices()
                .iter()
                .map(|p| {
                    let a = p.to_array();
                    Point3::try_from([
                        a[rotation] + 7.,
                        a[(rotation + 1) % 3] - 5.,
                        a[(rotation + 2) % 3] + 3.,
                    ])
                    .unwrap()
                })
                .collect();
            let mesh = TriangleMesh::try_from_face_records(vertices, faces.clone()).unwrap();
            let before = mesh.clone();
            for _ in 0..8 {
                let actual = measures(&mesh);
                assert!(
                    (actual.0 - expected.0).abs() < 1e-7 && (actual.1 - expected.1).abs() < 1e-7
                );
                assert_eq!(mesh, before);
            }
        }
    }
}

#[test]
fn coincident_coverage_is_counted_once_with_three_overlapping_faces() {
    let original = input("flat");
    let mesh = TriangleMesh::try_from_face_records(
        original.vertices().to_vec(),
        original.faces().iter().cycle().take(3).copied().collect(),
    )
    .unwrap();
    assert_eq!(measures(&mesh), (0., 14.));
}
