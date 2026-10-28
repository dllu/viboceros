//! Captured Global/Local sweeps, with complete bounds or certified gap witnesses.
use serde_json::Value;
use viboceros_geometry::{
    CurveRef, LineSegment, NurbsCurve, NurbsSurface, ParameterSide, Point3, Sweep1, SweepBlend,
    SweepFrameStyle, SweepSection, Tolerance, WeightedPoint3,
};

fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn line(v: &Value, tolerance: Tolerance) -> LineSegment {
    LineSegment::try_new(point(&v["start"]), point(&v["end"]), tolerance).unwrap()
}
fn native(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn boundary(surface: &NurbsSurface, row: usize) -> NurbsCurve {
    assert_eq!(surface.degree_v(), 1);
    assert_eq!(surface.control_point_count_v(), 2);
    let dv = surface.domain_v();
    assert_eq!(
        surface.knots_v(),
        &[*dv.start(), *dv.start(), *dv.end(), *dv.end()]
    );
    assert!((0..surface.control_point_count_u()).all(|i| {
        surface.control_point(i, 0).unwrap().weight()
            == surface.control_point(i, 1).unwrap().weight()
    }));
    NurbsCurve::try_new_rational(
        surface.degree_u(),
        (0..surface.control_point_count_u())
            .map(|i| surface.control_point(i, row).unwrap())
            .collect(),
        surface.knots_u().to_vec(),
    )
    .unwrap()
}

#[test]
fn classify_complete_global_sweeps_without_requiring_known_gaps_to_persist() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let query_tolerance = Tolerance::try_new(1e-10, 1e-12, 1e-10).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in ["sweep1_global_shapes", "sweep1_global_holdouts"] {
        let input: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let observed: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let rail = line(&op["rail"], tolerance);
            let sections = op["sections"]
                .as_array()
                .unwrap()
                .iter()
                .zip(op["parameters"].as_array().unwrap())
                .map(|(v, t)| SweepSection {
                    parameter: t.as_f64().unwrap(),
                    curve: line(v, tolerance).to_nurbs().unwrap(),
                })
                .collect::<Vec<_>>();
            let model = Sweep1::try_new(
                CurveRef::Line(&rail),
                &sections,
                SweepFrameStyle::Freeform,
                if op["blend"] == 0 {
                    SweepBlend::Local
                } else {
                    SweepBlend::Global
                },
                tolerance,
            )
            .unwrap();
            let actual = if op["command"] == true && op["refit_rail"] != true {
                model.to_rail_basis_surface().unwrap()
            } else {
                model.to_surface().unwrap()
            };
            assert_eq!(row["value"].as_array().unwrap().len(), 1);
            let expected = native(&row["value"][0]["definition"]);
            let a = [boundary(&actual, 0), boundary(&actual, 1)];
            let b = [boundary(&expected, 0), boundary(&expected, 1)];
            let bounds = (0..2)
                .map(|i| a[i].parameter_curve_deviation_bound(&b[i], 1e-6).unwrap())
                .collect::<Vec<_>>();
            let bound = bounds
                .iter()
                .copied()
                .try_fold(0_f64, |a, b| b.map(|b| a.max(b)));
            if op["blend"] == 0 || sections.len() == 2 {
                assert!(bound.is_some(), "passing control {}: {bounds:?}", op["id"]);
            }
            let mut worst = None;
            for (direction, curves, other) in [
                ("actual-to-native", &a, &expected),
                ("native-to-actual", &b, &actual),
            ] {
                for (edge, curve) in curves.iter().enumerate() {
                    for i in 0..=16 {
                        let t = curve.parameter_at(i as f64 / 16.).unwrap();
                        let p = curve.evaluate(t).unwrap();
                        let (u, v) = other.closest_parameters(p, query_tolerance).unwrap();
                        let distance = p.distance_to(other.evaluate(u, v).unwrap()).unwrap();
                        if worst
                            .as_ref()
                            .is_none_or(|(old, _, _, _, _, _)| distance > *old)
                        {
                            worst = Some((distance, direction, edge, t, p, [u, v]));
                        }
                    }
                }
            }
            let (_, direction, edge, t, p, uv) = worst.unwrap();
            let (curve, other) = if direction == "actual-to-native" {
                (&a[edge], &expected)
            } else {
                (&b[edge], &actual)
            };
            assert!(
                curve
                    .positions_within_distance(&[(t, ParameterSide::Right, p)], 1e-12)
                    .unwrap()
            );
            let distance = other
                .minimum_distance_to_point_seeded(
                    p,
                    Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap(),
                    uv,
                )
                .unwrap()
                .expect("small captured surface should certify");
            let lower = (distance.lower_distance_bound() - 1e-12)
                .max(0.)
                .next_down()
                .max(0.);
            let classification = if bound.is_some() {
                "complete_images_within_epsilon"
            } else if lower > 1e-6 {
                "proved_gap"
            } else {
                "inconclusive"
            };
            println!(
                "GLOBAL_BLEND {}",
                serde_json::json!({
                    "id":op["id"],"family":family,"classification":classification,
                    "complete_image_bound":bound,"direction":direction,"edge":edge,
                    "source_parameter":t,"source_point":p.to_array(),"source_membership_error":1e-12,
                    "target_parameters":distance.parameters,"upper_distance":distance.upper_distance,
                    "maximum_error":distance.maximum_error,"image_distance_lower_bound":lower,
                    "comparison_epsilon":1e-6,"construction_tolerance":1e-7
                })
            );
            count += 1;
        }
    }
    assert_eq!(count, 18);
}
