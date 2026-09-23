//! Offset-compatible representations of flat polycurves.

use crate::{
    Curve3, CurveSegment3, GeometryError, Point3, PolyCurve3, Polyline3, Real, Tolerance,
    parameter::map_parameter,
};

use super::nurbs::linear_nurbs_leaf_proxy;

pub(super) fn offset_proxy(
    curve: &PolyCurve3,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    let mut linear_nurbs = Vec::with_capacity(curve.segments().len());
    for segment in curve.segments() {
        match segment {
            CurveSegment3::Line(_) | CurveSegment3::Polyline(_) => linear_nurbs.push(None),
            CurveSegment3::NurbsCurve(source) => {
                let Some(polyline) = linear_nurbs_leaf_proxy(source, tolerance)? else {
                    return Ok(Curve3::NurbsCurve(curve.to_nurbs()?));
                };
                linear_nurbs.push(Some(polyline));
            }
            CurveSegment3::Arc(_) => return Ok(Curve3::NurbsCurve(curve.to_nurbs()?)),
        }
    }
    let mut vertices = Vec::<Point3>::new();
    let mut parameters = Vec::<Real>::new();
    for (index, segment) in curve.segments().iter().enumerate() {
        let outer = curve.segment_domain(index)?;
        match segment {
            CurveSegment3::Line(line) => {
                if vertices.is_empty() {
                    vertices.push(line.start());
                    parameters.push(*outer.start());
                }
                vertices.push(line.end());
                parameters.push(*outer.end());
            }
            CurveSegment3::Polyline(_) | CurveSegment3::NurbsCurve(_) => {
                let polyline = match segment {
                    CurveSegment3::Polyline(polyline) => polyline,
                    _ => linear_nurbs[index]
                        .as_ref()
                        .expect("validated linear NURBS leaf"),
                };
                if vertices.is_empty() {
                    vertices.push(polyline.vertices()[0]);
                    parameters.push(*outer.start());
                }
                for (&vertex, &parameter) in polyline
                    .vertices()
                    .iter()
                    .zip(polyline.parameters())
                    .skip(1)
                {
                    vertices.push(vertex);
                    parameters.push(map_parameter(parameter, polyline.domain(), outer.clone())?);
                }
            }
            CurveSegment3::Arc(_) => unreachable!("linear-family check"),
        }
    }
    Ok(Curve3::Polyline(Polyline3::try_with_parameters(
        vertices, parameters, tolerance,
    )?))
}
