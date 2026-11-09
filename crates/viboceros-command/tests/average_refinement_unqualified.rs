//! Retain the full discovery scope while averaged refinement is unqualified.
#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "support/surface_match.rs"]
mod matching;
use viboceros_geometry::*;

#[test]
fn refinement_rejects_every_discovery_case_before_backend_work() {
    let mut count = 0;
    for family in [
        "match_surface_average_holdouts",
        "match_surface_average_refinement_pilot",
        "match_surface_average_refinement_iso",
        "match_surface_average_refinement_tolerances",
        "match_surface_average_refinement_signs",
        "match_surface_average_position_refinement",
        "match_surface_average_fit_stages",
    ] {
        for row in captured::read(family)["results"].as_array().unwrap() {
            let recipe = &row["value"]["prescribed_case"];
            if recipe["refine"] != true {
                continue;
            }
            let a = captured::surface(&recipe["sources"][0]);
            let b = captured::surface(&recipe["sources"][1]);
            let before = (a.clone(), b.clone());
            let options = SurfaceAverageOptions {
                matching: matching::options(recipe),
                reverse_target: recipe["reverse_target"].as_bool().unwrap(),
            };
            let first_side = matching::side(recipe["side"].as_str().unwrap());
            let second_side = matching::side(recipe["target_side"].as_str().unwrap());
            let result = try_average_match_nurbs_surfaces_with_operations(
                &a,
                first_side,
                &b,
                second_side,
                options,
                captured::model_tolerance(),
                &mut SurfaceMatchOperations {
                    clamp: &mut |_| panic!("unqualified refinement reached clamping"),
                    elevate: &mut |_, _| panic!("unqualified refinement reached elevation"),
                    interpolate: &mut |_| panic!("unqualified refinement reached interpolation"),
                    refine: &mut |_, _| panic!("unqualified refinement reached insertion"),
                },
            );
            assert_rejected(result, &row["id"]);
            #[cfg(feature = "native-smlib")]
            assert_rejected(
                viboceros_smlib::average_match_surface_supports(
                    &a,
                    first_side,
                    &b,
                    second_side,
                    options,
                    captured::model_tolerance(),
                )
                .map_err(|error| match error {
                    viboceros_smlib::Error::Geometry(error) => error,
                    error => panic!("unexpected refinement error: {error:?}"),
                }),
                &row["id"],
            );
            assert_eq!((a, b), before);
            count += 1;
        }
    }
    assert_eq!(count, 152);
}

fn assert_rejected(result: Result<[NurbsSurface; 2], GeometryError>, id: &serde_json::Value) {
    assert!(
        matches!(
            result,
            Err(GeometryError::InvalidSurfaceMatch {
                context: "averaging requires refinement off"
            })
        ),
        "{id}: {result:?}"
    );
}
