//! Independent closest-point expectations; Rhino discrepancies are explicit.
use super::*;

#[test]
fn robust_curve_fixture_preserves_analytic_minima_and_records_rhino_disagreement() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/nurbs_curve_closest_robust.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../docs/curve-closest-rhino-reference.json"
    ))
    .unwrap();
    let radius = 2_f64.powi(54);
    let expected = [
        ("distant-line-projection", 0.37, [0.37, 0., 0.], 1e100),
        (
            "distant-wide-domain-projection",
            960000000000.,
            [0.37, 0., 0.],
            1e100,
        ),
        (
            "large-speed-line-projection",
            0.37,
            [0.37 * 1e160, 0., 0.],
            1.,
        ),
        (
            "signed-pole-distant-projection",
            0.25,
            [-0.5, 0., 0.],
            1e100,
        ),
        (
            "translated-semicircle-offset",
            1.,
            [radius, 0., 0.],
            radius.hypot(radius),
        ),
    ];
    assert_eq!(response.results.len(), expected.len());
    for (actual, (id, parameter, point, distance)) in response.results.iter().zip(expected) {
        assert_eq!(actual.id, id);
        // Formula-derived expectations, not copies of the native response.
        assert_eq!(
            actual.value,
            json!({"parameter": parameter, "point": point, "distance": distance})
        );
    }
    let rhino = reference["results"].as_array().unwrap();
    assert_eq!(rhino.len(), 3);
    for (actual, reference) in response.results.iter().zip(rhino).take(2) {
        assert_eq!(actual.id, reference["id"].as_str().unwrap());
        assert_eq!(actual.value, reference["value"]);
    }
    // Rhino rejected the mixed-sign curve and returned an invalid, out-of-domain
    // parameter for the large-speed line. Neither is a usable reference result.
    assert_eq!(rhino[2]["id"], "translated-semicircle-offset");
    assert_eq!(rhino[2]["value"]["parameter"], json!(0.));
    assert_eq!(rhino[2]["value"]["point"], json!([-radius, 0., 0.]));
    // The distances round alike, but the original target strictly prefers the
    // right endpoint. Never hide that geometric disagreement in a wide epsilon.
    assert_eq!(
        rhino[2]["value"]["distance"],
        response.results[4].value["distance"]
    );
    assert_ne!(
        rhino[2]["value"]["point"],
        response.results[4].value["point"]
    );
}
