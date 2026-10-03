use super::*;
use serde_json::Value;

fn p(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}

#[test]
fn bend_point_maps_match_public_sdk_spatial_spines_symmetry_limits_and_angles() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/bend_points.json"
    ))
    .unwrap();
    let captured: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/bend_points.json"
    ))
    .unwrap();
    compare(&fixture, &captured, 1e-11);
}

#[test]
fn bend_point_maps_match_native_center_crossings_and_degenerate_targets() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/bend_edge_points.json"
    ))
    .unwrap();
    let captured: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/bend_edge_points.json"
    ))
    .unwrap();
    // A radius of about 50,000 amplifies the native circle-center subtraction.
    compare(&fixture, &captured, 1e-10);
}

#[test]
fn bend_sdk_validity_and_short_circular_regions_match_native_boundaries() {
    for (fixture, captured) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_angle_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_angle_points.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_boundary_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_boundary_points.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_small_angle_points.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/bend_small_angle_points.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_short_arc_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_short_arc_points.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_validity_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_validity_points.json"),
        ),
    ] {
        compare(
            &serde_json::from_str(fixture).unwrap(),
            &serde_json::from_str(captured).unwrap(),
            1e-10,
        );
    }
}

fn compare(fixture: &Value, captured: &Value, epsilon: Real) {
    assert_eq!(
        fixture["operations"].as_array().unwrap().len(),
        captured["results"].as_array().unwrap().len()
    );
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(captured["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let morph = BendPointMorph::try_new(
            p(&op["start"]),
            p(&op["end"]),
            p(&op["through"]),
            op["angle"].as_f64(),
            op["straight"].as_bool().unwrap(),
            op["symmetric"].as_bool().unwrap(),
            Tolerance::DEFAULT,
        );
        assert_eq!(
            op["points"].as_array().unwrap().len(),
            row["value"]["points"].as_array().unwrap().len()
        );
        assert_eq!(
            morph.is_ok(),
            row["value"]["valid"].as_bool().unwrap(),
            "{}",
            op["id"]
        );
        for (source, expected) in op["points"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["value"]["points"].as_array().unwrap())
        {
            let source = p(source);
            let actual = match &morph {
                Ok(morph) => morph.morph_point(source).unwrap(),
                Err(_) => source,
            };
            let error = actual.distance_to(p(expected)).unwrap();
            assert!(
                error <= epsilon,
                "{}: {source:?} mapped to {actual:?}; expected {expected}; error {error}",
                op["id"]
            );
        }
    }
}

