//! Curve-point jets and the normal section of a surface at a picked edge point.
use super::*;
use viboceros_geometry::{UnitVector3, Vector3};

pub(super) fn evaluate(
    (source, source_end): (&Curve3, bool),
    reference: &Endpoint,
    target: CurveRef<'_>,
    parameter: Real,
    options: Options,
) -> Result<CurveMatchContact, CommandError> {
    let point = target.evaluate(parameter)?;
    if options.continuity == CurveBlendContinuity::Position {
        return Ok(CurveMatchContact {
            point,
            tangent: None,
            curvature: None,
        });
    }
    let source = source.as_ref();
    let (source_parameter, source_side) = if source_end {
        (*source.domain().end(), ParameterSide::Left)
    } else {
        (*source.domain().start(), ParameterSide::Right)
    };
    let original = source
        .evaluate_with_tangent_on_side(source_parameter, source_side)?
        .tangent();
    let side = if parameter == *target.domain().end() {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let edge_tangent = target
        .evaluate_with_tangent_on_side(parameter, side)?
        .tangent();
    let (mut tangent, curvature) = if let Some(index) = reference.edge {
        let direction = if options.perpendicular {
            edge_tangent
        } else {
            original
        };
        surface_section(reference, index, point, direction, options)?
    } else {
        (
            edge_tangent,
            (options.continuity == CurveBlendContinuity::Curvature)
                .then(|| target.curvature_vector(parameter))
                .transpose()?,
        )
    };
    let align = if reference.edge.is_some() && options.perpendicular {
        // The matched curve approaches the edge from its original endpoint.
        let approach = source.evaluate(source_parameter)?.vector_to(point)?;
        if approach.length()? <= reference.tolerance.absolute() {
            original.as_vector()
        } else {
            approach.scaled(if source_end { 1.0 } else { -1.0 })?
        }
    } else {
        original.as_vector()
    };
    if tangent.as_vector().dot(align)? < 0.0 {
        tangent = tangent.opposite();
    }
    Ok(CurveMatchContact {
        point,
        tangent: Some(tangent),
        curvature,
    })
}

fn surface_section(
    reference: &Endpoint,
    index: usize,
    point: Point3,
    direction: UnitVector3,
    options: Options,
) -> Result<(UnitVector3, Option<Vector3>), CommandError> {
    let surface = match reference.object.geometry() {
        Geometry::NurbsSurface(surface) => Some(surface),
        Geometry::Brep(brep) => {
            let mut faces = brep.faces().iter().filter(|face| {
                face.loops().iter().any(|boundary| {
                    boundary
                        .trims()
                        .iter()
                        .any(|trim| trim.edge() == Some(index))
                })
            });
            let face = faces.next();
            if faces.next().is_some() {
                return Err(CommandError::Usage(
                    "Choose an edge belonging to a single surface face",
                ));
            }
            face.map(|face| face.surface())
        }
        _ => None,
    }
    .ok_or(CommandError::Usage("Select a surface edge"))?;
    let (u, v) = surface.closest_parameters(point, reference.tolerance)?;
    let jet = surface.evaluate_with_second_derivatives(u, v)?;
    let du = jet.derivative_u;
    let dv = jet.derivative_v;
    let area = du.cross(dv)?;
    let normal = area.normalized_nonzero()?.as_vector();
    let cross = normal.cross(direction.as_vector())?;
    let tangent = if options.perpendicular {
        cross
    } else {
        cross.cross(normal)?
    }
    .normalized_nonzero()?;
    let curvature = if options.continuity == CurveBlendContinuity::Curvature {
        let direction = tangent.as_vector();
        let scale = area.length()?;
        let alpha = direction.cross(dv)?.dot(normal)? / scale;
        let beta = du.cross(direction)?.dot(normal)? / scale;
        let k = jet.derivative_uu.dot(normal)? * alpha * alpha
            + 2.0 * jet.derivative_uv.dot(normal)? * alpha * beta
            + jet.derivative_vv.dot(normal)? * beta * beta;
        Some(normal.scaled(k)?)
    } else {
        None
    };
    Ok((tangent, curvature))
}
