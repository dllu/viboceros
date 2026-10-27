use super::*;
use serde_json::{Value, json};

const FAMILIES: &[&str] = &[
    "loft_refit_coarse",
    "loft_refit_candidates",
    "loft_refit_candidate_boundary",
    "loft_refit_adjustment",
    "loft_refit_adjustment_fit_clock",
    "loft_refit_adjustment_ellipses",
    "loft_refit_adjustment_local",
    "loft_refit_adjustment_commands",
    "loft_refit",
    "loft_refit_polylines",
    "loft_refit_polyline_angles",
    "loft_refit_polyline_ends",
    "loft_refit_lines",
    "loft_refit_multispan",
    "loft_refit_initial_stations",
    "loft_refit_initial_holdouts",
    "loft_refit_adaptive_curved",
    "loft_refit_curved_clocks",
    "loft_refit_join_scales",
    "loft_refit_join_curvature",
    "loft_refit_unequal_sampling",
    "loft_refit_stage_reserves",
    "loft_refit_source_continuity",
    "loft_refit_adaptive_shapes",
    "loft_refit_join_modes",
    "loft_refit_adaptive_contexts",
    "loft_refit_adaptive_admission",
    "loft_refit_arc_clocks",
    "loft_refit_angles",
    "loft_refit_angle_holdouts",
    "loft_refit_shallow_angles",
    "loft_refit_shallow_holdouts",
    "loft_refit_corner_boundary",
    "loft_refit_corner_suppression",
    "loft_refit_suppressed_initial",
    "loft_refit_matched_probe_angles",
    "loft_refit_subdivision",
];
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle")
}
fn curve(def: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        def["degree"].as_u64().unwrap() as usize,
        def["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    crate::Point3::try_from(
                        serde_json::from_value::<[Real; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(def["knots"].clone()).unwrap(),
    )
    .unwrap()
}
fn baseline() -> Value {
    let mut cases = Vec::new();
    for family in FAMILIES {
        let query: Value = serde_json::from_slice(
            &std::fs::read(root().join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        for op in query["operations"].as_array().unwrap() {
            if op["op"] != "loft" {
                assert_eq!(op["op"], "nurbs_curve_closest_point");
                continue;
            }
            let sources = op["curves"]
                .as_array()
                .unwrap()
                .iter()
                .map(curve)
                .collect::<Vec<_>>();
            let before = sources.clone();
            let accepted = initial(
                &sources,
                op["refit_tolerance"].as_f64().unwrap(),
                op["refit_angle_tolerance"].as_f64(),
            )
            .is_ok();
            assert_eq!(sources, before);
            cases.push(json!({"family":family,"id":op["id"],"initial_preparation":accepted}));
        }
    }
    json!({"source_base_commit":"85d130537e64272f21ab4f4b023a5164c4477991","cases":cases})
}
#[test]
#[ignore]
fn export_legacy_preparation_baseline() {
    let baseline = baseline();
    std::fs::write(
        "/tmp/viboceros-refit-preparation-baseline.json",
        serde_json::to_vec_pretty(&baseline).unwrap(),
    )
    .unwrap();
}
#[test]
fn existing_preparation_admission_retains_its_frozen_case_set() {
    let saved: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures/loft_refit_preparation_baseline.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(baseline(), saved);
}

#[test]
fn adaptive_shared_basis_edits_require_a_fresh_original_source_certificate() {
    let sources = [0., 2.].map(|z| {
        NurbsCurve::try_new_rational(
            2,
            [
                ([2., 0., z], 1.),
                ([2., 2., z], std::f64::consts::FRAC_1_SQRT_2),
                ([0., 2., z], 1.),
            ]
            .map(|(point, weight)| {
                WeightedPoint3::try_new(crate::Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .to_vec(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
    });
    let before = sources.clone();
    let prepared = propose(&sources, 0.0007, None).unwrap();
    assert!(prepared.mode == Mode::Adaptive);
    let mut edited = false;
    let result = prepared.finish(
        &sources,
        0.0007,
        None,
        &mut |c: &NurbsCurve| {
            edited = true;
            NurbsCurve::try_new_rational(
                c.degree(),
                c.control_points()
                    .iter()
                    .map(|p| {
                        let [x, y, z] = p.point().to_array();
                        WeightedPoint3::try_new(crate::Point3::try_new(x, y, z + 0.25)?, p.weight())
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?,
                c.knots().to_vec(),
            )
        },
        &mut |c, d| c.try_change_degree(d, false),
        &mut |c, k, m| c.try_insert_knot(k, m),
    );
    assert!(edited);
    assert!(matches!(
        result,
        Err(GeometryError::UnsupportedCurveApproximation { .. })
    ));
    assert_eq!(sources, before);
    let prepared = propose(&sources, 0.0007, None).unwrap();
    let result = prepared.finish(
        &sources,
        0.0007,
        None,
        &mut |_| Err(GeometryError::InvalidTolerance),
        &mut |c, d| c.try_change_degree(d, false),
        &mut |c, k, m| c.try_insert_knot(k, m),
    );
    assert_eq!(result, Err(GeometryError::InvalidTolerance));
    assert_eq!(sources, before);
}
