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
    let spans = [surface.domain_u(), surface.domain_v()].map(|d| d.end() - d.start());
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
    // All coefficients must lie within 1e-12 normalized UV of a single rectangle side.
    // Retain complete unsnapped p-curves in the raw protocol comparison.
    for trim in trims {
        let cps = trim["definition"]["control_points"].as_array().unwrap();
        assert!((0..2).any(|axis| bounds[axis].iter().any(|end| {
            cps.iter()
                .all(|cp| (cp["point"][axis].as_f64().unwrap() - end).abs() / spans[axis] <= 1e-12)
        })));
    }
    for (axis, domain) in [surface.domain_u(), surface.domain_v()].iter().enumerate() {
        for (end, value) in [*domain.start(), *domain.end()].into_iter().enumerate() {
            if (end == 0 && bounds[axis][end] < value) || (end == 1 && bounds[axis][end] > value) {
                assert!((bounds[axis][end] - value).abs() / spans[axis] <= 1e-12);
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
    check_fixture("surface_fillet_parents");
}
#[test]
fn sdk_parent_domains_and_canonical_regions_match_under_affine_parameter_changes() {
    check_fixture("surface_fillet_charts");
}
fn check_fixture(name: &str) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: ProbeRequest = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures").join(format!("{name}.json"))).unwrap(),
    )
    .unwrap();
    let reference: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations").join(format!("{name}.json"))).unwrap(),
    )
    .unwrap();
    let canonical_request: ProbeRequest = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/surface_fillet_parents.json")).unwrap(),
    )
    .unwrap();
    let canonical_reference: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/surface_fillet_parents.json")).unwrap(),
    )
    .unwrap();
    let mut inaccurate_sdk = 0;
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
        // Literal SDK conditioning defects remain explicit negative controls.
        // Canonical inputs have exactly identical normalized NURBS coefficients
        // and the same physical picks; use their accurate geometry, while
        // checking every new parent chart against the unmodified fresh SDK.
        let primary = e;
        let curved = fixture.surfaces[1].degree_u > 1 || fixture.surfaces[1].degree_v > 1;
        let unreliable = name == "surface_fillet_charts"
            && curved
            && conditioning::cylinder_radius_error(primary, fixture) > tolerance.absolute();
        if name == "surface_fillet_charts" {
            let id = op.id().rsplit_once('-').unwrap().0;
            let canonical = canonical_request
                .operations
                .iter()
                .find(|o| o.id() == id)
                .unwrap();
            let Operation::SurfaceFillet {
                fixture: canonical, ..
            } = canonical
            else {
                panic!()
            };
            conditioning::same_input_geometry(fixture, canonical, tolerance);
            if unreliable {
                inaccurate_sdk += 1;
                assert!(conditioning::component_tolerance(primary) > tolerance.absolute());
                assert!(
                    conditioning::cylinder_radius_error(primary, fixture) > tolerance.absolute()
                );
                assert!(conditioning::cylinder_radius_error(a, fixture) <= tolerance.absolute());
            }
        }
        let e = if unreliable {
            let id = op.id().rsplit_once('-').unwrap().0;
            &canonical_reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap()["value"]
        } else {
            primary
        };
        assert!(conditioning::component_tolerance(a) <= tolerance.absolute());
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
            let fresh = primary["parents"][i].as_array().unwrap();
            assert_eq!(a.len(), fresh.len());
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
                for axis in ["domain_u", "domain_v"] {
                    assert_eq!(
                        parent["geometry"]["faces"][0]["definition"][axis],
                        fresh[0]["geometry"]["faces"][0]["definition"][axis],
                        "{} parent {i} {axis}",
                        op.id()
                    );
                }
                check_parent(parent, &e[index], &fixture.queries, tolerance);
            }
        }
    }
    assert_eq!(
        inaccurate_sdk,
        if name == "surface_fillet_charts" {
            8
        } else {
            0
        }
    );
}
