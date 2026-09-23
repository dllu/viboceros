//! Offset-compatible representations of flat polycurves.

use crate::{
    Curve3, CurveSegment3, GeometryError, Point3, PolyCurve3, Polyline3, Real, Tolerance,
    parameter::map_parameter,
};

pub(super) fn offset_proxy(
    curve: &PolyCurve3,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    if !curve
        .segments()
        .iter()
        .all(|segment| matches!(segment, CurveSegment3::Line(_) | CurveSegment3::Polyline(_)))
    {
        return Ok(Curve3::NurbsCurve(curve.to_nurbs()?));
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
            CurveSegment3::Polyline(polyline) => {
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
            _ => unreachable!("linear-family check"),
        }
    }
    Ok(Curve3::Polyline(Polyline3::try_with_parameters(
        vertices, parameters, tolerance,
    )?))
}
