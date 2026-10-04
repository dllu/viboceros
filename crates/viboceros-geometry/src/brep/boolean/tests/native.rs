//! Replay successful public SDK outputs, with retained no-result countercases.
use super::*;
use serde_json::Value;

type Boundary = Vec<Vec<Vec<[Real; 3]>>>;

fn xyz(value: &Value) -> [Real; 3] {
    std::array::from_fn(|i| value[i].as_f64().unwrap())
}

fn source(case: &str) -> (Brep, Brep) {
    let mut a = if case.starts_with("tetra_") {
        tetra([0.; 3])
    } else {
        cube([[0., 2.]; 3])
    };
    let mut b = match case {
        "corner" | "sheared" => cube([[1., 3.]; 3]),
        "pierce" => cube([[0.5, 1.5], [0.5, 1.5], [-1., 3.]]),
        "disjoint" => cube([[4., 5.]; 3]),
        "contained" => cube([[0.5, 1.5]; 3]),
        "contains" => cube([[-1., 3.]; 3]),
        "equal" => a.clone(),
        "touch_face" => cube([[2., 4.], [0., 2.], [0., 2.]]),
        "boundary_contained" => cube([[0., 1.]; 3]),
        "partial_coplanar" => cube([[1., 3.], [0., 2.], [0.5, 1.5]]),
        "tetra_box" => cube([[0.5, 2.]; 3]),
        "tetra_tetra" => tetra([0.5; 3]),
        _ => panic!("unknown closed recipe"),
    };
    if case == "sheared" {
        let transform = AffineTransform3::try_new(
            [[1., 1., 0.], [0., 1., 0.5], [0., 0., 1.]],
            Vector3::try_new(10., -4., 0.).unwrap(),
        )
        .unwrap();
        a = a.transformed(transform, Tolerance::DEFAULT).unwrap();
        b = b.transformed(transform, Tolerance::DEFAULT).unwrap();
    }
    (a, b)
}

