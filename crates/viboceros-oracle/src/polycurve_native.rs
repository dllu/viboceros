//! Native analytic polycurve evaluation, editing, and representation checks.

use super::{ProbeError, curve_join_close::CurveInput, nurbs_curve_definition_value};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    AffineTransform3, CurveEvaluationSide, CurveSegment3, GeometryError, PolyCurve3, Tolerance,
    Vector3,
};

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_native_curve_fixtures_check_geometry_editing_and_document_outputs() {
        for (fixture, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/polycurve_native.json"),
                12,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/polycurve_analytic_editing.json"
                ),
                8,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/polycurve_native_document.json"),
                2,
            ),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(fixture).unwrap();
            let response = crate::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NativePolyCurveFixture {
    pub curve: CurveInput,
    #[serde(default)]
    pub deformable: bool,
    #[serde(default)]
    pub document_checks: bool,
    #[serde(default)]
    pub domain: Option<[f64; 2]>,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub trim: Option<[f64; 2]>,
    #[serde(default)]
    pub split: Option<f64>,
    #[serde(default)]
    pub transform: Option<[[f64; 4]; 3]>,
}

pub(super) fn run(
    fixture: &NativePolyCurveFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = fixture.curve.geometry()?.to_polycurve()?;
    let compute = || -> Result<Value, ProbeError> {
        let mut curve = source.clone();
        if let Some([a, b]) = fixture.domain {
            curve = curve.try_reparameterized_by_length(a..=b, tolerance)?;
        }
        if fixture.reversed {
            curve = curve.reversed()?;
        }
        if let Some([a, b]) = fixture.trim {
            curve = curve.try_trimmed(a..=b)?;
        }
        if fixture.deformable {
            curve = curve.try_deformable()?;
        }
        if let Some(rows) = fixture.transform {
            curve = curve.transformed(AffineTransform3::try_new(
                rows.map(|r| [r[0], r[1], r[2]]),
                Vector3::try_new(rows[0][3], rows[1][3], rows[2][3])?,
            )?)?;
        }
        let curves = if let Some(t) = fixture.split {
            let (a, b) = curve.try_split(t)?;
            vec![a, b]
        } else {
            vec![curve]
        };
        Ok(json!({"curves":curves.iter().map(|c| {
                let mut value=record(c,tolerance)?;
                if fixture.document_checks { value["document"]=super::polycurve_document::record(c,tolerance)?; }
                Ok(value)
            }).collect::<Result<Vec<_>,ProbeError>>()?}))
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

pub(super) fn record(curve: &PolyCurve3, tolerance: Tolerance) -> Result<Value, GeometryError> {
    let mut segments = Vec::new();
    for (index, segment) in curve.segments().iter().enumerate() {
        let domain = curve.segment_domain(index)?;
        let mut samples = Vec::new();
        for fraction in [0.0, 0.125, 0.375, 0.5, 0.875, 1.0] {
            let parameter = if fraction == 1.0 {
                *domain.end()
            } else {
                domain.start() + (domain.end() - domain.start()) * fraction
            };
            let side = if fraction == 1.0 {
                CurveEvaluationSide::Left
            } else {
                CurveEvaluationSide::Right
            };
            let (point, first, second) = curve.evaluate_with_second_derivative(parameter, side)?;
            samples.push(json!({"parameter":parameter,"point":point.to_array(),"first":first.to_array(),"second":second.to_array()}));
        }
        let kind = match segment {
            CurveSegment3::Line(_) => "line",
            CurveSegment3::Arc(_) => "arc",
            CurveSegment3::Polyline(_) => "polyline",
            CurveSegment3::NurbsCurve(_) => "nurbs",
        };
        segments.push(json!({"type":kind,"domain":[*domain.start(),*domain.end()],"samples":samples,
            "nurbs":nurbs_curve_definition_value(&segment.try_reparameterized(domain)?.to_nurbs()?)}));
    }
    Ok(
        json!({"domain":[*curve.domain().start(),*curve.domain().end()],"closed":curve.is_closed()?,"length":curve.length(tolerance)?,"segments":segments}),
    )
}
