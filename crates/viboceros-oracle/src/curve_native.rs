//! Native curve parameter, derivative, division, and transformation contracts.

use super::{ProbeError, curve_join_close::CurveInput, nurbs_curve_definition_value};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_document::Geometry;
use viboceros_geometry::{AffineTransform3, Curve3, CurveRef, Tolerance, Vector3};

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_parameter_correspondence_fixture_checks_both_maps() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_parameter_map.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 38);
        for result in response.results {
            assert_eq!(result.value["parameter_map"].as_array().unwrap().len(), 65);
        }
    }

    #[test]
    fn permanent_native_cutting_fixture_checks_command_output_representations() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_native_cutting.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 34);
        for result in response.results {
            let objects = result.value["objects"].as_array().unwrap();
            assert!(!objects.is_empty());
            for object in objects {
                assert_eq!(object["native"]["points"].as_array().unwrap().len(), 17);
                assert_eq!(object["native"]["domain"], object["curve"]["domain"]);
                assert_eq!(object["attributes_match_source"], true);
                assert_eq!(object["in_source_group"], true);
            }
        }
    }

    #[test]
    fn permanent_native_extrusion_fixture_preserves_profile_parameterization() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_native_extrusion.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 3);
        assert_eq!(
            response.results[0].value["surfaces"][0]["knots_u"],
            serde_json::json!([0.0, 0.0, 1.0, 2.0, 2.0])
        );
        assert_eq!(
            response.results[1].value["surfaces"][0]["knots_u"],
            serde_json::json!([-7.0, -7.0, 3.0, 13.0, 13.0])
        );
    }

    #[test]
    fn permanent_native_parameters_fixture_checks_every_curve_family() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_native_parameters.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 38);
        for result in response.results {
            assert_eq!(result.value["samples"].as_array().unwrap().len(), 33);
            assert_eq!(result.value["divisions"].as_array().unwrap().len(), 18);
            assert_eq!(result.value["domain"], result.value["nurbs"]["domain"]);
        }
    }

    #[test]
    fn permanent_native_editing_fixture_keeps_parameterized_outputs() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_native_editing.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 86);
        for result in response.results {
            let curves = result.value["curves"].as_array().unwrap();
            assert!(!curves.is_empty());
            for curve in curves {
                assert_eq!(curve["samples"].as_array().unwrap().len(), 33);
                assert_eq!(curve["divisions"].as_array().unwrap().len(), 18);
                assert_eq!(curve["domain"], curve["nurbs"]["domain"]);
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NativeCurveFixture {
    pub curve: CurveInput,
    #[serde(default)]
    pub domain: Option<[f64; 2]>,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub transform: Option<[[f64; 4]; 3]>,
    #[serde(default)]
    pub edit: Option<NativeCurveEdit>,
    #[serde(default)]
    pub parameter_map: bool,
}

pub(super) fn run(
    fixture: &NativeCurveFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = fixture.curve.geometry()?;
    let compute = || -> Result<Value, ProbeError> {
        let mut curve = source.clone();
        if let Some([a, b]) = fixture.domain {
            curve = if let Curve3::PolyCurve(c) = curve {
                Curve3::PolyCurve(c.try_reparameterized_by_length(a..=b, tolerance)?)
            } else {
                curve.try_reparameterized(a..=b)?
            };
        }
        if fixture.reversed {
            curve = curve.reversed(tolerance)?;
        }
        if let Some(rows) = fixture.transform {
            let geometry = Geometry::from(curve).transformed(
                AffineTransform3::try_new(
                    rows.map(|r| [r[0], r[1], r[2]]),
                    Vector3::try_new(rows[0][3], rows[1][3], rows[2][3])?,
                )?,
                tolerance,
            )?;
            curve = geometry
                .curve_ref()
                .ok_or(ProbeError::FixtureInvariant(
                    "transformed source is not a curve",
                ))?
                .to_owned();
        }
        if let Some(edit) = &fixture.edit {
            let curves = match edit {
                NativeCurveEdit::Trim { domain: [a, b] } => vec![curve.try_trimmed(*a..=*b)?],
                NativeCurveEdit::Subcurve { domain: [a, b] } => vec![curve.try_subcurve(*a, *b)?],
                NativeCurveEdit::Split { parameters } => {
                    curve.try_split_at_parameters(parameters)?
                }
                NativeCurveEdit::Seam { parameter } => {
                    vec![curve.try_change_closed_seam(*parameter)?]
                }
            };
            let records = curves
                .iter()
                .map(|curve| {
                    let mut value = record(curve.as_ref(), tolerance)?;
                    value["type"] = json!(match curve {
                        Curve3::Line(_) => "line",
                        Curve3::Circle(_) | Curve3::Arc(_) => "arc",
                        Curve3::Polyline(_) => "polyline",
                        Curve3::PolyCurve(_) => "polycurve",
                        _ => "nurbs",
                    });
                    Ok(value)
                })
                .collect::<Result<Vec<_>, ProbeError>>()?;
            Ok(json!({"curves": records}))
        } else {
            let mut value = record(curve.as_ref(), tolerance)?;
            if fixture.parameter_map {
                value["parameter_map"] = Value::Array(
                    (0..=64)
                        .map(|i| {
                            let view = curve.as_ref();
                            let t = view.parameter_at(i as f64 / 64.0)?;
                            Ok(json!({
                                "parameter": t,
                                "nurbs": view.nurbs_parameter(t)?,
                                "native": view.parameter_from_nurbs(t)?,
                            }))
                        })
                        .collect::<Result<Vec<_>, ProbeError>>()?,
                );
            }
            Ok(value)
        }
    };
    let mut value = std::hint::black_box(compute()?);
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        value = std::hint::black_box(compute()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((value, elapsed))
}

/// Native command output samples catch an accidental parameterization change
/// even when two rational definitions describe the same geometric locus.
pub(super) fn command_record(
    view: CurveRef<'_>,
) -> Result<Value, viboceros_geometry::GeometryError> {
    let points = (0..=16)
        .map(|i| {
            view.evaluate(view.parameter_at(i as f64 / 16.0)?)
                .map(|point| point.to_array())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "type": match view {
            CurveRef::Line(_) => "line",
            CurveRef::Circle(_) | CurveRef::Arc(_) => "arc",
            CurveRef::Polyline(_) => "polyline",
            CurveRef::PolyCurve(_) => "polycurve",
            _ => "nurbs",
        },
        "domain": [*view.domain().start(), *view.domain().end()],
        "points": points,
    }))
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CutSource {
    Native {
        native: CurveInput,
        #[serde(default)]
        domain: Option<[f64; 2]>,
        #[serde(default)]
        reversed: bool,
    },
    Nurbs(super::NurbsCurveDefinition),
}

impl From<super::NurbsCurveDefinition> for CutSource {
    fn from(value: super::NurbsCurveDefinition) -> Self {
        Self::Nurbs(value)
    }
}

impl CutSource {
    pub(super) fn geometry(
        &self,
        tolerance: Tolerance,
    ) -> Result<Geometry, viboceros_geometry::GeometryError> {
        match self {
            Self::Nurbs(definition) => Ok(Geometry::NurbsCurve(
                super::nurbs_curve_from_definition(definition)?,
            )),
            Self::Native {
                native,
                domain,
                reversed,
            } => {
                let mut curve = native.geometry()?;
                if let Some([a, b]) = domain {
                    curve = match curve {
                        Curve3::PolyCurve(c) => {
                            Curve3::PolyCurve(c.try_reparameterized_by_length(*a..=*b, tolerance)?)
                        }
                        c => c.try_reparameterized(*a..=*b)?,
                    };
                }
                if *reversed {
                    curve = curve.reversed(tolerance)?;
                }
                Ok(Geometry::from(curve))
            }
        }
    }
}

pub(super) fn extrude_command(
    definition: &CutSource,
    distance: f64,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = definition.geometry(tolerance)?;
    let run = || -> Result<Value, ProbeError> {
        let mut document = viboceros_document::Document::new(tolerance);
        let source_id = document.add_geometry(source.clone())?;
        document.select_objects_direct([source_id], viboceros_document::SelectionMode::Replace)?;
        viboceros_command::CommandRegistry::with_builtins().execute(
            &mut document,
            &format!("ExtrudeCrv {distance} Output=Surface Solid=No"),
        )?;
        let surfaces = document
            .objects()
            .filter(|object| object.id() != source_id)
            .map(|object| {
                let Geometry::NurbsSurface(surface) = object.geometry() else {
                    return Err(ProbeError::FixtureInvariant(
                        "open extrusion must produce a surface",
                    ));
                };
                Ok(super::nurbs_surface_definition_value(surface))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({"surfaces": surfaces}))
    };
    let mut value = run()?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        value = std::hint::black_box(run()?);
    }
    Ok((
        value,
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeCurveEdit {
    Trim { domain: [f64; 2] },
    Subcurve { domain: [f64; 2] },
    Split { parameters: Vec<f64> },
    Seam { parameter: f64 },
}

fn record(view: CurveRef<'_>, tolerance: Tolerance) -> Result<Value, ProbeError> {
    let mut samples = Vec::new();
    for i in 0..=32 {
        let t = view.parameter_at(i as f64 / 32.0)?;
        let (p, d, dd) = view.evaluate_with_second_derivative(t)?;
        samples.push(json!({
            "parameter": t,
            "point": p.to_array(),
            "first": d.to_array(),
            "second": dd.to_array(),
            "tangent": view.evaluate_with_tangent(t)?.tangent().as_vector().to_array(),
        }));
    }
    let divisions = view
        .divide_by_count_samples(17, true, tolerance)?
        .iter()
        .map(|sample| {
            json!({
                "parameter": sample.parameter(),
                "point": sample.point().to_array(),
                "tangent": sample.tangent().as_vector().to_array(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "domain": [*view.domain().start(), *view.domain().end()],
        "closed": view.is_closed()?,
        "length": view.length(tolerance)?,
        "samples": samples,
        "divisions": divisions,
        "nurbs": nurbs_curve_definition_value(&view.to_nurbs()?),
    }))
}
