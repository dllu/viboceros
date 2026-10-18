//! SDK parent regions for these rectangular planar/cylindrical recipes.
use super::*;

fn rect(record: &Value, tolerance: Tolerance) -> Brep {
    let faces = record["geometry"]["faces"].as_array().unwrap();
    assert_eq!(faces.len(), 1);
    let face = &faces[0];
    let surface = images::surface(face);
    let loops = face["loops"].as_array().unwrap();
    assert_eq!(loops.len(), 1);
    let trims = loops[0].as_array().unwrap();
    assert_eq!(trims.len(), 4);
    let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for trim in trims {
        for cp in trim["definition"]["control_points"].as_array().unwrap() {
            for (axis, b) in bounds.iter_mut().enumerate() {
                let x = cp["point"][axis].as_f64().unwrap();
                b[0] = b[0].min(x);
                b[1] = b[1].max(x);
            }
        }
    }
    // This helper qualifies rectangular fixtures, not arbitrary SDK trim loops.
    // All coefficients must lie within 1e-12 UV of a single rectangle side.
    // Retain complete unsnapped p-curves in the raw protocol comparison.
    for trim in trims {
        let cps = trim["definition"]["control_points"].as_array().unwrap();
        assert!((0..2).any(|axis| bounds[axis].iter().any(|end| {
            cps.iter()
                .all(|cp| (cp["point"][axis].as_f64().unwrap() - end).abs() <= 1e-12)
        })));
    }
    for (axis, domain) in [surface.domain_u(), surface.domain_v()].iter().enumerate() {
        for (end, value) in [*domain.start(), *domain.end()].into_iter().enumerate() {
            if (end == 0 && bounds[axis][end] < value) || (end == 1 && bounds[axis][end] > value) {
                assert!((bounds[axis][end] - value).abs() <= 1e-12);
                bounds[axis][end] = value;
            }
        }
    }
    Brep::try_rectangular_surface_face(
        surface,
        bounds[0][0]..=bounds[0][1],
        bounds[1][0]..=bounds[1][1],
        tolerance,
    )
    .unwrap()
}
fn membership(target: &Brep, p: Point3, tolerance: Tolerance) {
    let q = closest_point(target, p, tolerance).unwrap();
    assert!(
        q.distance_to(p).unwrap() <= tolerance.absolute(),
        "parent image: {p:?} vs {q:?}"
    );
}
fn check_parent(a: &Value, e: &Value, queries: &[[f64; 3]], tolerance: Tolerance) {
    assert_eq!(a["valid"], true);
    assert_eq!(e["valid"], true);
    let actual = rect(a, tolerance);
    let expected = rect(e, tolerance);
    for (source, target, record) in [(&actual, &expected, a), (&expected, &actual, e)] {
        let f = &source.faces()[0];
        let b = f.rectangular_trim_bounds(tolerance).unwrap().unwrap();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = b[0][0] + (b[0][1] - b[0][0]) * i as f64 / 8.;
                let v = b[1][0] + (b[1][1] - b[1][0]) * j as f64 / 8.;
                membership(target, f.surface().evaluate(u, v).unwrap(), tolerance);
            }
        }
        for boundary in record["geometry"]["faces"][0]["loops"][0]
            .as_array()
            .unwrap()
        {
            for p in boundary["lifted"].as_array().unwrap() {
                membership(
                    target,
                    point(serde_json::from_value(p.clone()).unwrap()).unwrap(),
                    tolerance,
                );
            }
        }
    }
    for ((p, q), query) in a["closest_points"]
        .as_array()
        .unwrap()
        .iter()
        .zip(e["closest_points"].as_array().unwrap())
        .zip(queries)
    {
        let p = point(serde_json::from_value(p.clone()).unwrap()).unwrap();
        let q = point(serde_json::from_value(q.clone()).unwrap()).unwrap();
        let query = point(*query).unwrap();
        membership(&expected, p, tolerance);
        membership(&actual, q, tolerance);
        assert!(
            (query.distance_to(p).unwrap() - query.distance_to(q).unwrap()).abs()
                <= tolerance.absolute()
        );
    }
}
#[test]
fn sdk_parent_trim_and_split_regions_match_without_mutating_sources() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_fillet_parents.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rhino_oracle/observations/surface_fillet_parents.json"
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(request.operations.len(), 32);
    let tolerance = request.tolerance.geometry().unwrap();
    for op in &request.operations {
        let Operation::SurfaceFillet { fixture, .. } = op else {
            panic!("wrong operation")
        };
        assert!(fixture.trim.is_some());
        assert!(fixture.sides.is_none());
        let mut one = request.clone();
        one.operations = vec![op.clone()];
        let response = run_request(&one).unwrap_or_else(|e| panic!("{}: {e}", op.id()));
        let a = &response.results[0].value;
        let e = &reference["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == op.id())
            .unwrap()["value"];
        assert_eq!(a["source_unchanged"], true);
        assert_eq!(e["source_unchanged"], true);
        assert_eq!(a["outputs"].as_array().unwrap().len(), 1);
        assert_eq!(e["outputs"].as_array().unwrap().len(), 1);
        images::assert_agreement(
            &a["outputs"][0],
            &e["outputs"][0],
            &fixture.queries,
            tolerance,
        );
        for (p, q) in a["outputs"][0]["bounds"]
            .as_array()
            .unwrap()
            .iter()
            .zip(e["outputs"][0]["bounds"].as_array().unwrap())
        {
            assert!((0..3).all(
                |axis| (p[axis].as_f64().unwrap() - q[axis].as_f64().unwrap()).abs()
                    <= tolerance.absolute()
            ));
        }
        for i in 0..2 {
            let a = a["parents"][i].as_array().unwrap();
            let e = e["parents"][i].as_array().unwrap();
            assert_eq!(a.len(), e.len(), "{} parent {i}", op.id());
            let mut used = vec![false; e.len()];
            for parent in a {
                let index = e
                    .iter()
                    .enumerate()
                    .position(|(j, r)| {
                        !used[j]
                            && parent["bounds"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .zip(r["bounds"].as_array().unwrap())
                                .all(|(p, q)| {
                                    (0..3).all(|axis| {
                                        (p[axis].as_f64().unwrap() - q[axis].as_f64().unwrap())
                                            .abs()
                                            <= 1e-8
                                    })
                                })
                    })
                    .unwrap_or_else(|| panic!("{} parent {i} bounds", op.id()));
                used[index] = true;
                check_parent(parent, &e[index], &fixture.queries, tolerance);
            }
        }
    }
}
