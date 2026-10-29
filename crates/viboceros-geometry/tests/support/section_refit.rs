//! Full section-fitting pipelines and exact, complete curve-image comparisons.
use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, Sweep1, SweepSection, WeightedPoint3};
#[allow(dead_code)]
#[path = "loft_parameters.rs"]
pub mod definitions;
#[path = "profile_rows.rs"]
mod profile_rows;

const EPSILON: f64 = 1e-7;

fn tensor(curve: &NurbsCurve, axis: Option<usize>) -> NurbsSurface {
    let controls = curve
        .control_points()
        .iter()
        .flat_map(|control| {
            (0..2).map(move |u| {
                let mut point = control.point().to_array();
                if let Some(axis) = axis {
                    point[axis] = f64::from(u);
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
        "control_points":curve.control_points().iter().map(|p|
            json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()})
}

pub fn compare(a: &NurbsCurve, b: &NurbsCurve) -> Value {
    // Orthogonal projection is 1-Lipschitz. Try all coordinate planes for
    // counterexamples, including spatial inputs whose XY image hides a gap.
    let separation = [2, 1, 0].into_iter().find_map(|axis| {
        let (a, b) = (tensor(a, Some(axis)), tensor(b, Some(axis)));
        profile_rows::complete_image_separation(&a, &b, EPSILON)
            .or_else(|| profile_rows::complete_image_separation(&b, &a, EPSILON))
    });
    // Identical U rows retain the complete XYZ curve for positive proofs.
    let bound = if separation.is_some() {
        None
    } else {
        profile_rows::complete_bound(&tensor(a, None), &tensor(b, None), EPSILON)
    };
    json!({"actual_controls":a.control_points().len(),"native_controls":b.control_points().len(),
        "complete_image_bound":bound,"complete_image_separation_lower_bound":separation})
}

pub fn require_complete_family(family: &str, expected: usize) {
    require_complete_cases(family, expected, 0..expected);
}

pub fn require_complete_cases(family: &str, expected: usize, cases: std::ops::Range<usize>) {
    require_complete_family_with_cases(family, expected, cases, |op, sections| {
        Sweep1::refit_sections(sections, op["refit_tolerance"].as_f64().unwrap())
            .map_err(|error| error.to_string())
    });
}

pub fn require_complete_family_with_cases(
    family: &str,
    expected: usize,
    cases: std::ops::Range<usize>,
    mut fit: impl FnMut(&Value, &[SweepSection]) -> Result<Vec<SweepSection>, String>,
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let (query, native) = (read("fixtures"), read("observations"));
    let operations = query["operations"].as_array().unwrap();
    let references = native["results"].as_array().unwrap();
    assert_eq!((operations.len(), references.len()), (expected, expected));
    assert!(cases.start < cases.end && cases.end <= expected);
    let checked = cases.len();
    let mut failures = Vec::new();
    let mut constructed = 0;
    let mut image_passes = 0;
    let mut image_gaps = 0;
    let mut inconclusive = 0;
    for (op, row) in operations
        .iter()
        .zip(references)
        .skip(cases.start)
        .take(checked)
    {
        assert_eq!(op["id"], row["id"]);
        assert_eq!(row["value"]["before"], row["value"]["after"]);
        let sections = op["sections"]
            .as_array()
            .unwrap()
            .iter()
            .zip(op["parameters"].as_array().unwrap())
            .map(|(c, t)| SweepSection {
                parameter: t.as_f64().unwrap(),
                curve: definitions::curve(c),
            })
            .collect::<Vec<_>>();
        let before = sections.clone();
        let actual = fit(op, &sections);
        assert_eq!(sections, before);
        let mut comparisons = Vec::new();
        if let Ok(actual) = &actual {
            constructed += 1;
            let reference = row["value"]["curves"].as_array().unwrap();
            assert_eq!(
                (actual.len(), reference.len()),
                (sections.len(), sections.len())
            );
            for ((actual, native), source) in actual.iter().zip(reference).zip(&sections) {
                assert_eq!(actual.parameter, source.parameter);
                comparisons.push(compare(&actual.curve, &definitions::curve(native)));
            }
            if comparisons
                .iter()
                .all(|c| c["complete_image_bound"].is_number())
            {
                image_passes += 1;
            } else {
                if comparisons
                    .iter()
                    .any(|c| c["complete_image_separation_lower_bound"].is_number())
                {
                    image_gaps += 1;
                } else {
                    inconclusive += 1;
                }
                failures.push(format!("{}: complete native image", op["id"]));
            }
        } else {
            failures.push(format!("{}: {}", op["id"], actual.as_ref().unwrap_err()));
        }
        let result = json!({"family":family,"id":op["id"],"epsilon":EPSILON,
            "construction_error":actual.as_ref().err().map(ToString::to_string),"curves":comparisons});
        if let Some(directory) = std::env::var_os("VIBOCEROS_SECTION_REFIT_RECORDS") {
            let directory = std::path::PathBuf::from(directory).join(family);
            std::fs::create_dir_all(&directory).unwrap();
            let mut record = result.clone();
            record["operation"] = op.clone();
            record["native"] = row["value"]["curves"].clone();
            record["actual"] = json!(actual.as_ref().ok().map(|curves| {
                curves
                    .iter()
                    .map(|s| definition(&s.curve))
                    .collect::<Vec<_>>()
            }));
            std::fs::write(
                directory.join(format!("{}.json", op["id"].as_str().unwrap())),
                serde_json::to_vec(&record).unwrap(),
            )
            .unwrap();
        }
        println!("SECTION_REFIT_IMAGE {result}");
    }
    println!(
        "SECTION_REFIT_SUMMARY {}",
        json!({"family":family,"contexts":checked,
        "constructed":constructed,"complete_image_passes":image_passes,
        "certified_image_gaps":image_gaps,"inconclusive":inconclusive})
    );
    assert!(
        failures.is_empty(),
        "Section Refit differences: {failures:#?}"
    );
}