#[test]
fn bend_command_maps_match_limit_to_spine_attenuation_and_rigid_group_placement() {
    let mut compared = 0;
    for (fixture, captured) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_command_points.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_command_points.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_command_followup.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_command_followup.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_angle_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_angle_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/bend_rigid_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/bend_rigid_command.json"),
        ),
    ] {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        let captured: Value = serde_json::from_str(captured).unwrap();
        assert_eq!(
            fixture["operations"].as_array().unwrap().len(),
            captured["results"].as_array().unwrap().len()
        );
        for (op, row) in fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(captured["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let value = &row["value"];
            if !value["success"].as_bool().unwrap() {
                // Ten original macros used Angle=value for a prompt-only option.
                // Two corrected macros intentionally try a rejected negative angle.
                assert_eq!(value["before"], value["after"]);
                continue;
            }
            let morph = BendPointMorph::try_for_command(
                p(&op["start"]),
                p(&op["end"]),
                p(&op["through"]),
                op["angle"].as_f64(),
                op["straight"].as_bool().unwrap(),
                op["symmetric"].as_bool().unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .with_non_attenuated(op["non_attenuated"].as_bool().unwrap());
            let sources = op["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(p)
                .collect::<Vec<_>>();
            let rigid = op["rigid"].as_bool().unwrap();
            let transform = rigid.then(|| {
                morph
                    .rigid_transform(
                        crate::BoundingBox3::from_points(sources.iter().copied())
                            .unwrap()
                            .center()
                            .unwrap(),
                    )
                    .unwrap()
            });
            let results = value["after"].as_array().unwrap();
            let results = if op["copy"].as_bool().unwrap() {
                assert_eq!(results.len(), 2 * sources.len());
                &results[sources.len()..]
            } else {
                assert_eq!(results.len(), sources.len());
                results.as_slice()
            };
            for (source, result) in sources.iter().zip(results) {
                let actual = match transform {
                    Some(transform) => transform.transform_point(*source).unwrap(),
                    None => morph.morph_point(*source).unwrap(),
                };
                let error = actual.distance_to(p(&result["point"])).unwrap();
                assert!(
                    error <= if rigid { 1e-7 } else { 1e-11 },
                    "{} {source:?}: {error}",
                    op["id"]
                );
            }
            compared += 1;
        }
    }
    assert_eq!(compared, 36);
}

#[test]
fn bend_keeps_unchanged_regions_and_rejects_nonfinite_construction() {
    let frame = Frame3::try_from_points(
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(1., 0., 0.).unwrap(),
        Point3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (radius, angle) in [(0., 1.), (1., 0.), (f64::NAN, 1.), (1., f64::INFINITY)] {
        assert!(BendPointMorph::try_from_arc(frame, radius, angle, false).is_err());
    }
    let morph = BendPointMorph::try_from_arc(frame, 10., 1., false).unwrap();
    let point = Point3::try_new(-5., 2., 3.).unwrap();
    assert_eq!(morph.morph_point(point).unwrap(), point);
}

#[test]
fn explicit_limited_bends_do_not_validate_an_unused_through_point_radius() {
    let start = Point3::try_new(0., 0., 0.).unwrap();
    let end = Point3::try_new(0., 0., 10.).unwrap();
    let through = Point3::try_new(0.01, 0., 0.).unwrap();
    let angle = 1e-8;
    assert!(
        BendPointMorph::try_new(
            start,
            end,
            through,
            Some(angle),
            false,
            false,
            Tolerance::DEFAULT
        )
        .is_err()
    );
    let morph = BendPointMorph::try_for_command(
        start,
        end,
        through,
        Some(angle),
        true,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(morph.radius(), 10. / angle);
    assert_eq!(morph.arc_length(), 10.);
}

#[test]
fn finite_near_center_images_survive_intermediate_radial_overflow() {
    let frame = Frame3::try_from_points(
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(1., 0., 0.).unwrap(),
        Point3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let radius: Real = 1e308;
    let radial = radius.next_down();
    let morph = BendPointMorph::try_from_arc(frame, radius, 1., false).unwrap();
    let actual = morph
        .morph_point(Point3::try_new(0.6 * radius, radial, 0.).unwrap())
        .unwrap()
        .to_array();
    assert!(actual.into_iter().all(Real::is_finite));
    let difference = radius - radial;
    assert!(actual[0].abs() <= difference);
    assert!((radial..=radius + difference).contains(&actual[1]));
    let center = morph
        .morph_point(Point3::try_new(0.6 * radius, radius, 0.).unwrap())
        .unwrap()
        .to_array();
    assert_eq!(center, [0., radius, 0.]);
}

#[test]
fn bend_fits_the_native_spine_image_and_preserves_rational_control_structures() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/bend_points.json"
    ))
    .unwrap();
    let captured: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/bend_points.json"
    ))
    .unwrap();
    let op = &fixture["operations"][0];
    let expected = &captured["results"][0]["value"]["points"];
    let morph = BendPointMorph::try_new(
        p(&op["start"]),
        p(&op["end"]),
        p(&op["through"]),
        None,
        false,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let line = crate::LineSegment::try_new(p(&op["start"]), p(&op["end"]), tolerance).unwrap();
    let fitted = morph.morph_line(line, tolerance).unwrap();
    assert_eq!(fitted.domain(), line.domain());
    for i in 33..38 {
        let parameter = op["points"][i][2].as_f64().unwrap();
        assert!(
            fitted
                .evaluate(parameter)
                .unwrap()
                .distance_to(p(&expected[i]))
                .unwrap()
                <= tolerance.absolute()
        );
    }

    let indices = [3, 7, 25, 29];
    let weights = [1., 0.5, 1.25, 2.];
    let controls = indices
        .iter()
        .zip(weights)
        .map(|(i, weight)| crate::WeightedPoint3::try_new(p(&op["points"][*i]), weight).unwrap())
        .collect::<Vec<_>>();
    let curve =
        NurbsCurve::try_new_rational(1, controls[..2].to_vec(), vec![2., 2., 8., 8.]).unwrap();
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        controls,
        vec![2., 2., 8., 8.],
        vec![-2., -2., 4., 4.],
    )
    .unwrap();
    let morph = morph.with_preserve_structure(true);
    let mapped_curve = morph.morph_nurbs_curve(&curve, tolerance).unwrap();
    let mapped_surface = morph.morph_nurbs_surface(&surface, tolerance).unwrap();
    assert_eq!(mapped_curve.degree(), curve.degree());
    assert_eq!(mapped_curve.knots(), curve.knots());
    assert_eq!(mapped_curve.domain(), curve.domain());
    assert_eq!(
        (mapped_surface.degree_u(), mapped_surface.degree_v()),
        (surface.degree_u(), surface.degree_v())
    );
    assert_eq!(mapped_surface.knots_u(), surface.knots_u());
    assert_eq!(mapped_surface.knots_v(), surface.knots_v());
    for (mapped, count) in [
        (mapped_curve.control_points(), 2),
        (mapped_surface.control_points(), 4),
    ] {
        assert_eq!(mapped.len(), count);
        for (j, control) in mapped.iter().enumerate() {
            assert_eq!(control.weight(), weights[j]);
            assert!(
                control
                    .point()
                    .distance_to(p(&expected[indices[j]]))
                    .unwrap()
                    <= 1e-11
            );
        }
    }
}
