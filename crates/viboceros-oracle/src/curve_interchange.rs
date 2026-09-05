//! Cross-reader validation: Rhino reads the file written by this probe.

use super::{OracleTemporaryFile, ProbeError, curve_join_close::CurveInput};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{Curve3, CurveRef, Tolerance};
use viboceros_io::{
    ThreeDmColorSource, ThreeDmGeometry, ThreeDmGroup, ThreeDmLayer, ThreeDmModel, ThreeDmObject,
    read_3dm_file, write_3dm_file,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurveInterchangeFixture {
    pub curve: CurveInput,
    /// Filled by OracleClient.compare in a private temporary directory. Native
    /// standalone runs use an automatically removed file instead.
    #[serde(default)]
    pub artifact_path: Option<String>,
    #[serde(default)]
    pub segment_parameters: Option<Vec<f64>>,
}

pub(super) fn run(
    fixture: &CurveInterchangeFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let mut source = fixture.curve.geometry()?;
    if let Some(parameters) = &fixture.segment_parameters {
        let Curve3::PolyCurve(curve) = source else {
            return Err(ProbeError::FixtureInvariant(
                "segment parameters require polycurve",
            ));
        };
        source = Curve3::PolyCurve(viboceros_geometry::PolyCurve3::try_with_segment_domains(
            curve.segments().to_vec(),
            parameters.clone(),
        )?);
    }
    let geometry = match source {
        Curve3::Line(c) => ThreeDmGeometry::Line(c),
        Curve3::Arc(c) => ThreeDmGeometry::Arc(c),
        Curve3::Polyline(c) => ThreeDmGeometry::Polyline(c),
        Curve3::NurbsCurve(c) => ThreeDmGeometry::NurbsCurve(c),
        Curve3::PolyCurve(c) => ThreeDmGeometry::PolyCurve(c),
        Curve3::Circle(c) => ThreeDmGeometry::NurbsCurve(c.to_nurbs()?),
        Curve3::Ellipse(c) => ThreeDmGeometry::NurbsCurve(c.to_nurbs()?),
    };
    // Test all four combinations: mode and visibility are independently stored.
    let objects = [(true, false), (false, false), (true, true), (false, true)]
        .into_iter()
        .enumerate()
        .map(|(i, (visible, locked))| {
            let mut object = ThreeDmObject::new(geometry.clone(), 0);
            object.name = Some(format!("Curve {i}"));
            object.visible = visible;
            object.locked = locked;
            object.object_color = [13, 71, 201];
            object.color_source = ThreeDmColorSource::Object;
            object.wire_density = -1;
            object.group_indices = vec![2, 0];
            object
        })
        .collect();
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Geometry".into(),
            color: [30, 60, 90],
            visible: true,
            locked: false,
        }],
        ["Outer", "Empty", "Overlap"]
            .map(|name| ThreeDmGroup { name: name.into() })
            .to_vec(),
        objects,
    );
    let temporary = OracleTemporaryFile::new("curve-interchange");
    let path = fixture
        .artifact_path
        .as_deref()
        .map(Path::new)
        .unwrap_or(&temporary.path);
    // Never overwrite a caller's existing artifact. The client assigns a fresh
    // unique path per operation, not one based on an untrusted operation id.
    let reservation = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    drop(reservation);
    write_3dm_file(path, &model)?;
    // Timing compares readers only, not Viboceros export against Rhino import.
    let mut value = record(path, tolerance)?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        value = std::hint::black_box(record(path, tolerance)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed))
}