fn boundary(brep: &Brep) -> Boundary {
    brep.faces
        .iter()
        .map(|face| {
            face.loops
                .iter()
                .map(|l| {
                    l.trims
                        .iter()
                        .map(|t| brep.vertices[t.vertices[0]].point.to_array())
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn cross_f(a: [Real; 3], b: [Real; 3]) -> [Real; 3] {
    std::array::from_fn(|i| a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3])
}

fn sub_f(a: [Real; 3], b: [Real; 3]) -> [Real; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot_f(a: [Real; 3], b: [Real; 3]) -> Real {
    (0..3).map(|i| a[i] * b[i]).sum()
}

/// A finite boundary witness, including concave native loops and inner holes.
/// This intentionally does not assert equal face/edge counts or parameterization.
fn on_boundary(p: [Real; 3], boundary: &Boundary) -> bool {
    boundary.iter().any(|face| {
        let ring = &face[0];
        let Some(normal) = (1..ring.len() - 1)
            .map(|i| cross_f(sub_f(ring[i], ring[0]), sub_f(ring[i + 1], ring[0])))
            .find(|n| dot_f(*n, *n) > 1e-20)
        else {
            return false;
        };
        if dot_f(normal, sub_f(p, ring[0])).abs() > 1e-7 * dot_f(normal, normal).sqrt() {
            return false;
        }
        let axis = (0..3)
            .max_by(|&i, &j| normal[i].abs().total_cmp(&normal[j].abs()))
            .unwrap();
        let a = (axis + 1) % 3;
        let b = (axis + 2) % 3;
        let mut inside = false;
        for ring in face {
            for i in 0..ring.len() {
                let start = ring[i];
                let end = ring[(i + 1) % ring.len()];
                let d = sub_f(end, start);
                let t = (dot_f(sub_f(p, start), d) / dot_f(d, d)).clamp(0., 1.);
                let q = std::array::from_fn(|j| start[j] + t * d[j]);
                if dot_f(sub_f(p, q), sub_f(p, q)) <= 1e-14 {
                    return true;
                }
                if (start[b] > p[b]) != (end[b] > p[b])
                    && p[a]
                        < start[a] + (p[b] - start[b]) * (end[a] - start[a]) / (end[b] - start[b])
                {
                    inside = !inside;
                }
            }
        }
        inside
    })
}

#[test]
fn replays_all_26_nonempty_sdk_results_and_keeps_six_semantic_differences_visible() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../../tools/rhino_oracle/fixtures/convex_boolean.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../../../tools/rhino_oracle/observations/convex_boolean.json"
    ))
    .unwrap();
    let mut matched = 0;
    let mut no_result_with_nonempty_set = Vec::new();
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let case = op["case"].as_str().unwrap();
        let name = op["operation"].as_str().unwrap();
        let operation = match name {
            "union" => BrepBooleanOperation::Union,
            "intersection" => BrepBooleanOperation::Intersection,
            "difference" => BrepBooleanOperation::Difference,
            _ => unreachable!(),
        };
        let (a, b) = source(case);
        let result = a
            .try_boolean_convex(&b, operation, Tolerance::DEFAULT)
            .unwrap_or_else(|e| panic!("{case} {name}: {e}"));
        let outputs = row["value"]["outputs"].as_array().unwrap();
        if outputs.is_empty() {
            if result.is_some() {
                no_result_with_nonempty_set.push(format!("{case}_{name}"));
            }
            continue;
        }
        let brep = result.unwrap();
        let volume: Real = outputs.iter().map(|g| g["volume"].as_f64().unwrap()).sum();
        let area: Real = outputs.iter().map(|g| g["area"].as_f64().unwrap()).sum();
        let mass = brep.volume_mass_properties(Tolerance::DEFAULT).unwrap();
        assert!(
            (mass.signed_volume().unwrap() - volume).abs() < 1e-10,
            "{case} {name} volume"
        );
        assert!(
            (brep.area(Tolerance::DEFAULT).unwrap() - area).abs() < 1e-9,
            "{case} {name} area"
        );
        let expected: [Real; 3] = std::array::from_fn(|i| {
            outputs
                .iter()
                .map(|g| g["volume"].as_f64().unwrap() * g["centroid"][i].as_f64().unwrap())
                .sum::<Real>()
                / volume
        });
        assert!(
            mass.centroid()
                .unwrap()
                .distance_to(Point3::try_from(expected).unwrap())
                .unwrap()
                < 1e-10,
            "{case} {name} centroid"
        );
        let tight = BoundingBox3::from_points(brep.vertices.iter().map(|v| v.point)).unwrap();
        for (side, actual) in [tight.min(), tight.max()].into_iter().enumerate() {
            for (i, value) in actual.to_array().into_iter().enumerate() {
                let expected = outputs
                    .iter()
                    .map(|g| g["bounds"][side][i].as_f64().unwrap())
                    .reduce(|a, b| if side == 0 { a.min(b) } else { a.max(b) })
                    .unwrap();
                assert!((value - expected).abs() < 1e-10, "{case} {name} bound");
            }
        }
        let own_boundary = boundary(&brep);
        let native_boundary: Boundary = outputs
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
                assert!(
                    on_boundary(xyz(p), &own_boundary),
                    "{case} {name} native boundary {p}"
                );
            }
        }
        for face in &own_boundary {
            let ring = &face[0];
            let center = std::array::from_fn(|i| {
                ring.iter().map(|p| p[i]).sum::<Real>() / ring.len() as Real
            });
            assert!(
                on_boundary(center, &native_boundary),
                "{case} {name} face interior"
            );
            for i in 0..ring.len() {
                let midpoint =
                    std::array::from_fn(|j| (ring[i][j] + ring[(i + 1) % ring.len()][j]) * 0.5);
                assert!(
                    on_boundary(ring[i], &native_boundary),
                    "{case} {name} kernel vertex"
                );
                assert!(
                    on_boundary(midpoint, &native_boundary),
                    "{case} {name} kernel edge midpoint"
                );
            }
        }
        matched += 1;
    }
    assert_eq!(matched, 26);
    assert_eq!(
        no_result_with_nonempty_set,
        [
            "disjoint_difference",
            "contained_intersection",
            "contained_difference",
            "contains_intersection",
            "equal_union",
            "equal_intersection"
        ]
    );
}
