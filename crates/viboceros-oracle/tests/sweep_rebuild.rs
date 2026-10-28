use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "../../viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;

#[test]
fn rebuilt_sweep_sections_match_complete_sdk_and_command_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut checked = 0;
    for (family, count) in [
        ("sweep1_rebuild_sdk", 16),
        ("sweep1_rebuild_commands", 12),
        ("sweep1_rebuild_holdouts", 4),
    ] {
        let bytes = std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap();
        let request: ProbeRequest = serde_json::from_slice(&bytes).unwrap();
        let input: Value = serde_json::from_slice(&bytes).unwrap();
        let observed: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), count);
        assert_eq!(observed["results"].as_array().unwrap().len(), count);
        for ((op, a), b) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(actual.results)
            .zip(observed["results"].as_array().unwrap())
        {
            assert_eq!(a.id, b["id"].as_str().unwrap());
            assert_eq!(op["id"], b["id"]);
            assert_eq!(a.value.as_array().unwrap().len(), 1);
            assert_eq!(b["value"].as_array().unwrap().len(), 1);
            let a = nurbs_surface::from_definition(&a.value[0]["definition"]);
            let b = nurbs_surface::from_definition(&b["value"][0]["definition"]);
            let first = a.control_point(0, 0).unwrap().point();
            let reverse = first
                .distance_to(b.control_point(0, 0).unwrap().point())
                .unwrap()
                > first
                    .distance_to(
                        b.control_point(b.control_point_count_u() - 1, 0)
                            .unwrap()
                            .point(),
                    )
                    .unwrap();
            let b = if reverse {
                b.try_reversed_u().unwrap()
            } else {
                b
            };
            let bound = profile_rows::complete_bound(&a, &b, 1e-7)
                .unwrap_or_else(|| panic!("{family} {} whole surface not within 1e-7", op["id"]));
            assert!(
                bound <= 1e-7,
                "{family} {} complete-image bound {bound:e}",
                a.control_point_count_v()
            );
            println!(
                "SWEEP_REBUILD_RESULT {}",
                serde_json::json!({
                    "family":family,"id":op["id"],"complete_image_bound":bound,
                    "comparison_epsilon":1e-7,"reversed_u":reverse,
                })
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 32);
}

fn profile_surface() -> viboceros_geometry::NurbsSurface {
    use viboceros_geometry::{NurbsSurface, Point3};
    NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| Point3::try_new(u as f64 / 2., v as f64, 0.).unwrap())
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn row_certificate_checks_interior_rows_and_weight_coupling() {
    use viboceros_geometry::WeightedPoint3;
    let a = profile_surface();
    for perturb_weight in [false, true] {
        let controls = (0..2)
            .flat_map(|v| {
                let a = &a;
                (0..3).map(move |u| {
                    let p = a.control_point(u, v).unwrap().point();
                    let changed = u == 1 && v == 1;
                    let p = if changed && !perturb_weight {
                        viboceros_geometry::Point3::try_new(p.x(), p.y(), 0.5).unwrap()
                    } else {
                        p
                    };
                    WeightedPoint3::try_new(p, if changed && perturb_weight { 2. } else { 1. })
                        .unwrap()
                })
            })
            .collect();
        let b = viboceros_geometry::NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            a.knots_u().to_vec(),
            a.knots_v().to_vec(),
        )
        .unwrap();
        assert!(profile_rows::complete_bound(&a, &b, 1e-7).is_none());
    }
}

#[test]
fn row_certificate_accepts_refinement_affine_domains_and_common_weight_scales() {
    use viboceros_geometry::{NurbsSurface, WeightedPoint3};
    let a = profile_surface();
    let b = a
        .try_insert_knot_v(0.5, 1)
        .unwrap()
        .try_reparameterized(10. ..=30., 5. ..=12.)
        .unwrap();
    assert!(profile_rows::complete_bound(&a, &b, 1e-12).unwrap() <= 1e-12);
    let b = NurbsSurface::try_new_rational(
        a.degree_u(),
        a.degree_v(),
        a.control_point_count_u(),
        a.control_point_count_v(),
        a.control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), 5.).unwrap())
            .collect(),
        a.knots_u().to_vec(),
        a.knots_v().to_vec(),
    )
    .unwrap();
    assert!(profile_rows::complete_bound(&a, &b, 1e-12).unwrap() <= 1e-12);
}
