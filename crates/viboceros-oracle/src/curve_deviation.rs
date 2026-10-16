//! Native command/SDK observations versus an independent interval-query backend.
use super::*;
use crate::curve_join_close::CurveInput;
type SubcurveRanges = [Option<[f64; 2]>; 2];
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: [CurveInput; 2],
    pub postselect: bool,
    #[serde(default, deserialize_with = "keep_marks")]
    pub keep_marks: Option<bool>,
    #[serde(default, deserialize_with = "subcurve_ranges")]
    pub subcurves: Option<[Option<[f64; 2]>; 2]>,
}
fn subcurve_ranges<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<SubcurveRanges>, D::Error> {
    SubcurveRanges::deserialize(d).map(Some)
}
fn keep_marks<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    bool::deserialize(d).map(Some)
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if let Some(ranges) = f.subcurves
        && (!f.postselect
            || f.keep_marks.is_none()
            || ranges.iter().flatten().any(|r| {
                r[0] == r[1] || r.iter().any(|t| !t.is_finite() || !(0. ..=1.).contains(t))
            }))
    {
        return Err(ProbeError::FixtureInvariant(
            "SubCrv needs postselection, KeepMarks and valid normalized ranges",
        ));
    }
    if let Some(keep) = f.keep_marks {
        return run_command(f, keep, iterations, tolerance);
    }
    #[cfg(feature = "native-smlib")]
    {
        let a = f.curves[0].geometry()?;
        let b = f.curves[1].geometry()?;
        measure_probe(iterations, || -> Result<Value, ProbeError> {
            let ranges =
                viboceros_smlib::projected_curve_intervals(a.as_ref(), b.as_ref(), tolerance)
                    .map_err(CommandError::from)?;
            let report = ranges
                .map(|ranges| {
                    viboceros_smlib::curve_distance_extrema(
                        a.as_ref(),
                        b.as_ref(),
                        ranges,
                        tolerance,
                    )
                })
                .transpose()
                .map_err(CommandError::from)?
                .flatten();
            let witness = |w: &viboceros_smlib::CurveDistanceWitness| json!({"normalized_parameters":w.normalized_parameters,"parameters":w.parameters,"points":w.points.map(Point3::to_array),"distance":w.distance});
            Ok(
                json!({"backend_only":true,"command_registered":CommandRegistry::with_builtins().recognizes("CrvDeviation"),"intervals":ranges,
                "sdk":{"created":report.is_some(),"maximum":report.as_ref().map(|r|witness(&r.maximum)),"minimum":report.as_ref().map(|r|witness(&r.minimum))}}),
            )
        })
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (f, iterations, tolerance);
        Err(ProbeError::FixtureInvariant(
            "curve distance queries require native-smlib",
        ))
    }
}

