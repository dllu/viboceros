#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    LoftStyle, NurbsCurve, NurbsSurface, WeightedPoint3, try_loft_nurbs_curves,
};
use viboceros_smlib::{loft_surface, prepare_loft_sections};

fn profiles(op: &Value) -> Vec<NurbsCurve> {
    op["curves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let curve = NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                support::points(&c["control_points"]),
                support::knots(&c["knots"]),
            )
            .unwrap();
            if let Some(d) = c.get("domain") {
                curve
                    .try_reparameterized(d[0].as_f64().unwrap()..=d[1].as_f64().unwrap())
                    .unwrap()
            } else {
                curve
            }
        })
        .collect()
}
fn compare(a: &NurbsSurface, e: &NurbsSurface, id: &str) {
    support::compare(&support::surface_value(a), &support::surface_value(e), id);
    for i in 0..=16 {
        for j in 0..=16 {
            let a = a
                .evaluate(
                    a.parameter_at_u(i as f64 / 16.).unwrap(),
                    a.parameter_at_v(j as f64 / 16.).unwrap(),
                )
                .unwrap();
            let e = e
                .evaluate(
                    e.parameter_at_u(i as f64 / 16.).unwrap(),
                    e.parameter_at_v(j as f64 / 16.).unwrap(),
                )
                .unwrap();
            for (a, e) in a.to_array().into_iter().zip(e.to_array()) {
                assert!(
                    (a - e).abs() <= 1e-9 + 1e-12 * a.abs().max(e.abs()),
                    "{id}: {a} != {e}"
                );
            }
        }
    }
}

#[test]
fn native_common_basis_lofts_preserve_all_styles_closed_modes_and_reference_loci() {
    let fixtures = [
        include_str!("../../../tools/rhino_oracle/fixtures/loft.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/loft_command.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/loft_end_weights.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/loft_end_weights_diagnostics.json"),
    ];
    let mut cases = 0;
    for fixture in fixtures {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        for op in fixture["operations"].as_array().unwrap() {
            let source = profiles(op);
            let before = source.clone();
            let style = match op["style"].as_str().unwrap_or("normal") {
                "normal" => LoftStyle::Normal,
                "loose" => LoftStyle::Loose,
                "tight" => LoftStyle::Tight,
                "straight" => LoftStyle::Straight,
                "uniform" => LoftStyle::Uniform,
                _ => unreachable!(),
            };
            let closed = op["closed"].as_bool().unwrap_or(false);
            let actual = loft_surface(&source, style, closed).unwrap();
            let expected = try_loft_nurbs_curves(&source, style, closed).unwrap();
            compare(&actual, &expected, op["id"].as_str().unwrap());
            assert_eq!(source, before);
            cases += 1;
        }
    }
    assert_eq!(cases, 72);
}

#[test]
fn common_weight_gauges_and_binary_profile_charts_leave_loft_geometry_unchanged() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/loft.json"
    ))
    .unwrap();
    let source = profiles(&fixture["operations"][0]);
    let expected = loft_surface(&source, LoftStyle::Normal, false).unwrap();
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let curves = source
                .iter()
                .map(|c| {
                    NurbsCurve::try_new_rational(
                        c.degree(),
                        c.control_points()
                            .iter()
                            .map(|p| {
                                WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap()
                            })
                            .collect(),
                        c.knots().iter().map(|k| k * chart).collect(),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            compare(
                &loft_surface(&curves, LoftStyle::Normal, false).unwrap(),
                &expected,
                "gauge/chart",
            );
        }
    }
}

#[test]
fn native_interpolation_preserves_tiny_channels_across_open_and_closed_systems() {
    use viboceros_geometry::Point3;
    for tiny in [1e-18, 1e-180, 1e-280] {
        let curves = [(0., 0.), (0.5, 1.), (1.7, -2.), (4., 3.)].map(|(x, z)| {
            NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(x, 0., z * tiny).unwrap(),
                    Point3::try_new(x + 1., 1., z * tiny).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap()
        });
        for closed in [false, true] {
            let actual = loft_surface(&curves, LoftStyle::Normal, closed).unwrap();
            let expected = try_loft_nurbs_curves(&curves, LoftStyle::Normal, closed).unwrap();
            for (a, e) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                assert!((a.point().z() / tiny - e.point().z() / tiny).abs() < 1e-10);
            }
        }
    }
}

#[test]
fn native_preparation_creates_one_basis_without_mutating_original_profiles() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/loft.json"
    ))
    .unwrap();
    for op in fixture["operations"].as_array().unwrap() {
        let source = profiles(op);
        let before = source.clone();
        let prepared = prepare_loft_sections(&source).unwrap();
        for c in &prepared {
            assert_eq!(c.degree(), prepared[0].degree());
            assert_eq!(c.knots(), prepared[0].knots());
            assert_eq!(c.domain(), 0.0..=1.0);
        }
        assert_eq!(source, before);
    }
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
