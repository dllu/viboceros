//! Shared mixed-curve joining and command-closure fixtures.

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_join_close_fixture_executes_kernel_and_document_policies() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_join_close.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 39);
        let joined = response
            .results
            .iter()
            .find(|result| result.id == "join-first-direction-minority-command")
            .unwrap();
        assert_eq!(joined.value["curves"].as_array().unwrap().len(), 2);
        assert_eq!(
            joined.value["curves"][0]["source_index"],
            serde_json::Value::Null
        );
        assert_eq!(joined.value["curves"][1]["source_index"], 1);
        let closed = response
            .results
            .iter()
            .find(|result| result.id == "close-arc-force")
            .unwrap();
        assert_eq!(closed.value["curves"][0]["type"], "arc");
        assert_eq!(closed.value["curves"][0]["closed"], true);
    }
}

use super::{
    NurbsCurveDefinition, ProbeError, nurbs_curve_definition_value, nurbs_curve_from_definition,
    point,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Instant;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{
    CircularArc3, Curve3, CurveJoinOptions, GeometryError, LineSegment, NurbsCurve, PolyCurve3,
    Polyline3, Tolerance, join_curves,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CurveInput {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Arc {
        points: [[f64; 3]; 3],
    },
    Polyline {
        vertices: Vec<[f64; 3]>,
    },
    Nurbs {
        #[serde(flatten)]
        curve: NurbsCurveDefinition,
    },
    Polycurve {
        segments: Vec<CurveInput>,
    },
}

impl CurveInput {
    fn geometry(&self) -> Result<Curve3, GeometryError> {
        Ok(match self {
            Self::Line { start, end } => Curve3::Line(LineSegment::try_new(
                point(*start)?,
                point(*end)?,
                Tolerance::DEFAULT,
            )?),
            Self::Arc { points } => Curve3::Arc(CircularArc3::try_from_three_points(
                point(points[0])?,
                point(points[1])?,
                point(points[2])?,
                Tolerance::DEFAULT,
            )?),
            Self::Polyline { vertices } => Curve3::Polyline(Polyline3::try_new(
                vertices
                    .iter()
                    .copied()
                    .map(point)
                    .collect::<Result<_, _>>()?,
                Tolerance::DEFAULT,
            )?),
            Self::Nurbs { curve } => Curve3::NurbsCurve(nurbs_curve_from_definition(curve)?),
            Self::Polycurve { segments } => {
                let segments = segments
                    .iter()
                    .map(|segment| segment.geometry()?.to_polycurve())
                    .collect::<Result<Vec<_>, _>>()?;
                Curve3::PolyCurve(PolyCurve3::concatenate(&segments)?)
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Join,
    JoinCommand,
    Close,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurveJoinCloseFixture {
    pub action: Action,
    pub curves: Vec<CurveInput>,
    #[serde(default)]
    pub join_tolerance: Option<f64>,
    #[serde(default)]
    pub close_tolerance: Option<f64>,
    #[serde(default)]
    pub preserve_direction: bool,
    #[serde(default = "super::default_true")]
    pub close_wide_gaps_with_line: bool,
}

pub(super) fn run(
    fixture: &CurveJoinCloseFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let inputs = fixture
        .curves
        .iter()
        .map(CurveInput::geometry)
        .collect::<Result<Vec<_>, _>>()?;
    let compute = || -> Result<Value, ProbeError> {
        let result = match fixture.action {
            Action::Join => {
                let curves = join_curves(
                    &inputs,
                    CurveJoinOptions {
                        tolerance: fixture.join_tolerance.unwrap_or(tolerance.absolute()),
                        preserve_direction: fixture.preserve_direction,
                        style: viboceros_geometry::CurveJoinStyle::Batch,
                    },
                    tolerance,
                )?;
                json!(
                    curves
                        .iter()
                        .map(|curve| record(curve.curve(), tolerance))
                        .collect::<Result<Vec<_>, _>>()?
                )
            }
            Action::Close | Action::JoinCommand => command(fixture, &inputs, tolerance)?,
        };
        Ok(result)
    };
    let mut value = compute()?;
    let started = Instant::now();
    for _ in 0..iterations {
        value = compute()?;
    }
    Ok((
        value,
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

fn command(
    fixture: &CurveJoinCloseFixture,
    inputs: &[Curve3],
    tolerance: Tolerance,
) -> Result<Value, ProbeError> {
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    for (index, curve) in inputs.iter().enumerate() {
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name(format!("source-{index}"));
        let id =
            document.add_geometry_with_attributes(Geometry::from(curve.clone()), attributes)?;
        if matches!(fixture.action, Action::JoinCommand) {
            document.add_group(Some(format!("source-{index}")), [id])?;
        }
        ids.push(id);
    }
    if matches!(fixture.action, Action::JoinCommand) {
        document.add_group(Some("shared".into()), ids.iter().copied())?;
    }
    document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    let registry = CommandRegistry::with_builtins();
    let macro_text = match fixture.action {
        Action::JoinCommand => "Join".into(),
        _ => format!(
            "CloseCrv CloseWideGapsWithLine={} Tolerance={}",
            if fixture.close_wide_gaps_with_line {
                "Yes"
            } else {
                "No"
            },
            fixture.close_tolerance.unwrap_or(tolerance.absolute())
        ),
    };
    registry.execute(&mut document, &macro_text)?;
    let mut curves = document
        .objects()
        .map(|object| {
            let curve = object
                .geometry()
                .curve_ref()
                .expect("the command returns curves")
                .to_owned();
            let mut value = record(&curve, tolerance)?;
            if matches!(fixture.action, Action::JoinCommand) {
                value["name"] = json!(object.attributes().name());
                value["source_index"] = json!(ids.iter().position(|id| *id == object.id()));
                let mut groups = document
                    .groups()
                    .filter(|group| group.members().any(|member| member == object.id()))
                    .map(|group| group.name().map(str::to_owned))
                    .collect::<Vec<_>>();
                groups.sort();
                value["groups"] = json!(groups);
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    if matches!(fixture.action, Action::JoinCommand) {
        curves.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    }
    Ok(json!({"succeeded":true,"curves":curves}))
}

fn record(curve: &Curve3, tolerance: Tolerance) -> Result<Value, GeometryError> {
    let tolerance = Tolerance::try_new(
        tolerance.absolute().min(1e-10),
        tolerance.relative().min(1e-12),
        tolerance.angular(),
    )?;
    let mut segments: Vec<NurbsCurve> = Vec::new();
    let kind = match curve {
        Curve3::Line(_) => "line",
        Curve3::Circle(_) | Curve3::Arc(_) => "arc",
        Curve3::Ellipse(_) | Curve3::NurbsCurve(_) => "nurbs",
        Curve3::Polyline(_) => "polyline",
        Curve3::PolyCurve(curve) => {
            for (index, segment) in curve.segments().iter().enumerate() {
                segments.push(segment.try_reparameterized(curve.segment_domain(index)?)?);
            }
            "polycurve"
        }
    };
    if segments.is_empty() {
        segments.push(curve.as_ref().to_nurbs()?);
    }
    Ok(json!({"type":kind,"closed":curve.as_ref().is_closed()?,
        "domain":[*segments[0].domain().start(), *segments.last().unwrap().domain().end()],
        "segments":segments.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),
        "length":curve.as_ref().length(tolerance)?,
    }))
}
