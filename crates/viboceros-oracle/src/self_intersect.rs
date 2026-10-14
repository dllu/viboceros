//! Command and optional native paired-parameter self-contact observations.
use super::*;
use crate::curve_join_close::CurveInput;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    #[serde(default)]
    vertices: Option<Vec<[f64; 3]>>,
    #[serde(default)]
    curve: Option<CurveInput>,
    #[serde(default)]
    include_selection: bool,
    #[serde(default)]
    include_api: bool,
}

pub fn run(
    iterations: u32,
    fixture: &Fixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = match (&fixture.curve, &fixture.vertices) {
        (Some(curve), None) => Geometry::from(curve.geometry()?),
        (None, Some(vertices)) => Geometry::Polyline(Polyline3::try_new(
            vertices
                .iter()
                .copied()
                .map(point)
                .collect::<Result<Vec<_>, _>>()?,
            tolerance,
        )?),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "self-intersection requires one curve or vertex list",
            ));
        }
    };
    let api_events = if fixture.include_api {
        Some(api_events(&source, tolerance)?)
    } else {
        None
    };
    measure_probe(iterations, || -> Result<_, ProbeError> {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(tolerance);
        let source_id = document.add_geometry(source.clone())?;
        let selected = if fixture.include_selection {
            registry.execute(&mut document, "SelSelfIntersectingCrv")?;
            let selected = document.is_selected(source_id);
            document.clear_selection();
            Some(selected)
        } else {
            None
        };
        document.select_objects_direct([source_id], SelectionMode::Replace)?;
        let report = registry.execute(&mut document, "IntersectSelf")?;
        let mut output = document.objects().filter(|object| object.id()!=source_id).map(|object| {
            match object.geometry() {
                Geometry::Point(p) => Ok(json!({"type":"point","point":p.to_array()})),
                geometry => {
                    let curve = geometry.curve_ref().ok_or(ProbeError::FixtureInvariant("unexpected self-intersection output"))?;
                    Ok(json!({"type":"curve","start":curve.start_point()?.to_array(),"end":curve.end_point()?.to_array()}))
                }
            }
        }).collect::<Result<Vec<_>, ProbeError>>()?;
        output.sort_by_key(|item| {
            (
                item["type"].to_string(),
                item.get("start").unwrap_or(&item["point"]).to_string(),
                item["end"].to_string(),
            )
        });
        let mut value = json!({"report":report,"output":output,"source_selected":document.is_selected(source_id),
            "selected_output_count":document.selected_objects().filter(|object| object.id()!=source_id).count()});
        if let Some(selected) = selected {
            value["selected_as_self_intersecting"] = json!(selected);
        }
        if let Some(events) = &api_events {
            value["api_events"] = events.clone();
        }
        Ok(value)
    })
}

#[cfg(feature = "native-smlib")]
fn api_events(source: &Geometry, tolerance: Tolerance) -> Result<Value, ProbeError> {
    use viboceros_geometry::CurveCurveIntersectionEvent;
    let curve = source
        .nurbs_curve_representation()?
        .ok_or(ProbeError::FixtureInvariant(
            "self-intersection requires a curve",
        ))?;
    let events = viboceros_smlib::curve_self_intersection_events(&curve, tolerance)
        .map_err(CommandError::from)?
        .ok_or(ProbeError::FixtureInvariant(
            "self-intersection input outside native API limits",
        ))?;
    Ok(Value::Array(events.into_iter().map(|event|match event {
        CurveCurveIntersectionEvent::Point(p)=>json!({"type":"point","a":p.first_parameter(),"b":p.second_parameter(),"point":p.point().to_array()}),
        CurveCurveIntersectionEvent::Overlap(o)=>json!({"type":"overlap","a":[o.start().first_parameter(),o.end().first_parameter()],
            "b":[o.start().second_parameter(),o.end().second_parameter()],"points":[o.start().point().to_array(),o.end().point().to_array()]}),
    }).collect()))
}
#[cfg(not(feature = "native-smlib"))]
fn api_events(_: &Geometry, _: Tolerance) -> Result<Value, ProbeError> {
    Err(ProbeError::FixtureInvariant(
        "self-intersection paired-parameter API requires native-smlib",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_basic_crossings_seams_and_selection_are_replayed_through_real_commands() {
        for name in ["self_intersect", "self_intersect_selection"] {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
            let mut request: ProbeRequest = serde_json::from_slice(
                &fs::read(root.join("fixtures").join(format!("{name}.json"))).unwrap(),
            )
            .unwrap();
            // Nested collinear overlaps have a documented prior parity gap;
            // the standalone qualification retains every such raw diagnostic.
            request.operations.retain(|op| {
                matches!(
                    op.id(),
                    "crossing" | "simple-closed" | "backtrack" | "cubic-crossing" | "cubic-seam"
                )
            });
            // API event diagnostics are separate from command/selection replay.
            for op in &mut request.operations {
                if let Operation::SelfIntersectProbe { fixture, .. } = op {
                    fixture.include_api = false;
                }
            }
            let expected: ProbeResponse = serde_json::from_slice(
                &fs::read(root.join("observations").join(format!("{name}.json"))).unwrap(),
            )
            .unwrap();
            let actual = run_request(&request).unwrap();
            assert_eq!(actual.results.len(), 5);
            for a in actual.results {
                let b = expected.results.iter().find(|b| b.id == a.id).unwrap();
                assert_eq!(a.id, b.id);
                assert_eq!(a.value["report"], b.value["report"]);
                assert_eq!(a.value["source_selected"], b.value["source_selected"]);
                assert_eq!(
                    a.value["selected_output_count"],
                    b.value["selected_output_count"]
                );
                assert_eq!(
                    a.value["selected_as_self_intersecting"],
                    b.value["selected_as_self_intersecting"]
                );
                let left = a.value["output"].as_array().unwrap();
                let right = b.value["output"].as_array().unwrap();
                assert_eq!(left.len(), right.len(), "{}", a.id);
                for (a, b) in left.iter().zip(right) {
                    assert_eq!(a["type"], b["type"]);
                    for key in ["point", "start", "end"] {
                        if let Some(coords) = a[key].as_array() {
                            for (axis, c) in coords.iter().enumerate() {
                                assert!(
                                    (c.as_f64().unwrap() - b[key][axis].as_f64().unwrap()).abs()
                                        < 1e-8
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
