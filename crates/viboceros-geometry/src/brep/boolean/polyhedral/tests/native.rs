//! Public SDK evidence: preserve open and orientation-dependent countercases.
use super::*;
use crate::brep::boolean::tests::native::{Boundary, boundary, on_boundary, xyz};
use serde_json::Value;

#[test]
fn replays_24_physical_sdk_results_and_preserves_six_semantic_differences() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../../../tools/rhino_oracle/fixtures/polyhedral_boolean.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../../../../tools/rhino_oracle/observations/polyhedral_boolean.json"
    ))
    .unwrap();
    let mut matched = 0;
    let mut differences = Vec::new();
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let case = op["case"].as_str().unwrap();
        let name = op["operation"].as_str().unwrap();
        let label = format!("{case}_{name}");
        let operation = match name {
            "union" => BrepBooleanOperation::Union,
            "intersection" => BrepBooleanOperation::Intersection,
            "difference" => BrepBooleanOperation::Difference,
            _ => unreachable!(),
        };
        let (a, b) = source(case);
        let before = (a.clone(), b.clone());
        let result = a.try_boolean_polyhedral(&b, operation, Tolerance::DEFAULT);
        assert_eq!((&a, &b), (&before.0, &before.1));
        let outputs = row["value"]["outputs"].as_array().unwrap();
        assert!(!outputs.is_empty());
        if label == "singular_two_holes_intersection" {
            assert!(matches!(
                result,
                Err(GeometryError::UnrepresentableBrepBoolean)
            ));
            assert!(outputs.iter().any(|g| !g["solid"].as_bool().unwrap()));
            differences.push(label);
            continue;
        }
        let brep = result.unwrap_or_else(|e| panic!("{label}: {e}")).unwrap();
        assert!(
            outputs
                .iter()
                .all(|g| g["solid"].as_bool().unwrap() && g["valid"].as_bool().unwrap())
        );
        let mass = brep.volume_mass_properties(Tolerance::DEFAULT).unwrap();
        let volume: Real = outputs.iter().map(|g| g["volume"].as_f64().unwrap()).sum();
        let area: Real = outputs.iter().map(|g| g["area"].as_f64().unwrap()).sum();
        if matches!(
            label.as_str(),
            "cavity_intersection"
                | "island_intersection"
                | "reversed_hole_union"
                | "reversed_hole_intersection"
                | "reversed_hole_difference"
        ) {
            assert!(
                (mass.signed_volume().unwrap() - volume).abs() > 1e-10
                    || (brep.area(Tolerance::DEFAULT).unwrap() - area).abs() > 1e-9
            );
            differences.push(label);
            continue;
        }
        assert!(
            (mass.signed_volume().unwrap() - volume).abs() < 1e-10,
            "{label} volume"
        );
        assert!(
            (brep.area(Tolerance::DEFAULT).unwrap() - area).abs() < 1e-9,
            "{label} area"
        );
        let centroid: [Real; 3] = std::array::from_fn(|i| {
            outputs
                .iter()
                .map(|g| g["volume"].as_f64().unwrap() * g["centroid"][i].as_f64().unwrap())
                .sum::<Real>()
                / volume
        });
        assert!(
            mass.centroid()
                .unwrap()
                .distance_to(Point3::try_from(centroid).unwrap())
                .unwrap()
                < 1e-10,
            "{label} centroid"
        );
        let bounds = BoundingBox3::from_points(brep.vertices.iter().map(|v| v.point)).unwrap();
        for (side, p) in [bounds.min(), bounds.max()].into_iter().enumerate() {
            for (i, v) in p.to_array().into_iter().enumerate() {
                let expected = outputs
                    .iter()
                    .map(|g| g["bounds"][side][i].as_f64().unwrap())
                    .reduce(|a, b| if side == 0 { a.min(b) } else { a.max(b) })
                    .unwrap();
                assert!((v - expected).abs() < 1e-10, "{label} bounds");
            }
        }
        let own = boundary(&brep);
        let native: Boundary = outputs
            .iter()
            .flat_map(|g| g["faces"].as_array().unwrap())
            .map(|f| {
                f["loops"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|l| l.as_array().unwrap().iter().map(xyz).collect())
                    .collect()
            })
            .collect();
        for g in outputs {
            for p in g["vertices"].as_array().unwrap().iter().chain(
                g["edge_samples"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|e| e.as_array().unwrap()),
            ) {
                assert!(on_boundary(xyz(p), &own), "{label} native witness {p}");
            }
        }
        for face in own {
            let ring = &face[0];
            let center = std::array::from_fn(|i| {
                ring.iter().map(|p| p[i]).sum::<Real>() / ring.len() as Real
            });
            assert!(on_boundary(center, &native), "{label} cell interior");
            for i in 0..ring.len() {
                assert!(on_boundary(ring[i], &native), "{label} kernel vertex");
                let midpoint =
                    std::array::from_fn(|j| (ring[i][j] + ring[(i + 1) % ring.len()][j]) * 0.5);
                assert!(
                    on_boundary(midpoint, &native),
                    "{label} kernel edge midpoint"
                );
            }
        }
        matched += 1;
    }
    eprintln!("Retained native polyhedral Boolean differences: {differences:?}");
    assert_eq!(matched, 24);
    assert_eq!(
        differences,
        [
            "cavity_intersection",
            "island_intersection",
            "singular_two_holes_intersection",
            "reversed_hole_union",
            "reversed_hole_intersection",
            "reversed_hole_difference"
        ]
    );
}
