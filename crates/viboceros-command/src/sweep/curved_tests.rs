//! World-space samples on complete captured definitions, independent of U/V charts.
use super::automatic_tests::{add_line, complete_ruled_bound, native_surface, point};
use super::*;
use serde_json::Value;
use viboceros_geometry::{CircularArc3, WeightedPoint3};

pub(super) fn rail(doc: &mut Document, v: &Value) -> ObjectId {
    let geometry = match v["type"].as_str().unwrap() {
        "arc" => Geometry::Arc(
            CircularArc3::try_from_three_points(
                point(&v["points"][0]),
                point(&v["points"][1]),
                point(&v["points"][2]),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "nurbs" => Geometry::NurbsCurve(
            NurbsCurve::try_new_rational(
                v["degree"].as_u64().unwrap() as usize,
                v["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap())
                            .unwrap()
                    })
                    .collect(),
                serde_json::from_value(v["knots"].clone()).unwrap(),
            )
            .unwrap(),
        ),
        "line" => return add_line(doc, v),
        _ => panic!("curved rail expected"),
    };
    doc.add_geometry(geometry).unwrap()
}

#[test]
fn automatic_extent_and_direction_follow_native_anchor_and_arc_length_choices() {
    let mut checked = 0;
    for (input, observed) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/sweep1_automatic_stations.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/sweep1_automatic_stations.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/sweep1_automatic_direction.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/sweep1_automatic_direction.json"
            ),
        ),
    ] {
        let input: Value = serde_json::from_str(input).unwrap();
        let observed: Value = serde_json::from_str(observed).unwrap();
        for op in input["operations"].as_array().unwrap() {
            // Rhino's non-Greville arc refit has a separate endpoint/clock gap;
            // retain that raw investigation record without qualifying it here.
            if op["id"] == "arc-thirty" {
                continue;
            }
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            let rail = rail(&mut doc, &op["rail"]);
            let profile = add_line(&mut doc, &op["sections"][0]);
            doc.select_objects_direct([rail, profile], SelectionMode::Replace)
                .unwrap();
            registry
                .execute(&mut doc, "Sweep1")
                .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
            let actual = doc
                .objects()
                .find_map(|o| {
                    if let Geometry::Brep(b) = o.geometry() {
                        Some(b.faces()[0].surface())
                    } else {
                        None
                    }
                })
                .unwrap();
            let native = observed["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == op["id"])
                .unwrap();
            let expected = native_surface(&native["value"][0]["definition"]);
            for (a, b) in [
                (0, 0),
                (
                    actual.control_point_count_u() - 1,
                    expected.control_point_count_u() - 1,
                ),
            ] {
                let error = actual
                    .control_point(a, 0)
                    .unwrap()
                    .point()
                    .distance_to(expected.control_point(b, 0).unwrap().point())
                    .unwrap();
                assert!(error < 1e-7, "{} endpoint {a}: {error}", op["id"]);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 12);
}

#[test]
fn automatic_curved_placement_matches_seven_captures_and_retains_an_existing_frame_gap() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/sweep1_automatic_curved.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/sweep1_automatic_curved.json"
    ))
    .unwrap();
    let mut gaps = vec![];
    for op in input["operations"].as_array().unwrap() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let rail = rail(&mut doc, &op["rail"]);
        let profiles = op["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| add_line(&mut doc, c))
            .collect::<Vec<_>>();
        doc.select_objects_direct(
            std::iter::once(rail).chain(profiles),
            SelectionMode::Replace,
        )
        .unwrap();
        if let Err(error) = registry.execute(&mut doc, "Sweep1") {
            gaps.push(format!("{}: {error}", op["id"]));
            continue;
        }
        let output = doc
            .objects()
            .find_map(|o| {
                if let Geometry::Brep(b) = o.geometry() {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(output.faces().len(), 1);
        let actual = output.faces()[0].surface();
        let row = observed["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == op["id"])
            .unwrap();
        let expected = native_surface(&row["value"][0]["definition"]);
        let mut maximum = 0_f64;
        for (a, b) in [(actual, &expected), (&expected, actual)] {
            for u in 0..9 {
                for v in 0..3 {
                    let du = a.domain_u();
                    let dv = a.domain_v();
                    let p = a
                        .evaluate(
                            (du.end() - du.start()).mul_add(u as Real / 8., *du.start()),
                            (dv.end() - dv.start()).mul_add(v as Real / 2., *dv.start()),
                        )
                        .unwrap();
                    let (u, v) = b.closest_parameters(p, Tolerance::DEFAULT).unwrap();
                    maximum = maximum.max(p.distance_to(b.evaluate(u, v).unwrap()).unwrap());
                }
            }
        }
        println!("{} bidirectional 54-point error {maximum:.17e}", op["id"]);
        if op["id"] == "automatic-spatial-reversed" {
            // Establish that this is the existing sweep model's discrepancy,
            // independently of automatic station discovery and rail reversal.
            let reversed = doc
                .object(rail)
                .unwrap()
                .geometry()
                .curve_ref()
                .unwrap()
                .to_nurbs()
                .unwrap()
                .reversed()
                .unwrap();
            let profiles = op["sections"]
                .as_array()
                .unwrap()
                .iter()
                .zip([-1., -0.5, 0.])
                .map(|(c, parameter)| {
                    let curve = LineSegment::try_new(
                        point(&c["start"]),
                        point(&c["end"]),
                        Tolerance::DEFAULT,
                    )
                    .unwrap()
                    .to_nurbs()
                    .unwrap();
                    SweepSection { parameter, curve }
                })
                .collect::<Vec<_>>();
            let baseline = model(
                CurveRef::NurbsCurve(&reversed),
                &profiles,
                SweepFrameStyle::Freeform,
                SweepBlend::Local,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let baseline = surface(&baseline, false).unwrap();
            let bound = complete_ruled_bound(actual, &baseline, 1e-9);
            println!("existing explicit-model correspondence bound {bound:.17e}");
            assert!(
                maximum > 1e-6 && maximum < 2e-6,
                "update the recorded frame discrepancy if it changes: {maximum}"
            );
        } else if maximum > 1e-6 {
            gaps.push(format!("{}: {maximum:.17e}", op["id"]));
        }
    }
    assert!(gaps.is_empty(), "curved placement differences: {gaps:?}");
}
