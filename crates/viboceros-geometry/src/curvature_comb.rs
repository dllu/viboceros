//! Bounded, one-sided curvature samples; no document or display policy.
use crate::{GeometryError, NurbsCurve, ParameterSide, Point3, Real, Vector3};
use std::ops::Range;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub struct CurvatureHair {
    pub parameter: Real,
    pub point: Point3,
    pub curvature: Vector3,
}

#[derive(Clone, Debug, Default)]
pub struct CurvatureComb {
    pub hairs: Vec<CurvatureHair>,
    /// Separate strips prevent a false connection across a knot or singularity.
    pub strips: Vec<Range<usize>>,
    pub singular_samples: usize,
}

/// Sample each nonzero knot span separately, including both one-sided ends.
/// A zero-speed sample breaks the graph and is counted, never replaced by a
/// made-up zero curvature. Other numerical failures remain errors.
pub fn curvature_comb(
    curve: &NurbsCurve,
    intervals_per_span: usize,
    maximum_samples: usize,
) -> Result<CurvatureComb, GeometryError> {
    let spans = curve.spans().count();
    let required = intervals_per_span
        .checked_add(1)
        .and_then(|n| spans.checked_mul(n))
        .ok_or(GeometryError::CurvatureCombLimit)?;
    if intervals_per_span == 0 || required > maximum_samples {
        return Err(GeometryError::CurvatureCombLimit);
    }
    let mut out = CurvatureComb::default();
    for (a, b) in curve.spans() {
        let mut start = out.hairs.len();
        for i in 0..=intervals_per_span {
            let t = match i {
                0 => a,
                _ if i == intervals_per_span => b,
                _ => crate::parameter::map_parameter(
                    i as Real / intervals_per_span as Real,
                    0. ..=1.,
                    a..=b,
                )?,
            };
            let side = if t == b {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            let (point, first, second) = curve.evaluate_with_second_derivative_on_side(t, side)?;
            if first.length()? == 0. {
                if start < out.hairs.len() {
                    out.strips.push(start..out.hairs.len());
                }
                out.singular_samples += 1;
                start = out.hairs.len();
                continue;
            }
            out.hairs.push(CurvatureHair {
                parameter: t,
                point,
                curvature: crate::curve::curvature_from_derivatives(first, second)?,
            });
        }
        if start < out.hairs.len() {
            out.strips.push(start..out.hairs.len());
        }
    }
    Ok(out)
}
