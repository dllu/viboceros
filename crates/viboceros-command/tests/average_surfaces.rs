#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "support/surface_match.rs"]
mod matching;
use viboceros_geometry::*;

#[test]
fn independent_position_and_tangency_averages_match_both_public_sdk_outputs() {
    let mut count = 0;
    for family in [
        "match_surface_holdouts",
        "match_surface_average_pilot",
        "match_surface_average_holdouts",
    ] {
        for row in captured::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let value = &row["value"];
            let recipe = &value["prescribed_case"];
            if recipe["average"] != true
                || recipe["continuity"] == "Curvature"
                || recipe["refine"] == true
            {
                continue;
            }
            println!("AVERAGE_SURFACE_CASE {id}");
            compare_case(id, value, "AVERAGE_SURFACE");
            count += 1;
        }
    }
    assert_eq!(count, 54);
}

#[test]
fn curvature_averages_match_both_public_sdk_outputs() {
    let mut count = 0;
    for family in [
        "match_surface_holdouts",
        "match_surface_average_pilot",
        "match_surface_average_curvature_contexts",
        "match_surface_average_curvature_rows",
        "match_surface_average_curvature_holdouts",
        "match_surface_average_curvature_nonuniform",
    ] {
        for row in captured::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let recipe = &value["prescribed_case"];
            if recipe["average"] != true
                || recipe["continuity"] != "Curvature"
                || recipe["refine"] == true
            {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            println!("AVERAGE_CURVATURE_CASE {id}");
            compare_case(id, value, "AVERAGE_CURVATURE");
            count += 1;
        }
    }
    assert_eq!(count, 65);
}

fn compare_case(id: &str, value: &serde_json::Value, prefix: &str) {
    let recipe = &value["prescribed_case"];
    let first = captured::surface(&recipe["sources"][0]);
    let second = captured::surface(&recipe["sources"][1]);
    let original = (first.clone(), second.clone());
    let options = SurfaceAverageOptions {
        matching: matching::options(recipe),
        reverse_target: recipe["reverse_target"].as_bool().unwrap(),
    };
    #[cfg(feature = "native-smlib")]
    let operation = viboceros_smlib::average_match_surface_supports;
    #[cfg(not(feature = "native-smlib"))]
    let operation = try_average_match_nurbs_surfaces;
    let result = operation(
        &first,
        matching::side(recipe["side"].as_str().unwrap()),
        &second,
        matching::side(recipe["target_side"].as_str().unwrap()),
        options,
        captured::model_tolerance(),
    )
    .unwrap();
    assert_eq!((first, second), original);
    let mut maximum = 0_f64;
    for (surface, key) in result.iter().zip(["output", "target"]) {
        let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
            surface.clone(),
            surface.domain_u(),
            surface.domain_v(),
            false,
            captured::model_tolerance(),
        )
        .unwrap();
        maximum = maximum.max(captured::compare(&brep, &value[key]));
        maximum = maximum.max(captured::compare(&captured::archive(&brep), &value[key]));
    }
    println!(
        "{prefix} {}",
        serde_json::json!({"id":id,"maximum_geometry_error":maximum,"outputs":2,"archives":2})
    );
}

#[test]
fn a_failed_second_average_does_not_change_either_input() {
    use std::cell::Cell;
    let sheet = |offset, count| {
        NurbsSurface::try_new_rational(
            1,
            1,
            2,
            count,
            (0..count)
                .flat_map(|j| {
                    (0..2).map(move |i| {
                        WeightedPoint3::try_new(
                            Point3::try_new(offset + i as f64, j as f64 / (count - 1) as f64, 0.)
                                .unwrap(),
                            1.,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 1., 1.],
            if count == 2 {
                vec![0., 0., 1., 1.]
            } else {
                vec![0., 0., 0.5, 1., 1.]
            },
        )
        .unwrap()
    };
    let first = sheet(0., 2);
    let second = sheet(2., 3);
    let before = (first.clone(), second.clone());
    let solves = Cell::new(0);
    let result = try_average_match_nurbs_surfaces_with_operations(
        &first,
        SurfaceExtensionEdge::East,
        &second,
        SurfaceExtensionEdge::West,
        SurfaceAverageOptions {
            matching: SurfaceMatchOptions {
                continuity: SurfaceBlendContinuity::Position,
                reverse_match: true,
                ..Default::default()
            },
            ..Default::default()
        },
        captured::model_tolerance(),
        &mut SurfaceMatchOperations {
            clamp: &mut |s| Ok(s.clone()),
            elevate: &mut |s, d| s.try_change_degree(d[0], d[1], false),
            refine: &mut |s, t| s.try_insert_knot_v(t, 1),
            interpolate: &mut |system| {
                solves.set(solves.get() + 1);
                if solves.get() == 2 {
                    return Err(GeometryError::InvalidSurfaceMatch {
                        context: "second averaging solve failed",
                    });
                }
                // Linear clamped bases interpolate their own Greville sites.
                Ok(system.targets.to_vec())
            },
        },
    );
    assert!(matches!(
        result,
        Err(GeometryError::InvalidSurfaceMatch {
            context: "second averaging solve failed"
        })
    ));
    assert_eq!(solves.get(), 2);
    assert_eq!((first, second), before);
}