fn run_command(
    f: &Fixture,
    keep: bool,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let ids = [
            doc.add_geometry(f.curves[0].geometry()?.into())?,
            doc.add_geometry(f.curves[1].geometry()?.into())?,
        ];
        doc.select_objects_direct(ids, viboceros_document::SelectionMode::Replace)?;
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        let mut input = format!("CrvDeviation KeepMarks={}", if keep { "Yes" } else { "No" });
        if let Some(ranges) = f.subcurves {
            for (id, range) in ids.into_iter().zip(ranges) {
                if let Some([a, b]) = range {
                    let c = doc.object(id).unwrap().geometry().curve_ref().unwrap();
                    input.push_str(&format!(
                        " SubCrv={id},{},{}",
                        c.parameter_at(a)?,
                        c.parameter_at(b)?
                    ));
                }
            }
        }
        let source_images = ids.map(|id| doc.object(id).unwrap().geometry().clone());
        let history = if f.postselect {
            registry.execute_postselected(
                &mut doc,
                &input,
                viboceros_command::CommandContext::default(),
            )?
        } else {
            registry.execute(&mut doc, &input)?
        };
        let view = registry.curve_deviation_view(&doc);
        let witness = |w: &viboceros_geometry::CurveDistanceWitness| json!({"distance":w.distance,"parameters":w.parameters,"points":w.points.map(Point3::to_array)});
        let sdk = json!({"created":view.is_some(),"maximum":view.as_ref().map(|v|witness(&v.extrema.maximum)),"minimum":view.as_ref().map(|v|witness(&v.extrema.minimum))});
        let ends = view.as_ref().map(|v| v.ends.each_ref().map(witness));
        if view.is_some() {
            registry.execute(&mut doc, "CrvDeviation End")?;
        }
        let outputs=doc.objects().filter(|o|!ids.contains(&o.id())).map(|o| {
            let a=o.attributes();let c=a.object_color();
            let mut v=json!({"layer":doc.layer(a.layer_id()).unwrap().name(),"selected":doc.is_selected(o.id()),"name":a.name(),"color_source":if a.color_source()==viboceros_document::ObjectColorSource::Object {"ColorFromObject"} else {"ColorFromLayer"},"color":[c.red,c.green,c.blue]});
            match o.geometry() {
                Geometry::Point(p) => { v["type"]=json!("Point");v["point"]=json!(p.to_array()); },
                Geometry::Line(l) => { v["type"]=json!("LineCurve");v["start"]=json!(l.start().to_array());v["end"]=json!(l.end().to_array()); },
                _ => unreachable!("deviation emits points and lines"),
            }
            v
        }).collect::<Vec<_>>();
        let mut value = json!({"command_registered":true,"backend_only":false,"sdk":sdk,"command_history":history,"ends":ends,"approximate_overlap":view.as_ref().is_some_and(|v|v.approximate_overlap),"inputs_retained":ids.iter().all(|id|doc.object(*id).is_some()),"created_object_count":outputs.len(),"outputs":outputs,"analysis_dismissed":registry.curve_deviation_view(&doc).is_none(),"undo_label":doc.undo_label()});
        if f.subcurves.is_some() {
            value["sdk_scope"] = json!("measured_inputs");
            value["inputs_unchanged"] = json!(
                ids.into_iter()
                    .zip(&source_images)
                    .all(|(id, old)| doc.object(id).is_some_and(|o| o.geometry() == old))
            );
        }
        Ok(value)
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn crv_deviation_subcurve_protocol_validates_ranges_and_executes_owned_parts() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_deviation_subcurves.json"
        ))
        .unwrap();
        let mut rows = Vec::new();
        for op in &request.operations {
            let mut one = request.clone();
            one.operations = vec![op.clone()];
            let response = run_request(&one).unwrap_or_else(|e| panic!("{}: {e}", op.id()));
            rows.extend(response.results);
        }
        let results = rows;
        assert_eq!(results.len(), 9);
        for row in &results {
            assert_eq!(row.value["sdk_scope"], "measured_inputs");
            assert_eq!(row.value["inputs_unchanged"], true);
            assert_eq!(row.value["analysis_dismissed"], true);
            assert_eq!(row.value["sdk"]["created"], true);
        }
        let reverse = &results
            .iter()
            .find(|r| r.id == "reverse-open")
            .unwrap()
            .value;
        assert_eq!(reverse["ends"][0]["parameters"][0], -7.);
        assert_eq!(reverse["ends"][1]["parameters"][0], -3.);
        let mut fixture: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_deviation_subcurves.json"
        ))
        .unwrap();
        for value in [
            Value::Null,
            json!([]),
            json!([true, null]),
            json!([[0., 1.]]),
        ] {
            fixture["operations"][0]["subcurves"] = value;
            assert!(serde_json::from_value::<ProbeRequest>(fixture.clone()).is_err());
        }
    }
    #[test]
    fn crv_deviation_protocol_requires_a_boolean_when_keep_marks_is_present() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_deviation_workflows.json"
        ))
        .unwrap();
        let mut fixture = request["operations"][0].clone();
        for value in [Value::Null, json!(1), json!("Yes"), json!([])] {
            fixture["keep_marks"] = value;
            assert!(serde_json::from_value::<Fixture>(fixture.clone()).is_err());
        }
        for value in [true, false] {
            fixture["keep_marks"] = json!(value);
            assert_eq!(
                serde_json::from_value::<Fixture>(fixture.clone())
                    .unwrap()
                    .keep_marks,
                Some(value)
            );
        }
    }
    #[test]
    fn crv_deviation_command_workflows_retain_native_status_maxima_and_known_differences() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_deviation_workflows.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/curve_deviation_workflows_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 22);
        let mut reports = 0;
        let mut displayed_minima = 0;
        for (a, b) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, b["id"]);
            assert_eq!(a.value["command_registered"], true);
            assert_eq!(a.value["backend_only"], false);
            assert_eq!(a.value["inputs_retained"], true);
            assert_eq!(a.value["analysis_dismissed"], true);
            let (x, y) = (&a.value["sdk"], &b["value"]["sdk"]);
            assert_eq!(x["created"], y["created"], "{}", a.id);
            if x["created"] == true {
                reports += 1;
                assert!(
                    (x["maximum"]["distance"].as_f64().unwrap()
                        - y["maximum"]["distance"].as_f64().unwrap())
                    .abs()
                        <= 1e-6,
                    "{}",
                    a.id
                );
                let history = b["value"]["command_history"].as_str().unwrap();
                let text = history
                    .split("Minimum deviation = ")
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                let error = (x["minimum"]["distance"].as_f64().unwrap() - text).abs();
                if a.id == "keep-curved-offset" {
                    assert!(error > 1e-4);
                } else {
                    assert!(error <= 1e-6, "{}: {error}", a.id);
                    displayed_minima += 1;
                }
            }
        }
        assert_eq!((reports, displayed_minima), (16, 15));
    }
    #[test]
    fn curve_distance_protocol_retains_four_native_metric_disagreements() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_deviation.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/curve_deviation_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        let mut scalar_matches = 0;
        let mut failures = 0;
        assert_eq!(actual.results.len(), 18);
        for (row, reference) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(row.id, reference["id"]);
            assert_eq!(row.value["backend_only"], true);
            let a = &row.value["sdk"];
            let b = &reference["value"]["sdk"];
            if a["created"] != b["created"] {
                assert!(matches!(
                    row.id.as_str(),
                    "crossing-circles" | "separated-circles"
                ));
                failures += 1;
            } else if a["created"] == true {
                assert!(
                    (a["maximum"]["distance"].as_f64().unwrap()
                        - b["maximum"]["distance"].as_f64().unwrap())
                    .abs()
                        <= 1e-6
                );
                if matches!(row.id.as_str(), "curved-offset" | "curved-spatial") {
                    assert!(
                        a["minimum"]["distance"].as_f64().unwrap()
                            < b["minimum"]["distance"].as_f64().unwrap() - 1e-4
                    );
                    failures += 1;
                } else {
                    assert!(
                        (a["minimum"]["distance"].as_f64().unwrap()
                            - b["minimum"]["distance"].as_f64().unwrap())
                        .abs()
                            <= 1e-6
                    );
                    scalar_matches += 1;
                }
            }
        }
        assert_eq!((scalar_matches, failures), (11, 4));
    }
}
