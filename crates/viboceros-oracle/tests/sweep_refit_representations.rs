//! Distinguish image-preserving input changes from native fitter behavior.
use serde_json::Value;
use std::path::PathBuf;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_parameters.rs"]
mod definitions;
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "../../viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle")
}
fn read(path: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(path)).unwrap()).unwrap()
}

#[test]
fn incremental_clocks_match_public_queries_on_represented_c1_profiles() {
    use viboceros_geometry::{ArcLengthSampler, CurveRef, Point3, Tolerance};
    let query = read("fixtures/sweep1_refit_representation_diagnostics.json");
    let observed = read("observations/sweep1_refit_representation_diagnostics.json");
    let operations = query["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!((operations.len(), rows.len()), (32, 32));
    let mut failed = Vec::new();
    let mut checked = 0;
    for (operation, row) in operations.iter().zip(rows) {
        assert_eq!(operation["id"], row["id"]);
        if operation["op"] != "arc_length_batch" {
            continue;
        }
        let curve = definitions::curve(&operation["curve"]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let fractions: Vec<f64> = serde_json::from_value(operation["fractions"].clone()).unwrap();
        let batch = &row["value"]["numerical_batches"][0];
        let mut errors = Vec::new();
        for parameters in [
            sampler
                .parameters_at_incremental_normalized_lengths(&fractions, 1e-4)
                .unwrap(),
            sampler
                .parameters_at_quadrature_normalized_lengths(&fractions, 1e-4)
                .unwrap(),
        ] {
            let maximum = parameters
                .iter()
                .zip(batch["points"].as_array().unwrap())
                .map(|(&t, p)| {
                    curve
                        .evaluate(t)
                        .unwrap()
                        .distance_to(
                            Point3::try_from(
                                serde_json::from_value::<[f64; 3]>(p.clone()).unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                })
                .fold(0_f64, f64::max);
            errors.push(maximum);
        }
        println!(
            "SWEEP_REPRESENTATION_CLOCK_RESULT {}",
            serde_json::json!({"id":operation["id"],"incremental_error":errors[0],"independent_error":errors[1],"epsilon":1e-7})
        );
        if errors[0] > 1e-7 {
            failed.push(operation["id"].clone());
        }
        checked += 1;
    }
    assert_eq!(checked, 16);
    assert!(
        failed.is_empty(),
        "public batch clock differences: {failed:?}"
    );
}

#[test]
fn public_degree_elevation_and_knot_insertion_preserve_complete_input_images() {
    let originals = read("capture_sources/sweep1_refit_c1_profiles.json");
    let inputs = read("fixtures/sweep1_refit_representations.json");
    let operations = inputs["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 16);
    let mut checked = 0;
    for profile in originals["profiles"].as_array().unwrap() {
        let id = profile["id"].as_str().unwrap();
        let source = definitions::curve(&profile["curve"]);
        for variant in ["affine-domain", "degree4", "inserted-knot"] {
            let name = format!("{id}-{variant}");
            let operation = operations.iter().find(|o| o["id"] == name).unwrap();
            let candidate = definitions::curve(&operation["sections"][0]);
            let bound = source
                .parameter_curve_deviation_bound(&candidate, 1e-12)
                .unwrap()
                .expect("complete input images must agree under normalized native clocks");
            assert!(bound <= 1e-12);
            println!(
                "SWEEP_REPRESENTATION_INPUT_RESULT {}",
                serde_json::json!({"id":name,"complete_image_bound":bound,"epsilon":1e-12})
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 12);
}

#[test]
fn capture_complete_native_surface_images_across_equivalent_representations() {
    let observed = read("observations/sweep1_refit_representations.json");
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(rows.len(), 16);
    let mut images = Vec::new();
    for original in rows
        .iter()
        .filter(|row| row["id"].as_str().unwrap().ends_with("-original"))
    {
        let id = original["id"]
            .as_str()
            .unwrap()
            .strip_suffix("-original")
            .unwrap();
        assert_eq!(original["value"].as_array().unwrap().len(), 1);
        let reference = nurbs_surface::from_definition(&original["value"][0]["definition"]);
        for variant in ["original", "affine-domain", "degree4", "inserted-knot"] {
            let name = format!("{id}-{variant}");
            let row = rows.iter().find(|row| row["id"] == name).unwrap();
            assert_eq!(row["value"].as_array().unwrap().len(), 1);
            let candidate = nurbs_surface::from_definition(&row["value"][0]["definition"]);
            let bound = profile_rows::complete_bound(&reference, &candidate, 1e-7);
            let result = serde_json::json!({
                "id":name,"complete_image_bound":bound,"epsilon":1e-7,
                "original_controls":reference.control_point_count_v(),
                "variant_controls":candidate.control_point_count_v(),
            });
            println!("SWEEP_REPRESENTATION_IMAGE_RESULT {result}");
            images.push(result);
        }
    }
    assert_eq!(images.len(), 16);
    let unproved = images
        .iter()
        .filter(|row| row["complete_image_bound"].is_null())
        .map(|row| row["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        unproved,
        [
            "open-refit-1e-06-inserted-knot",
            "closed-cubic-0.01-inserted-knot"
        ]
    );
}

#[test]
fn inserted_knots_can_change_complete_native_images_beyond_comparison_epsilon() {
    use num_rational::BigRational;
    use num_traits::Signed;
    use viboceros_geometry::{
        NurbsCurve, NurbsSurface, ParameterSide, Point3, Tolerance, WeightedPoint3,
    };
    const PROJECTION_RESERVE: f64 = 1e-12;
    // Weights agree across U. Exact L1 control differences bound projection
    // to the first row by partition of unity, including native control rounding.
    fn projected_row(surface: &NurbsSurface) -> NurbsCurve {
        assert!(
            surface.knots_u()[..=surface.degree_u()]
                .iter()
                .all(|knot| knot == surface.domain_u().start())
        );
        let controls = (0..surface.control_point_count_v())
            .map(|v| {
                let first = surface.control_point(0, v).unwrap();
                let p = first.point().to_array();
                assert_eq!(p[2], 0.);
                assert!(first.weight() > 0.);
                for u in 0..surface.control_point_count_u() {
                    let other = surface.control_point(u, v).unwrap();
                    let difference: BigRational = other.point().to_array()[..2]
                        .iter()
                        .zip(&p[..2])
                        .map(|(&a, &b)| {
                            (BigRational::from_float(a).unwrap()
                                - BigRational::from_float(b).unwrap())
                            .abs()
                        })
                        .sum();
                    assert!(difference <= BigRational::from_float(PROJECTION_RESERVE).unwrap());
                    assert_eq!(other.weight(), first.weight());
                }
                WeightedPoint3::try_new(Point3::try_from(p).unwrap(), first.weight()).unwrap()
            })
            .collect();
        NurbsCurve::try_new_rational(surface.degree_v(), controls, surface.knots_v().to_vec())
            .unwrap()
    }
    let observed = read("observations/sweep1_refit_representations.json");
    let rows = observed["results"].as_array().unwrap();
    for (id, fraction, target_seed) in [
        ("open-refit-1e-06", 429. / 1024., 1.5522176190965062),
        ("closed-cubic-0.01", 365. / 1024., 1.4520201622042264),
    ] {
        let curve = |suffix| {
            let name = format!("{id}-{suffix}");
            let row = rows.iter().find(|row| row["id"] == name).unwrap();
            projected_row(&nurbs_surface::from_definition(
                &row["value"][0]["definition"],
            ))
        };
        let source = curve("original");
        let target = curve("inserted-knot");
        let parameter = source.parameter_at(fraction).unwrap();
        let point = source.evaluate(parameter).unwrap();
        let evaluation_reserve = 1e-12;
        assert!(
            source
                .positions_within_distance(
                    &[(parameter, ParameterSide::Right, point)],
                    evaluation_reserve
                )
                .unwrap()
        );
        let constant = NurbsCurve::try_new(1, vec![point; 2], vec![0., 0., 1., 1.]).unwrap();
        let bracket = target
            .minimum_distance_with_curve_seeded(
                &constant,
                Tolerance::try_new(1e-10, 1e-12, 1e-10).unwrap(),
                [target_seed, 0.],
            )
            .unwrap()
            .expect("a complete global lower bound must separate this native source point");
        let lower = ((bracket.upper_distance - bracket.maximum_error).next_down()
            - evaluation_reserve)
            .next_down();
        let lower = (lower - PROJECTION_RESERVE).next_down();
        assert!(lower > 1e-7, "{id}: {lower}");
        println!(
            "SWEEP_REPRESENTATION_SEPARATION_RESULT {}",
            serde_json::json!({"id":id,"fraction":fraction,"source_parameter":parameter,
                "complete_image_separation_lower_bound":lower,"epsilon":1e-7})
        );
    }
}
