//! Direct public MakeCompatible references, before any Sweep surface construction.
use serde_json::{Value, json};
use viboceros_geometry::{
    NurbsCurve, NurbsSurface, ParameterSide, Point3, Sweep1, SweepSection, Tolerance,
    WeightedPoint3,
};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;
#[path = "support/profile_rows.rs"]
mod profile_rows;

const EPSILON: f64 = 1e-7;

#[test]
fn native_batch_output_can_exceed_half_source_tolerance_by_more_than_comparison_epsilon() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |directory| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{directory}/sweep1_refit_sections.json"))).unwrap(),
        )
        .unwrap()
    };
    let (query, observed) = (read("fixtures"), read("observations"));
    let id = "open-refit-1e-06-original-count-2";
    let operation = query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["id"] == id)
        .unwrap();
    let row = observed["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    let source = definitions::curve(&operation["sections"][0]);
    let native = definitions::curve(&row["value"]["curves"][0]);
    let q = native.parameter_at(0.671875).unwrap();
    let (start, end) = native
        .spans()
        .find(|&(start, end)| start < q && q < end)
        .unwrap();
    // This is the same midpoint stencil used by interval reconstruction.
    let parameter = start + (end - start) * 0.5;
    assert!((q - parameter).abs() < 16. * f64::EPSILON * (end - start));
    let point = native.evaluate(parameter).unwrap();
    let reserve = 1e-12;
    assert!(
        native
            .positions_within_distance(&[(parameter, ParameterSide::Right, point)], reserve)
            .unwrap()
    );
    let constant = NurbsCurve::try_new(1, vec![point; 2], vec![0., 0., 1., 1.]).unwrap();
    let bracket = source
        .minimum_distance_with_curve_seeded(
            &constant,
            Tolerance::try_new(1e-10, 1e-12, 1e-10).unwrap(),
            [source.parameter_at(0.7379538221464323).unwrap(), 0.],
        )
        .unwrap()
        .unwrap();
    let lower =
        ((bracket.upper_distance - bracket.maximum_error).next_down() - reserve).next_down();
    let half = operation["refit_tolerance"].as_f64().unwrap() * 0.5;
    assert!(lower > half + EPSILON, "{lower}");
    println!(
        "NATIVE_BATCH_SOURCE_RESERVE {}",
        json!({"id":id,"native_parameter":parameter,
        "source_distance_lower_bound":lower,"half_tolerance":half,"comparison_epsilon":EPSILON})
    );
}

fn tensor(curve: &NurbsCurve, projected_extrusion: bool) -> NurbsSurface {
    let controls = curve
        .control_points()
        .iter()
        .flat_map(|control| {
            (0..2).map(move |u| {
                let mut point = control.point().to_array();
                if projected_extrusion {
                    point[2] = f64::from(u);
                }
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), control.weight()).unwrap()
            })
        })
        .collect();
    NurbsSurface::try_new_rational(
        1,
        curve.degree(),
        2,
        curve.control_points().len(),
        controls,
        vec![0., 0., 1., 1.],
        curve.knots().to_vec(),
    )
    .unwrap()
}

fn definition(curve: &NurbsCurve) -> Value {
    json!({"degree":curve.degree(),"knots":curve.knots(),
        "control_points":curve.control_points().iter().map(|control|
            json!({"point":control.point().to_array(),"weight":control.weight()}))
            .collect::<Vec<_>>()})
}

#[test]
fn original_and_c1_single_and_paired_sections_match_complete_public_compatible_curves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/sweep1_refit_compatibility_stages.json")))
                .unwrap(),
        )
        .unwrap()
    };
    let (query, observed) = (read("fixtures"), read("observations"));
    let operations = query["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!((operations.len(), rows.len()), (16, 16));
    let mut checked = 0;
    let mut gaps = Vec::new();
    // Native zero/one-degree controls have already been proved identical in
    // the separate compatibility test. Sweep uses the one-degree default.
    for index in (1..16).step_by(2) {
        let (operation, row) = (&operations[index], &rows[index]);
        assert_eq!(operation["id"], row["id"]);
        assert_eq!(
            operation["refit_angle_tolerance"].as_f64().unwrap(),
            1_f64.to_radians()
        );
        for count in [1, 2] {
            checked += 1;
            let sections = operation["curves"].as_array().unwrap()[..count]
                .iter()
                .enumerate()
                .map(|(i, curve)| SweepSection {
                    parameter: i as f64 * 5.,
                    curve: definitions::curve(curve),
                })
                .collect::<Vec<_>>();
            let before = sections.clone();
            let native = row["value"]["refit_sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|record| record["simplified"] == false && record["source_count"] == count)
                .unwrap();
            let actual =
                Sweep1::refit_sections(&sections, operation["refit_tolerance"].as_f64().unwrap());
            assert_eq!(sections, before);
            let id = format!(
                "{}-count-{count}",
                operation["id"]
                    .as_str()
                    .unwrap()
                    .split("-angle-")
                    .next()
                    .unwrap()
            );
            if let Some(directory) = std::env::var_os("VIBOCEROS_COMPATIBLE_RECORDS") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join(format!("{id}.json")), serde_json::to_vec(&json!({
                    "id":id,"operation":operation,"source_count":count,
                    "actual":actual.as_ref().ok().map(|curves|curves.iter().map(|s|definition(&s.curve)).collect::<Vec<_>>()),
                    "native":native["curves"],"construction_error":actual.as_ref().err().map(ToString::to_string),
                })).unwrap()).unwrap();
            }
            let actual = match actual {
                Ok(actual) => actual,
                Err(error) => {
                    println!(
                        "DIRECT_COMPATIBLE_RESULT {}",
                        json!({"id":id,"construction_error":error.to_string(),"epsilon":EPSILON})
                    );
                    gaps.push(format!("{id}: {error}"));
                    continue;
                }
            };
            let native = native["curves"].as_array().unwrap();
            assert_eq!((actual.len(), native.len()), (count, count));
            let mut comparisons = Vec::new();
            for (actual, native) in actual.iter().zip(native) {
                let a = &actual.curve;
                let b = definitions::curve(native);
                // Projection is 1-Lipschitz: a separated XY image supplies a
                // sufficient full-curve counterexample, irrespective of Z.
                let separation = profile_rows::complete_image_separation(
                    &tensor(a, true),
                    &tensor(&b, true),
                    EPSILON,
                );
                // Identical U rows give a tensor whose complete image is the
                // original XYZ curve. No projection or sampling acceptance.
                let bound = if separation.is_some() {
                    None
                } else {
                    profile_rows::complete_bound(&tensor(a, false), &tensor(&b, false), EPSILON)
                };
                comparisons.push(json!({"complete_image_bound":bound,
                    "complete_image_separation_lower_bound":separation,
                    "actual_controls":a.control_points().len(),"native_controls":b.control_points().len()}));
                if bound.is_none() {
                    gaps.push(format!("{id} complete curve {}", comparisons.len() - 1));
                }
            }
            println!(
                "DIRECT_COMPATIBLE_RESULT {}",
                json!({"id":id,"epsilon":EPSILON,"curves":comparisons})
            );
        }
    }
    assert_eq!(checked, 16);
    assert!(gaps.is_empty(), "MakeCompatible curve gaps: {gaps:#?}");
}
