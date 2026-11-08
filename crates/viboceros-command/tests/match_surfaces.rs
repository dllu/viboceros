#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
use serde_json::json;
use viboceros_geometry::*;
fn side(s: &str) -> SurfaceExtensionEdge {
    match s {
        "West" => SurfaceExtensionEdge::West,
        "South" => SurfaceExtensionEdge::South,
        "East" => SurfaceExtensionEdge::East,
        "North" => SurfaceExtensionEdge::North,
        _ => panic!(),
    }
}
fn options(v: &serde_json::Value) -> SurfaceMatchOptions {
    SurfaceMatchOptions {
        continuity: match v["continuity"].as_str().unwrap() {
            "Position" => SurfaceBlendContinuity::Position,
            "Tangency" => SurfaceBlendContinuity::Tangency,
            "Curvature" => SurfaceBlendContinuity::Curvature,
            _ => panic!(),
        },
        other_end: match v["other_end"].as_str().unwrap() {
            "None" => CurveMatchPreserveEnd::None,
            "Position" => CurveMatchPreserveEnd::Position,
            "Tangency" => CurveMatchPreserveEnd::Tangency,
            "Curvature" => CurveMatchPreserveEnd::Curvature,
            _ => panic!(),
        },
        iso: match v["iso"].as_str().unwrap() {
            "Automatic" => SurfaceMatchIso::Automatic,
            "MatchTarget" => SurfaceMatchIso::MatchTarget,
            "Perpendicular" => SurfaceMatchIso::Perpendicular,
            "Preserve" => SurfaceMatchIso::Preserve,
            _ => panic!(),
        },
        reverse_match: v["reverse_match"].as_bool().unwrap(),
        closest_points: v["closest_points"].as_bool().unwrap(),
        refine: v["refine"].as_bool().unwrap(),
    }
}
#[test]
fn surface_matching_from_authored_jets_matches_public_sdk() {
    let mut count = 0;
    for family in [
        "match_surface_pilot",
        "match_surface_holdouts",
        "match_surface_final",
        "match_surface_oriented",
        "match_surface_refinement",
    ] {
        for row in captured::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let value = &row["value"];
            let recipe = &value["prescribed_case"];
            if recipe["average"] == true {
                continue;
            }
            println!("MATCH_SURFACE_CASE {id}");
            let a = captured::surface(&recipe["sources"][0]);
            let b = captured::surface(&recipe["sources"][1]);
            let original = (a.clone(), b.clone());
            #[cfg(feature = "native-smlib")]
            let operation = viboceros_smlib::match_surface_supports;
            #[cfg(not(feature = "native-smlib"))]
            let operation = try_match_nurbs_surfaces;
            let tolerance = Tolerance::try_new(
                1e-6,
                1e-9,
                recipe
                    .get("angle_tolerance")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1e-6),
            )
            .unwrap();
            let result = operation(
                &a,
                side(recipe["side"].as_str().unwrap()),
                &b,
                side(recipe["target_side"].as_str().unwrap()),
                options(recipe),
                tolerance,
            )
            .unwrap();
            assert_eq!((a, b), original);
            let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
                result.clone(),
                result.domain_u(),
                result.domain_v(),
                false,
                captured::model_tolerance(),
            )
            .unwrap();
            let error = captured::compare(&brep, &value["output"]);
            let restored = captured::archive(&brep);
            let error = error.max(captured::compare(&restored, &value["output"]));
            println!(
                "MATCH_SURFACE {}",
                json!({"id":id,"maximum_geometry_error":error,"archives":1,"qualified_capture":family!="match_surface_final"})
            );
            count += 1;
        }
    }
    assert_eq!(count, 91);
}

