//! Explicit negative SDK controls, kept separate from geometric qualification.
use super::*;

pub(super) fn component_tolerance(value: &Value) -> f64 {
    let mut largest = 0_f64;
    for body in value["outputs"].as_array().unwrap().iter().chain(
        value["parents"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g.as_array().unwrap()),
    ) {
        for table in ["vertices", "edges"] {
            for part in body["geometry"][table].as_array().unwrap() {
                largest = largest.max(part["tolerance"].as_f64().unwrap());
            }
        }
    }
    largest
}

pub(super) fn same_input_geometry(actual: &Fixture, canonical: &Fixture, tolerance: Tolerance) {
    for i in 0..2 {
        let a = nurbs_surface_from_definition(&actual.surfaces[i]).unwrap();
        let e = nurbs_surface_from_definition(&canonical.surfaces[i]).unwrap();
        assert_eq!(
            a.try_reparameterized(0. ..=1., 0. ..=1.).unwrap(),
            e.try_reparameterized(0. ..=1., 0. ..=1.).unwrap()
        );
        assert!(
            a.evaluate(actual.picks[i][0], actual.picks[i][1])
                .unwrap()
                .distance_to(
                    e.evaluate(canonical.picks[i][0], canonical.picks[i][1])
                        .unwrap()
                )
                .unwrap()
                <= tolerance.absolute()
        );
    }
    assert_eq!(actual.radius, canonical.radius);
    assert_eq!(actual.extend, canonical.extend);
    assert_eq!(actual.trim, canonical.trim);
}

pub(super) fn cylinder_radius_error(value: &Value, fixture: &Fixture) -> f64 {
    let inputs = fixture
        .surfaces
        .each_ref()
        .map(|d| nurbs_surface_from_definition(d).unwrap());
    // These particular recipes use a radius-five cylinder and z=0 plane.
    // Verify the analytic witness's input assumptions rather than generalizing
    // it to arbitrary surfaces or treating an SDK IsValid flag as accuracy.
    for (s, input) in inputs.iter().enumerate() {
        for i in 0..=2 {
            for j in 0..=2 {
                let p = input
                    .evaluate(
                        input.parameter_at_u(i as f64 / 2.).unwrap(),
                        input.parameter_at_v(j as f64 / 2.).unwrap(),
                    )
                    .unwrap();
                if s == 0 {
                    assert!(p.z().abs() < 1e-12);
                } else {
                    assert!((p.x().hypot(p.y()) - 5.).abs() < 1e-12);
                }
            }
        }
    }
    let first = inputs[0]
        .evaluate(fixture.picks[0][0], fixture.picks[0][1])
        .unwrap();
    let second = inputs[1]
        .evaluate(fixture.picks[1][0], fixture.picks[1][1])
        .unwrap();
    let r = fixture.radius;
    // A pick opposite the bounded quarter cylinder can have radius > 5
    // while still choosing its inward normal. Use its bounded projection,
    // not an imaginary full-cylinder radial classification.
    let (u, v) = inputs[1]
        .closest_parameters(first, Tolerance::DEFAULT)
        .unwrap();
    let q = inputs[1].evaluate(u, v).unwrap();
    let outward = (first.x() - q.x()) * q.x() + (first.y() - q.y()) * q.y();
    assert!(outward.abs() > 1e-8);
    let major = if outward > 0. { 5. + r } else { 5. - r };
    let z = if second.z() > 0. { r } else { -r };
    value["outputs"][0]["geometry"]["faces"][0]["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let p: [f64; 3] = serde_json::from_value(p.clone()).unwrap();
            ((p[0].hypot(p[1]) - major).hypot(p[2] - z) - r).abs()
        })
        .fold(0_f64, f64::max)
}

#[test]
fn sdk_anisotropic_fits_expose_eight_accuracy_failures_instead_of_passing_as_valid_geometry() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: ProbeRequest = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/surface_fillet_charts.json")).unwrap(),
    )
    .unwrap();
    let response: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/surface_fillet_charts.json")).unwrap(),
    )
    .unwrap();
    let tolerance = request.tolerance.geometry().unwrap();
    let mut failures = 0;
    for (op, row) in request
        .operations
        .iter()
        .zip(response["results"].as_array().unwrap())
    {
        assert_eq!(op.id(), row["id"].as_str().unwrap());
        let Operation::SurfaceFillet { fixture, .. } = op else {
            panic!()
        };
        let value = &row["value"];
        let curved = fixture.surfaces[1].degree_u > 1 || fixture.surfaces[1].degree_v > 1;
        let inaccurate = curved && cylinder_radius_error(value, fixture) > tolerance.absolute();
        if inaccurate {
            // Component tolerances can be conservative even on accurate
            // fits; require an independent spatial witness before declaring
            // an SDK accuracy failure.
            assert!(component_tolerance(value) > tolerance.absolute());
            assert!(op.id().ends_with("anisotropic"));
            assert_eq!(
                value["outputs"][0]["geometry"]["faces"][0]["definition"]["control_count"],
                json!([7, 3])
            );
            failures += 1;
        }
    }
    assert_eq!(failures, 8);
}