fn record(path: &Path, tolerance: Tolerance) -> Result<Value, ProbeError> {
    let model = read_3dm_file(path, tolerance)?;
    if model.unsupported_object_count() != 0 {
        return Err(ProbeError::FixtureInvariant(
            "unsupported interchange object",
        ));
    }
    let objects = model
        .objects
        .iter()
        .map(|object| {
            let curve = match &object.geometry {
                ThreeDmGeometry::Line(c) => CurveRef::Line(c),
                ThreeDmGeometry::Arc(c) => CurveRef::Arc(c),
                ThreeDmGeometry::Polyline(c) => CurveRef::Polyline(c),
                ThreeDmGeometry::NurbsCurve(c) => CurveRef::NurbsCurve(c),
                ThreeDmGeometry::PolyCurve(c) => CurveRef::PolyCurve(c),
                _ => return Err(ProbeError::FixtureInvariant("expected curve")),
            };
            Ok(json!({
                "name": object.name, "visible": object.visible, "locked": object.locked,
                "color": object.object_color, "color_source": object.color_source as u32,
                "wire_density": object.wire_density, "groups": object.group_indices,
                "layer": object.layer_index, "curve": curve_record(curve)?,
            }))
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    Ok(json!({
        "groups": model.groups.iter().map(|g| &g.name).collect::<Vec<_>>(),
        "layers": model.layers.iter().map(|l| json!({
            "name": l.name, "color": l.color, "visible": l.visible, "locked": l.locked,
        })).collect::<Vec<_>>(),
        "objects": objects,
    }))
}

fn curve_record(curve: CurveRef<'_>) -> Result<Value, ProbeError> {
    let kind = match curve {
        CurveRef::Line(_) => "line",
        CurveRef::Arc(_) => "arc",
        CurveRef::Polyline(_) => "polyline",
        CurveRef::NurbsCurve(_) => "nurbs",
        CurveRef::PolyCurve(_) => "polycurve",
        _ => return Err(ProbeError::FixtureInvariant("unexpected file curve type")),
    };
    let samples = (0..=32)
        .map(|i| {
            Ok(curve
                .evaluate(curve.parameter_at(i as f64 / 32.0)?)?
                .to_array())
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let mut value = json!({
        "type": kind, "domain": [*curve.domain().start(), *curve.domain().end()],
        "closed": curve.is_closed()?, "samples": samples,
    });
    if let CurveRef::NurbsCurve(c) = curve {
        value["definition"] = super::nurbs_curve_definition_value(c);
    }
    if let CurveRef::PolyCurve(c) = curve {
        value["parameters"] = json!(c.parameters());
        value["segments"] = json!(
            c.segments()
                .iter()
                .map(|s| curve_record(s.as_ref()))
                .collect::<Result<Vec<_>, _>>()?
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    #[test]
    fn explicitly_supplied_artifact_paths_never_overwrite_existing_files() {
        let path = crate::OracleTemporaryFile::new("protected-artifact");
        std::fs::write(&path.path, b"existing artifact").unwrap();
        let fixture = super::CurveInterchangeFixture {
            curve: super::CurveInput::Line {
                start: [0.0; 3],
                end: [1.0; 3],
            },
            artifact_path: Some(path.path.to_str().unwrap().to_owned()),
            segment_parameters: None,
        };
        assert!(
            matches!(super::run(&fixture, 1, viboceros_geometry::Tolerance::DEFAULT),
            Err(crate::ProbeError::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists)
        );
        assert_eq!(std::fs::read(&path.path).unwrap(), b"existing artifact");
    }

    #[test]
    fn permanent_interchange_fixture_checks_structure_and_independent_attributes() {
        let request = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_3dm_interchange.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 6);
        for (result, count) in response.results.iter().zip([4, 4, 8, 8, 4, 4]) {
            let objects = result.value["objects"].as_array().unwrap();
            assert_eq!(objects.len(), count, "{}", result.id);
            for object in objects {
                let name = object["name"].as_str().unwrap();
                let index: usize = name.strip_prefix("Curve ").unwrap().parse().unwrap();
                assert_eq!(object["visible"], index.is_multiple_of(2));
                assert_eq!(object["locked"], index >= 2);
                assert_eq!(object["groups"], serde_json::json!([2, 0]));
                assert_eq!(object["color"], serde_json::json!([13, 71, 201]));
            }
        }
        let mixed = &response.results[4].value["objects"][0]["curve"];
        assert_eq!(
            mixed["parameters"],
            serde_json::json!([-7.0, 0.0, 2.0, 4.0, 8.0, 15.0])
        );
        let kinds: Vec<_> = mixed["segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["type"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["line", "nurbs", "nurbs", "arc", "polyline"]);
    }
}
