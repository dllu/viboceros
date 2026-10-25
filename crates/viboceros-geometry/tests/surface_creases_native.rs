use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{
    Brep, CurveContinuity, NurbsSurface, ParameterSide, Point3, Tolerance, WeightedPoint3,
};
#[allow(dead_code)]
#[path = "support/loft_rebuild.rs"]
mod comparison;

fn surface(d: &Value) -> NurbsSurface {
    let controls = d["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let point: [f64; 3] = serde_json::from_value(c["point"].clone()).unwrap();
            WeightedPoint3::try_new(
                Point3::try_from(point).unwrap(),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    NurbsSurface::try_new_rational(
        d["degree_u"].as_u64().unwrap() as usize,
        d["degree_v"].as_u64().unwrap() as usize,
        d["control_point_count_u"].as_u64().unwrap() as usize,
        d["control_point_count_v"].as_u64().unwrap() as usize,
        controls,
        serde_json::from_value(d["knots_u"].clone()).unwrap(),
        serde_json::from_value(d["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}

fn record(surface: &NurbsSurface) -> Value {
    let samples = (0..=16)
        .flat_map(|i| {
            (0..=16).map(move |j| {
                surface
                    .evaluate(
                        surface.parameter_at_u(i as f64 / 16.).unwrap(),
                        surface.parameter_at_v(j as f64 / 16.).unwrap(),
                    )
                    .unwrap()
                    .to_array()
            })
        })
        .collect::<Vec<_>>();
    json!({"degree":[surface.degree_u(),surface.degree_v()],
        "control_count":[surface.control_point_count_u(),surface.control_point_count_v()],
        "control_points":surface.control_points().iter().map(|c| json!({"point":c.point().to_array(),"weight":c.weight()})).collect::<Vec<_>>(),
        "knots_u":surface.knots_u(),"knots_v":surface.knots_v(),
        "domain_u":[surface.domain_u().start(),surface.domain_u().end()],
        "domain_v":[surface.domain_v().start(),surface.domain_v().end()],"samples":samples})
}

#[test]
fn adaptive_partitions_preserve_native_geometry_and_expose_narrow_crease_witnesses() {
    compare_native(true);
}

#[test]
#[ignore = "Native splitter misses narrow witnessed creases; retain the full raw topology comparison"]
fn every_adaptive_partition_matches_the_complete_native_raw_record() {
    compare_native(false);
}

fn compare_native(allow_extra: bool) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut matched = 0;
    let mut extra = 0;
    let mut maximum = 0_f64;
    let mut maximum_weight = 0_f64;
    let mut failures = Vec::new();
    for family in [
        "surface_creases_smoke",
        "surface_creases",
        "surface_creases_holdouts",
        "surface_creases_witnesses",
    ] {
        let query: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            query["operations"].as_array().unwrap().len(),
            native["results"].as_array().unwrap().len()
        );
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let source = surface(&op["surface"]);
            let before = source.clone();
            let angle = op["inspect_crease"].as_f64().unwrap();
            let raw = source
                .discontinuity_parameters(CurveContinuity::G1, angle)
                .unwrap();
            let brep = match Brep::try_surface_partitioned_at_discontinuities(
                &source,
                &[(CurveContinuity::G1, angle)],
                Tolerance::DEFAULT,
            ) {
                Ok(b) => b,
                Err(e) => {
                    failures.push(format!("{}: {e:?}", op["id"]));
                    continue;
                }
            };
            let mut faces = brep.faces().iter().collect::<Vec<_>>();
            faces.sort_by(|a, b| {
                let a = a.surface();
                let b = b.surface();
                a.domain_v()
                    .start()
                    .total_cmp(b.domain_v().start())
                    .then(a.domain_v().end().total_cmp(b.domain_v().end()))
                    .then(a.domain_u().start().total_cmp(b.domain_u().start()))
                    .then(a.domain_u().end().total_cmp(b.domain_u().end()))
            });
            let mut actual = json!({"initial_faces":1,"succeeded":true,"valid":true,
                "raw_breaks":raw,"surfaces":faces.iter().map(|f| record(f.surface())).collect::<Vec<_>>(),
                "face_reversed":faces.iter().map(|f| f.is_reversed()).collect::<Vec<_>>(),
                "vertices":brep.vertices().len(),"edges":brep.edges().len(),"source_unchanged":source==before});
            assert_eq!(source, before);
            let mut expected = row["value"]["crease"].clone();
            assert_eq!(expected["source_unchanged"], true);
            if allow_extra
                && actual["surfaces"].as_array().unwrap().len() == 2
                && expected["surfaces"].as_array().unwrap().len() == 1
            {
                let (error, weight) = verify_extra(&source, &brep, angle, &expected);
                maximum = maximum.max(error);
                maximum_weight = maximum_weight.max(weight);
                extra += 1;
                continue;
            }
            maximum_weight =
                maximum_weight.max(normalize_weights(&mut actual, &mut expected).unwrap());
            match comparison::difference(&actual, &expected) {
                Ok(error) if error <= 1e-9 => {
                    matched += 1;
                    maximum = maximum.max(error);
                }
                other => failures.push(format!(
                    "{}: {other:?}; faces {} / {}",
                    op["id"],
                    actual["surfaces"].as_array().unwrap().len(),
                    expected["surfaces"].as_array().unwrap().len()
                )),
            }
        }
    }
    println!(
        "SURFACE_CREASE raw_matches={matched} extra_splits={extra} maximum={maximum} weight_relative={maximum_weight} failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(matched + extra, 117);
    if allow_extra {
        assert_eq!((matched, extra), (98, 19));
    }
}

// These compare complete coefficient records while preserving the geometric
// distinction between absolute model units and a common projective gauge.
fn normalize_weights(a: &mut Value, b: &mut Value) -> Result<f64, String> {
    let mut maximum = 0_f64;
    if a["surfaces"].as_array().unwrap().len() != b["surfaces"].as_array().unwrap().len() {
        return Ok(0.);
    }
    for (a, b) in a["surfaces"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(b["surfaces"].as_array_mut().unwrap())
    {
        for (a, b) in a["control_points"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(b["control_points"].as_array_mut().unwrap())
        {
            let (wa, wb) = (a["weight"].as_f64().unwrap(), b["weight"].as_f64().unwrap());
            let relative = (wa - wb).abs() / wa.abs().max(wb.abs());
            if relative > 1e-12 {
                return Err(format!("weight relative difference {relative}"));
            }
            maximum = maximum.max(relative);
            a["weight"] = json!(1.);
            b["weight"] = json!(1.);
        }
    }
    Ok(maximum)
}

fn verify_extra(source: &NurbsSurface, brep: &Brep, angle: f64, expected: &Value) -> (f64, f64) {
    // These independent spike families have a public derivative witness at
    // normalized transverse 1/p, including Rhino's own retained measurements.
    let axis = usize::from(source.degree_u() != 1);
    let (main, transverse, p) = if axis == 0 {
        (source.domain_u(), source.domain_v(), source.degree_v())
    } else {
        (source.domain_v(), source.domain_u(), source.degree_u())
    };
    let t = if axis == 0 {
        source.knots_u()[2]
    } else {
        source.knots_v()[2]
    };
    assert!(p >= 12 && t > *main.start() && t < *main.end());
    let v = *transverse.start() + (*transverse.end() - *transverse.start()) / p as f64;
    let parameters = if axis == 0 { [t, v] } else { [v, t] };
    let mut tangents = Vec::new();
    for side in [ParameterSide::Left, ParameterSide::Right] {
        let sides = if axis == 0 {
            [side, ParameterSide::Right]
        } else {
            [ParameterSide::Right, side]
        };
        let (_, du, dv) = source
            .evaluate_with_derivatives_on_sides(parameters[0], parameters[1], sides[0], sides[1])
            .unwrap();
        tangents.push(
            if axis == 0 { du } else { dv }
                .normalized_nonzero()
                .unwrap()
                .as_vector(),
        );
    }
    let measured = tangents[0].dot(tangents[1]).unwrap().clamp(-1., 1.).acos();
    assert!(
        measured > angle + 1e-8,
        "unwitnessed extra split: {measured} <= {angle}"
    );
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (2, 7, 6)
    );
    assert_eq!(
        brep.edge_use_counts()
            .into_iter()
            .filter(|n| *n == 2)
            .count(),
        1
    );
    let mut a = json!({"surfaces":[record(source)]});
    let mut b = json!({"surfaces":expected["surfaces"]});
    let weight = normalize_weights(&mut a, &mut b).unwrap();
    let mut error = comparison::difference(&a, &b).unwrap();
    assert!(error <= 1e-9);
    for face in brep.faces() {
        let s = face.surface();
        for u in 0..=16 {
            for v in 0..=16 {
                let (u, v) = (
                    s.parameter_at_u(u as f64 / 16.).unwrap(),
                    s.parameter_at_v(v as f64 / 16.).unwrap(),
                );
                let distance = source
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(s.evaluate(u, v).unwrap())
                    .unwrap();
                assert!(distance <= 1e-9);
                error = error.max(distance);
            }
        }
    }
    (error, weight)
}
