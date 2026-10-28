use serde_json::Value;
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, CurveRef, LineSegment, NurbsCurve, NurbsSurface, Point3, Sweep1,
    SweepBlend, SweepEndPoints, SweepFrameStyle, SweepSection, Tolerance, Vector3, WeightedPoint3,
};
#[path = "support/positive_tensor.rs"]
mod positive_tensor;

fn p(v: [f64; 3]) -> Point3 {
    Point3::try_from(v).unwrap()
}
fn point(v: &Value) -> Point3 {
    p(serde_json::from_value(v.clone()).unwrap())
}
fn line(v: &Value, tol: Tolerance) -> LineSegment {
    LineSegment::try_new(point(&v["start"]), point(&v["end"]), tol).unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                WeightedPoint3::try_new(point(&v["point"]), v["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn boundary(s: &NurbsSurface, row: usize) -> NurbsCurve {
    assert_eq!(s.degree_v(), 1);
    assert_eq!(s.control_point_count_v(), 2);
    assert!((0..s.control_point_count_u()).all(
        |u| s.control_point(u, 0).unwrap().weight() == s.control_point(u, 1).unwrap().weight()
    ));
    NurbsCurve::try_new_rational(
        s.degree_u(),
        (0..s.control_point_count_u())
            .map(|u| s.control_point(u, row).unwrap())
            .collect(),
        s.knots_u().to_vec(),
    )
    .unwrap()
}

#[test]
fn native_point_commands_match_complete_images_in_either_u_direction() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let tol = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let mut count = 0;
    for family in ["sweep1_point_ends_commands", "sweep1_point_ends_retained"] {
        let input: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let output: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(output["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            if op["blend"] == 1 {
                continue;
            } // Separate existing Global multi-profile diagnosis.
            let rail = line(&op["rail"], tol);
            let sections = op["sections"]
                .as_array()
                .unwrap()
                .iter()
                .zip(op["parameters"].as_array().unwrap())
                .map(|(v, t)| SweepSection {
                    parameter: t.as_f64().unwrap(),
                    curve: line(v, tol).to_nurbs().unwrap(),
                })
                .collect::<Vec<_>>();
            let ends = SweepEndPoints {
                start: op.get("start_point").map(point),
                end: op.get("end_point").map(point),
            };
            let model = Sweep1::try_new_with_point_ends(
                CurveRef::Line(&rail),
                &sections,
                ends,
                SweepFrameStyle::Freeform,
                SweepBlend::Local,
                tol,
            )
            .unwrap();
            // Actual Point commands request a cubic rail fit for either refit option.
            let actual = model.to_surface().unwrap();
            assert_eq!(row["value"].as_array().unwrap().len(), 1);
            let expected = surface(&row["value"][0]["definition"]);
            let a0 = actual.control_point(0, 0).unwrap().point();
            let reverse = a0
                .distance_to(expected.control_point(0, 0).unwrap().point())
                .unwrap()
                > a0.distance_to(
                    expected
                        .control_point(expected.control_point_count_u() - 1, 0)
                        .unwrap()
                        .point(),
                )
                .unwrap();
            let mut maximum = 0_f64;
            for v in 0..2 {
                let a = boundary(&actual, v);
                let mut b = boundary(&expected, v);
                if reverse {
                    b = b.reversed().unwrap();
                }
                let bound = a.parameter_curve_deviation_bound(&b, 1e-7).unwrap();
                println!(
                    "POINT_END {} v{v} reverse={reverse} complete bound {bound:?}",
                    op["id"]
                );
                maximum = maximum
                    .max(bound.expect("complete point-ended surface should match the command"));
            }
            assert!(maximum <= tol.absolute());
            println!(
                "POINT_END_RESULT {}",
                serde_json::json!({"family":family,"id":op["id"],"complete_image_bound":maximum,"reversed_u":reverse})
            );
            count += 1;
        }
    }
    assert_eq!(count, 10);
}

#[test]
fn poles_keep_exact_vertices_and_sew_a_closed_profile() {
    let tol = Tolerance::DEFAULT;
    let rail = LineSegment::try_new(p([0., 0., 0.]), p([0., 0., 5.]), tol).unwrap();
    let circle = Circle3::try_new(
        p([0., 0., 2.5]),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        tol,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let sections = [SweepSection {
        parameter: 2.5,
        curve: circle,
    }];
    let ends = SweepEndPoints {
        start: Some(p([0., 0., 0.])),
        end: Some(p([0., 0., 5.])),
    };
    let mut called = 0;
    let model = Sweep1::try_new_with_point_ends_and_section_preparation(
        CurveRef::Line(&rail),
        &sections,
        ends,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        tol,
        &mut |curves, maximum, limit| {
            called += 1;
            assert_eq!(curves.len(), 1);
            viboceros_geometry::prepare_nurbs_sections_with_operations(
                curves,
                maximum,
                limit,
                viboceros_geometry::SectionWeightScale::Common,
                &mut |c| c.try_trimmed(c.domain()),
                &mut |c, d| c.try_change_degree(d, false),
                &mut |c, t, m| c.try_insert_knot(t, m),
            )
        },
    )
    .unwrap();
    assert_eq!(called, 1);
    let s = model.to_surface().unwrap();
    for (u, point) in [
        (0, ends.start.unwrap()),
        (s.control_point_count_u() - 1, ends.end.unwrap()),
    ] {
        assert!(
            (0..s.control_point_count_v()).all(|v| s.control_point(u, v).unwrap().point() == point)
        );
    }
    let brep = Brep::try_surface_grid(&s, &[], &[], tol).unwrap();
    assert_eq!(brep.vertices().len(), 2);
    assert_eq!(brep.faces().len(), 1);
    assert!(brep.edge_use_counts().iter().all(|&count| count == 2));
    assert_eq!(sections[0].parameter, 2.5);
}

#[test]
fn conflicting_station_and_invalid_preparation_do_not_discard_profiles() {
    let tol = Tolerance::DEFAULT;
    let rail = LineSegment::try_new(p([0., 0., 0.]), p([0., 0., 5.]), tol).unwrap();
    let curve = LineSegment::try_new(p([0., 0., 0.]), p([2., 0., 0.]), tol)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let sections = [SweepSection {
        parameter: 0.,
        curve,
    }];
    let original = sections.clone();
    let ends = SweepEndPoints {
        start: Some(p([0., 0., 0.])),
        end: None,
    };
    assert!(
        Sweep1::try_new_with_point_ends(
            CurveRef::Line(&rail),
            &sections,
            ends,
            SweepFrameStyle::Freeform,
            SweepBlend::Local,
            tol
        )
        .is_err()
    );
    assert!(
        Sweep1::try_new_with_point_ends_and_section_preparation(
            CurveRef::Line(&rail),
            &sections,
            ends,
            SweepFrameStyle::Freeform,
            SweepBlend::Local,
            tol,
            &mut |_, _, _| Ok::<_, viboceros_geometry::GeometryError>(vec![])
        )
        .is_err()
    );
    assert_eq!(sections, original);
}

#[test]
fn tilted_rail_keeps_authored_poles_exact() {
    let tol = Tolerance::DEFAULT;
    let arc = CircularArc3::try_from_three_points(
        p([3.1, 0.2, 1.7]),
        p([2.3, 2.8, 3.9]),
        p([-0.7, 3.2, 2.1]),
        tol,
    )
    .unwrap();
    let rail = CurveRef::Arc(&arc);
    let middle = rail.parameter_at(0.5).unwrap();
    let sections = [SweepSection {
        parameter: middle,
        curve: LineSegment::try_new(p([2.3, 2.8, 3.9]), p([2.9, 3.1, 4.7]), tol)
            .unwrap()
            .to_nurbs()
            .unwrap(),
    }];
    let ends = SweepEndPoints {
        start: Some(p([0.1, -0.7, 1.3])),
        end: Some(p([4.9, 2.7, -0.3])),
    };
    let model = Sweep1::try_new_with_point_ends(
        rail,
        &sections,
        ends,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        tol,
    )
    .unwrap();
    let curves = model
        .sections_at(&[*rail.domain().start(), *rail.domain().end()])
        .unwrap();
    for (curve, point) in curves.iter().zip([ends.start.unwrap(), ends.end.unwrap()]) {
        assert!(
            curve.control_points().iter().all(|c| c.point() == point),
            "pole moved: {curve:?}"
        );
    }
}

#[test]
fn closed_point_commands_match_complete_positive_rational_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/sweep1_point_ends_closed.json")).unwrap(),
    )
    .unwrap();
    let observed: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/sweep1_point_ends_closed.json")).unwrap(),
    )
    .unwrap();
    let tol = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let rail = line(&op["rail"], tol);
        let circle = &op["sections"][0];
        let curve = Circle3::try_new(
            point(&circle["center"]),
            circle["radius"].as_f64().unwrap(),
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tol,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let sections = [SweepSection {
            parameter: op["parameters"][0].as_f64().unwrap(),
            curve,
        }];
        let ends = SweepEndPoints {
            start: op.get("start_point").map(point),
            end: op.get("end_point").map(point),
        };
        let a = Sweep1::try_new_with_point_ends(
            CurveRef::Line(&rail),
            &sections,
            ends,
            SweepFrameStyle::Freeform,
            SweepBlend::Local,
            tol,
        )
        .unwrap()
        .to_surface()
        .unwrap()
        .try_reparameterized(0. ..=1., 0. ..=1.)
        .unwrap();
        let b = surface(&row["value"][0]["definition"]);
        let b = if ends.start.is_some() {
            b.try_reversed_u().unwrap()
        } else {
            b
        };
        let bound = positive_tensor::complete_bound(&a, &b);
        assert!(bound < 1e-7, "{}: {bound}", op["id"]);
        println!(
            "POINT_END_RESULT {}",
            serde_json::json!({"family":"sweep1_point_ends_closed", "id":op["id"], "complete_image_bound":bound, "reversed_u":ends.start.is_some()})
        );
    }
}
