//! Admit one complete, certified transverse intersection.
use super::*;
use crate::WeightedPoint2;

pub(super) fn complete_trace(
    surfaces: [&NurbsSurface; 2],
    proposals: Vec<SurfaceSurfaceIntersectionProposal>,
    tolerance: Tolerance,
) -> Result<Option<[strip::Cut; 2]>, GeometryError> {
    let mut complete = None;
    for proposal in proposals {
        let SurfaceSurfaceIntersectionProposal::Curve {
            mut spatial,
            mut first_uv,
            mut second_uv,
        } = proposal
        else {
            continue;
        };
        let domain = first_uv.domain();
        if first_uv.evaluate(*domain.start())?.y() > first_uv.evaluate(*domain.end())?.y() {
            spatial = spatial.reversed()?;
            first_uv = first_uv.reversed()?;
            second_uv = second_uv.reversed()?;
        }
        let Some(first) = transverse_curve(surfaces[0], first_uv, &spatial, tolerance)? else {
            continue;
        };
        let Some(second) = transverse_curve(surfaces[1], second_uv, &spatial, tolerance)? else {
            continue;
        };
        if complete.is_some() {
            return Ok(None);
        }
        complete = Some([
            strip::Cut {
                spatial: spatial.clone(),
                uv: first,
            },
            strip::Cut {
                spatial,
                uv: second,
            },
        ]);
    }
    Ok(complete)
}

fn transverse_curve(
    surface: &NurbsSurface,
    uv: NurbsCurve2,
    spatial: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve2>, GeometryError> {
    let u = surface.domain_u();
    let v = surface.domain_v();
    let count = uv.control_points().len();
    let degree = uv.degree();
    let knots = uv.knots();
    if knots[0] != knots[degree] || knots[count] != knots[count + degree] {
        return Ok(None);
    }
    let controls = uv
        .control_points()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let y = match i {
                0 => *v.start(),
                i if i == count - 1 => *v.end(),
                _ => c.point().y(),
            };
            WeightedPoint2::try_new(Point2::try_new(c.point().x(), y)?, c.weight())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if controls
        .iter()
        .any(|c| c.weight() <= 0. || !u.contains(&c.point().x()))
        || controls
            .windows(2)
            .any(|pair| pair[0].point().y() > pair[1].point().y())
    {
        return Ok(None);
    }
    let uv = NurbsCurve2::try_new_rational(degree, controls, knots.to_vec())?;
    if surface
        .parameter_curve_deviation_bound(&uv, spatial, tolerance.absolute())?
        .is_none()
    {
        return Ok(None);
    }
    Ok(Some(uv))
}
