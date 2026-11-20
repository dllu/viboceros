//! Admit one complete, certified transverse intersection.
use super::*;
use crate::{PolyCurve3, WeightedPoint2};

pub(super) fn find<E: From<GeometryError>>(
    surfaces: [&NurbsSurface; 2],
    tolerance: Tolerance,
    intersect: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Tolerance,
    ) -> Result<Vec<SurfaceSurfaceIntersectionProposal>, E>,
) -> Result<Option<[strip::Cut; 2]>, E> {
    let [first, second] = surfaces;
    let v = first.domain_v();
    let degree = first.degree_v();
    if first.knots_v() == second.knots_v() && degree == second.degree_v() {
        let cuts = std::iter::once(*v.start())
            .chain(
                first
                    .knots_v()
                    .chunk_by(|a, b| a == b)
                    .filter(|g| g.len() == degree && v.start() < &g[0] && &g[0] < v.end())
                    .map(|g| g[0]),
            )
            .chain([*v.end()])
            .collect::<Vec<_>>();
        if cuts.len() > 2
            && let Some(trace) = section_trace(surfaces, &cuts, tolerance, intersect)?
        {
            return Ok(Some(trace));
        }
    }
    Ok(complete_trace(
        surfaces,
        intersect(first, second, tolerance)?,
        tolerance,
    )?)
}

/// Intersect each common section span separately so the solver reaches the
/// crease exactly. Each fragment must cross both spans. Recheck the stitched
/// trace against the original supports after reparameterizing and sewing.
fn section_trace<E: From<GeometryError>>(
    surfaces: [&NurbsSurface; 2],
    cuts: &[Real],
    tolerance: Tolerance,
    intersect: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Tolerance,
    ) -> Result<Vec<SurfaceSurfaceIntersectionProposal>, E>,
) -> Result<Option<[strip::Cut; 2]>, E> {
    let [first, second] = surfaces;
    let [mut spatial, mut first_uv, mut second_uv] = [(); 3].map(|_| Vec::new());
    for pair in cuts.windows(2) {
        let interval = pair[0]..=pair[1];
        let a = first.try_trimmed_v(interval.clone())?;
        let b = second.try_trimmed_v(interval)?;
        let Some([left, right]) =
            complete_trace([&a, &b], intersect(&a, &b, tolerance)?, tolerance)?
        else {
            return Ok(None);
        };
        spatial.push(left.spatial);
        first_uv.push(lift(&left.uv)?);
        second_uv.push(lift(&right.uv)?);
    }
    let concatenate =
        |segments| PolyCurve3::try_with_segment_domains(segments, cuts.to_vec())?.to_nurbs();
    let (Ok(spatial), Ok(first_uv), Ok(second_uv)) = (
        concatenate(spatial),
        concatenate(first_uv),
        concatenate(second_uv),
    ) else {
        return Ok(None);
    };
    let Some(first_uv) = transverse_curve(first, project(&first_uv)?, &spatial, tolerance)? else {
        return Ok(None);
    };
    let Some(second_uv) = transverse_curve(second, project(&second_uv)?, &spatial, tolerance)?
    else {
        return Ok(None);
    };
    Ok(Some([
        strip::Cut {
            spatial: spatial.clone(),
            uv: first_uv,
        },
        strip::Cut {
            spatial,
            uv: second_uv,
        },
    ]))
}

fn lift(curve: &NurbsCurve2) -> Result<NurbsCurve, GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(
                Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                p.weight(),
            )
        })
        .collect::<Result<_, _>>()?;
    NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
}

fn project(curve: &NurbsCurve) -> Result<NurbsCurve2, GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|p| {
            WeightedPoint2::try_new(Point2::try_new(p.point().x(), p.point().y())?, p.weight())
        })
        .collect::<Result<_, _>>()?;
    NurbsCurve2::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
}

fn complete_trace(
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
