//! Headless document-unit API probe, independent of interactive Units commands.
use super::*;
use viboceros_geometry::LengthUnitSystem;

#[cfg(test)]
mod tests {
    use super::*;

    fn compare(actual: &Value, expected: &Value) {
        match (actual, expected) {
            (Value::Number(a), Value::Number(b)) => {
                let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "{a} != {b}");
            }
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    compare(a, b);
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len());
                for (key, b) in b {
                    compare(a.get(key).unwrap(), b);
                }
            }
            _ => assert_eq!(actual, expected),
        }
    }

    #[test]
    fn protocol_matches_retained_rhino_unit_measurements() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/document_units.json"
        ))
        .unwrap();
        let reference: Value = serde_json::from_str(include_str!(
            "../../viboceros-document/src/units/fixtures/rhino8.json"
        ))
        .unwrap();
        let result = run_request(&request).unwrap();
        let expected = reference["results"].as_array().unwrap();
        assert_eq!(result.results.len(), 8);
        assert_eq!(result.results.len(), expected.len());
        for (actual, expected) in result.results.iter().zip(expected) {
            assert_eq!(actual.id, expected["id"]);
            compare(&actual.value, &expected["value"]);
        }
    }

    #[test]
    fn rejects_invalid_unit_codes_and_nonboolean_rescale() {
        assert!(
            run(&Fixture {
                source: 255,
                target: 2,
                rescale: false
            })
            .is_err()
        );
        assert!(
            run(&Fixture {
                source: 2,
                target: 255,
                rescale: true
            })
            .is_err()
        );
        for bad in [json!(1), json!("false"), Value::Null] {
            assert!(
                serde_json::from_value::<Fixture>(json!({"source":2,"target":4,"rescale":bad}))
                    .is_err()
            );
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    source: u32,
    target: u32,
    rescale: bool,
}

fn units(code: u32) -> Result<LengthUnitSystem, ProbeError> {
    match code {
        0 => Ok(LengthUnitSystem::None),
        2 => Ok(LengthUnitSystem::Millimeters),
        4 => Ok(LengthUnitSystem::Meters),
        8 => Ok(LengthUnitSystem::Inches),
        _ => Err(ProbeError::FixtureInvariant("unsupported unit code")),
    }
}

pub(super) fn run(f: &Fixture) -> Result<(Value, u64), ProbeError> {
    let source = units(f.source)?;
    let target = units(f.target)?;
    // These settings are deliberately fixed to match the public-API reference.
    let tolerance = Tolerance::try_new(0.001, 0.0001, 0.00001)?;
    let mut document = Document::new(tolerance);
    document.set_units(source, false)?;
    let mut ids = Vec::new();
    for x in [1000.0, 500.0, 250.0] {
        ids.push(document.add_geometry(Geometry::Point(Point3::try_new(x, 2.0 * x, 3.0 * x)?))?);
    }
    document.select_object(ids[0], SelectionMode::Replace)?;
    document.set_objects_visibility([ids[1]], false)?;
    document.set_objects_locked([ids[2]], true)?;
    let before = snapshot_document(&document);
    document.set_units(target, f.rescale)?;
    let after = snapshot_document(&document);
    Ok((json!({"before": before, "after": after}), 0))
}

fn snapshot_document(document: &Document) -> Value {
    let code = match document.units() {
        LengthUnitSystem::None => 0,
        LengthUnitSystem::Millimeters => 2,
        LengthUnitSystem::Meters => 4,
        LengthUnitSystem::Inches => 8,
        _ => unreachable!("probe only uses four unit systems"),
    };
    let objects = document.objects().map(|object| {
        let Geometry::Point(point) = object.geometry() else { unreachable!("probe only adds points") };
        let mode = if object.attributes().is_locked() { "Locked" }
            else if !object.attributes().is_visible() { "Hidden" } else { "Normal" };
        json!({"point": point.to_array(), "mode": mode, "selected": document.is_selected(object.id())})
    }).collect::<Vec<_>>();
    json!({"units": code, "absolute": document.tolerance().absolute(),
        "relative": document.tolerance().relative(), "angular": document.tolerance().angular(), "objects": objects})
}