#[test]
fn native_refined_control_definitions_confirm_the_recorded_tolerance_excess() {
    let records = captured::read("match_surface_refinement");
    for row in records["results"].as_array().unwrap() {
        let value = &row["value"];
        let surface = captured::surface(&value["output"]["surfaces"][0]);
        let target = captured::surface(&value["prescribed_case"]["sources"][1]);
        let mut max_query_difference = 0_f64;
        let mut maximum = 0_f64;
        for sample in value["audit"]["samples"].as_array().unwrap() {
            let (_, u, v) = surface
                .evaluate_with_derivatives(*surface.domain_u().end(), sample["v"].as_f64().unwrap())
                .unwrap();
            let (_, a, b) = target
                .evaluate_with_derivatives(
                    *target.domain_u().start(),
                    sample["target_v"].as_f64().unwrap(),
                )
                .unwrap();
            let n = u
                .cross(v)
                .unwrap()
                .normalized_nonzero()
                .unwrap()
                .as_vector();
            let m = a
                .cross(b)
                .unwrap()
                .normalized_nonzero()
                .unwrap()
                .as_vector();
            let angle = n
                .cross(m)
                .unwrap()
                .length()
                .unwrap()
                .atan2(n.dot(m).unwrap().abs());
            maximum = maximum.max(angle);
            max_query_difference =
                max_query_difference.max((angle - sample["angle"].as_f64().unwrap()).abs());
        }
        // Rhino's acos-based VectorAngle rounds almost parallel normals at
        // sqrt(machine epsilon); our atan2-based comparison retains tiny angles.
        assert!(max_query_difference <= 3e-8);
        if value["prescribed_case"]["continuity"] != "Position" {
            assert!(maximum > value["audit"]["settings"]["angle"].as_f64().unwrap());
        }
        println!(
            "MATCH_REFINED_NORMAL_AUDIT {}",
            json!({"id":row["id"],"maximum_angle":maximum,"maximum_query_difference":max_query_difference})
        );
    }
}

#[test]
fn failed_preparation_and_work_limits_leave_native_operands_reusable() {
    let row = &captured::read("match_surface_pilot")["results"][4];
    let value = &row["value"];
    let recipe = &value["prescribed_case"];
    let source = captured::surface(&recipe["sources"][0]);
    let target = captured::surface(&recipe["sources"][1]);
    let snapshot = (source.clone(), target.clone());
    #[cfg(feature = "native-smlib")]
    let operation = viboceros_smlib::match_surface_supports;
    #[cfg(not(feature = "native-smlib"))]
    let operation = try_match_nurbs_surfaces;
    let singular = NurbsSurface::try_new_rational(
        3,
        3,
        4,
        4,
        (0..4)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    WeightedPoint3::try_new(Point3::try_new((i + j) as f64, 0., 0.).unwrap(), 1.)
                        .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(
        operation(
            &source,
            SurfaceExtensionEdge::East,
            &singular,
            SurfaceExtensionEdge::West,
            options(recipe),
            captured::model_tolerance()
        )
        .is_err()
    );
    let mut knots = vec![0.];
    knots.extend((0..=400).map(|n| n as f64));
    knots.push(400.);
    let large = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        401,
        (0..401)
            .flat_map(|j| {
                (0..2).map(move |i| {
                    WeightedPoint3::try_new(Point3::try_new(i as f64, j as f64, 0.).unwrap(), 1.)
                        .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1.],
        knots,
    )
    .unwrap();
    assert!(
        operation(
            &large,
            SurfaceExtensionEdge::East,
            &target,
            SurfaceExtensionEdge::West,
            options(recipe),
            captured::model_tolerance()
        )
        .is_err()
    );
    let result = operation(
        &source,
        SurfaceExtensionEdge::East,
        &target,
        SurfaceExtensionEdge::West,
        options(recipe),
        captured::model_tolerance(),
    )
    .unwrap();
    let b = Brep::try_rectangular_surface_face_with_surface_edge_domains(
        result.clone(),
        result.domain_u(),
        result.domain_v(),
        false,
        captured::model_tolerance(),
    )
    .unwrap();
    captured::compare(&b, &value["output"]);
    assert_eq!((source, target), snapshot);
}

#[test]
fn malformed_backend_interpolation_is_a_typed_failure() {
    let recipe = &captured::read("match_surface_pilot")["results"][4]["value"]["prescribed_case"];
    let source = captured::surface(&recipe["sources"][0]);
    let target = captured::surface(&recipe["sources"][1]);
    for invalid in 0..2 {
        let result = try_match_nurbs_surfaces_with_operations(
            &source,
            SurfaceExtensionEdge::East,
            &target,
            SurfaceExtensionEdge::West,
            options(recipe),
            captured::model_tolerance(),
            &mut SurfaceMatchOperations {
                clamp: &mut |s| Ok(s.clone()),
                elevate: &mut |s, d| s.try_change_degree(d[0], d[1], false),
                refine: &mut |s, t| s.try_insert_knot_v(t, 1),
                interpolate: &mut |job| {
                    Ok::<_, GeometryError>(if invalid == 0 {
                        Vec::new()
                    } else {
                        vec![f64::NAN; job.parameters.len() * job.columns]
                    })
                },
            },
        );
        assert!(matches!(
            result,
            Err(GeometryError::InvalidSurfaceMatch { .. })
        ));
    }
}
