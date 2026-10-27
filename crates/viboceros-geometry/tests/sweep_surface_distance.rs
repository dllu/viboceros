use serde_json::Value;
use viboceros_geometry::{
    CurveRef, LineSegment, NurbsCurve, NurbsSurface, ParameterSide, Point3, Sweep1, SweepBlend,
    SweepFrameStyle, SweepSection, Tolerance, WeightedPoint3,
};
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn certified_distances_classify_the_spatial_sweep_witness() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/sweep1_automatic_curved.json"
    ))
    .unwrap();
    let op = input["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "automatic-spatial-reversed")
        .unwrap();
    let r = &op["rail"];
    let rail = NurbsCurve::try_new_rational(
        3,
        r["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                WeightedPoint3::try_new(point(&v["point"]), v["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(r["knots"].clone()).unwrap(),
    )
    .unwrap()
    .reversed()
    .unwrap();
    let sections = op["sections"]
        .as_array()
        .unwrap()
        .iter()
        .zip([-1., -0.5, 0.])
        .map(|(l, parameter)| SweepSection {
            parameter,
            curve: LineSegment::try_new(point(&l["start"]), point(&l["end"]), Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap(),
        })
        .collect::<Vec<_>>();
    let tol = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let actual = Sweep1::try_new(
        CurveRef::NurbsCurve(&rail),
        &sections,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        tol,
    )
    .unwrap()
    .to_rail_basis_surface()
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/sweep1_automatic_curved.json"
    ))
    .unwrap();
    let row = observed["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == op["id"])
        .unwrap();
    assert_eq!(row["value"].as_array().unwrap().len(), 1);
    let d = &row["value"][0]["definition"];
    let native = NurbsSurface::try_new_rational(
        d["degree"][0].as_u64().unwrap() as usize,
        d["degree"][1].as_u64().unwrap() as usize,
        d["control_count"][0].as_u64().unwrap() as usize,
        d["control_count"][1].as_u64().unwrap() as usize,
        d["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                WeightedPoint3::try_new(point(&v["point"]), v["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(d["knots_u"].clone()).unwrap(),
        serde_json::from_value(d["knots_v"].clone()).unwrap(),
    )
    .unwrap();
    let numeric = Tolerance::try_new(1e-10, 1e-12, 1e-10).unwrap();
    let mut worst = None;
    for (direction, a, b) in [
        ("actual-to-native", &actual, &native),
        ("native-to-actual", &native, &actual),
    ] {
        assert_eq!(a.degree_v(), 1);
        assert_eq!(a.control_point_count_v(), 2);
        let dv = a.domain_v();
        assert_eq!(
            a.knots_v(),
            &[*dv.start(), *dv.start(), *dv.end(), *dv.end()]
        );
        // At a clamped V endpoint the exact tensor image is this stored row.
        // Copy its original controls and weights without rounded isocurve extraction.
        let curve = NurbsCurve::try_new_rational(
            a.degree_u(),
            (0..a.control_point_count_u())
                .map(|i| a.control_point(i, 1).unwrap())
                .collect(),
            a.knots_u().to_vec(),
        )
        .unwrap();
        for i in 0..=64 {
            let u = curve.parameter_at(i as f64 / 64.).unwrap();
            let p = curve.evaluate(u).unwrap();
            let (bu, bv) = b.closest_parameters(p, numeric).unwrap();
            let distance = p.distance_to(b.evaluate(bu, bv).unwrap()).unwrap();
            if worst
                .as_ref()
                .is_none_or(|(old, _, _, _, _, _)| distance > *old)
            {
                worst = Some((distance, direction, u, p, [bu, bv], curve.clone()));
            }
        }
    }
    let (distance, direction, u, p, seed, curve) = worst.unwrap();
    println!("worst {direction} u={u} distance={distance} point={p:?} seed={seed:?}");
    assert!(
        curve
            .positions_within_distance(&[(u, ParameterSide::Right, p)], 1e-12)
            .unwrap()
    );
    let other = if direction == "actual-to-native" {
        &native
    } else {
        &actual
    };
    let certified = other
        .minimum_distance_to_point_seeded(p, Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap(), seed)
        .unwrap();
    println!("surface certificate {certified:?}");
    let certified = certified.expect("surface distance should certify");
    assert!(certified.lower_distance_bound() >= 0.);
    assert!(certified.lower_distance_bound() <= certified.upper_distance);
    assert_eq!(certified.maximum_error, 1e-9);
    let lower = (certified.lower_distance_bound() - 1e-12)
        .max(0.)
        .next_down()
        .max(0.);
    let classification = if lower > 1e-6 {
        "proved_gap"
    } else if certified.upper_distance + 1e-12 <= 1e-6 {
        "witness_within_epsilon"
    } else {
        "inconclusive"
    };
    // Record current compatibility without requiring a known bug to persist.
    println!(
        "SWEEP_DISTANCE {}",
        serde_json::json!({
            "classification": classification, "direction": direction,
            "source_parameter": u, "source_point": p.to_array(),
            "source_membership_error": 1e-12,
            "target_parameters": certified.parameters,
            "upper_distance": certified.upper_distance,
            "maximum_error": certified.maximum_error,
            "image_distance_lower_bound": lower,
            "construction_tolerance": 1e-7,
            "comparison_epsilon": 1e-6,
            "complete_surface_agreement_qualified": false
        })
    );
}
