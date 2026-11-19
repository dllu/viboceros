//! Direct sections of expanding, convex line-and-arc profiles.
use super::*;
use viboceros_geometry::{BrepFace, BrepLoop, BrepTrim, ParameterSide, SurfaceIso};
use viboceros_smlib::KernelCurve;

#[cfg(test)]
mod tests;

pub(super) fn produce(
    curve: &NurbsCurve,
    profile: &KernelCurve,
    cutter: &Brep,
    displacement: Vector3,
    draft: f64,
    options: TaperedExtrusionOptions,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    if draft >= 0.
        || curve.degree() > 2
        || !curve.is_closed()?
        || curve.control_points().iter().any(|p| p.weight() <= 0.)
    {
        return Ok(None);
    }
    let direction = displacement.normalized_nonzero()?.as_vector();
    let length = displacement.length()?;
    let slope = (-draft).to_radians().tan();
    let Some(corners) = cutter_corners(&cutter.faces()[0], tolerance)? else {
        return Ok(None);
    };
    let Some(height) = covered_depth(curve, corners, direction, length, slope, tolerance)? else {
        return Ok(None);
    };
    if !convex_profile(curve, direction, length * slope, tolerance)? {
        return Ok(None);
    }
    // The outer parallel curves of a convex G1 profile have the same
    // correspondence at every depth. The selected plane is one section.
    let sweep = produce_curve_shape(
        curve,
        profile,
        direction.scaled(height)?,
        draft,
        options,
        options.solid,
        tolerance,
    )?;
    let result = sweep.to_brep(tolerance)?;
    if options.solid {
        Ok(Some(on_cutter(
            result,
            cutter.faces()[0].surface(),
            direction,
            tolerance,
        )?))
    } else {
        Ok(Some(result))
    }
}

fn cutter_corners(
    face: &BrepFace,
    tolerance: Tolerance,
) -> Result<Option<[Point3; 4]>, GeometryError> {
    let Some([[u0, u1], [v0, v1]]) = face.rectangular_trim_bounds(tolerance)? else {
        return Ok(None);
    };
    let surface = face.surface();
    if surface.try_projective_patch_corners(tolerance).is_err() {
        return Ok(None);
    }
    Ok(Some([
        surface.evaluate(u0, v0)?,
        surface.evaluate(u1, v0)?,
        surface.evaluate(u1, v1)?,
        surface.evaluate(u0, v1)?,
    ]))
}

fn covered_depth(
    curve: &NurbsCurve,
    corners: [Point3; 4],
    direction: Vector3,
    length: f64,
    slope: f64,
    tolerance: Tolerance,
) -> Result<Option<f64>, GeometryError> {
    let base = curve.evaluate(*curve.domain().start())?;
    let depth = |point| base.vector_to(point)?.dot(direction);
    let height = depth(corners[0])?;
    let epsilon = tolerance.absolute();
    if height <= epsilon || height >= length - epsilon {
        return Ok(None);
    }
    for corner in corners {
        if (depth(corner)? - height).abs() > epsilon / 4. {
            return Ok(None);
        }
    }
    for control in curve.control_points() {
        if depth(control.point())?.abs() > epsilon / 4. {
            return Ok(None);
        }
    }
    // Positive weights enclose the profile in its control hull. The selected
    // section is the offset of that profile at the cutter height.
    let outset = height * slope;
    for i in 0..4 {
        let a = corners[i];
        let side = a.vector_to(corners[(i + 1) % 4])?;
        let side_normal = direction.cross(side)?.normalized(tolerance)?;
        let opposite = a.vector_to(corners[(i + 2) % 4])?;
        let inward = if side_normal.as_vector().dot(opposite)? < 0. {
            side_normal.opposite()
        } else {
            side_normal
        };
        for control in curve.control_points() {
            if inward.as_vector().dot(a.vector_to(control.point())?)? <= outset + 4. * epsilon {
                return Ok(None);
            }
        }
    }
    Ok(Some(height))
}

fn convex_profile(
    curve: &NurbsCurve,
    direction: Vector3,
    outset: f64,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let epsilon = tolerance.absolute();
    let angular = tolerance.angular().min(epsilon / outset.max(epsilon));
    let domain = curve.domain();
    let mut end = curve.evaluate(*domain.end())?;
    let mut tangent = curve
        .tangent_at_on_side(*domain.end(), ParameterSide::Left)?
        .as_vector();
    let mut turn = 0_f64;
    // Positive-weight quadratic circle spans turn through less than pi.
    // Continuous tangents and one full turn with a consistent sign certify
    // a convex loop, including the join across the original seam.
    for span in curve.try_bezier_spans()? {
        let domain = span.domain();
        let start = span.evaluate(*domain.start())?;
        let incoming = span
            .tangent_at_on_side(*domain.start(), ParameterSide::Right)?
            .as_vector();
        if start.distance_to(end)? > epsilon || incoming.angle_to(tangent)? > angular {
            return Ok(false);
        }
        end = span.evaluate(*domain.end())?;
        tangent = span
            .tangent_at_on_side(*domain.end(), ParameterSide::Left)?
            .as_vector();
        if span.is_linear_at_zero_tolerance()? {
            if incoming.angle_to(tangent)? > angular {
                return Ok(false);
            }
            continue;
        }
        if span.circular_radius(tolerance)?.is_none() {
            return Ok(false);
        }
        let angle = incoming
            .cross(tangent)?
            .dot(direction)?
            .atan2(incoming.dot(tangent)?);
        if angle == 0. || turn != 0. && angle.signum() != turn.signum() {
            return Ok(false);
        }
        turn += angle;
    }
    Ok((turn.abs() - std::f64::consts::TAU).abs() <= tolerance.angular())
}

fn on_cutter(
    result: Brep,
    cutter: &NurbsSurface,
    direction: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let end = temporary_end(&result, direction, tolerance)?;
    let cap = result
        .faces()
        .iter()
        .position(|face| std::ptr::eq(face.surface(), end))
        .ok_or(REJECTED)?;
    let old = &result.faces()[cap];
    let [boundary] = old.loops() else {
        return Err(REJECTED);
    };
    let normal = |surface: &NurbsSurface| {
        surface.normal_at(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)
    };
    let flip = normal(cutter)?.as_vector().dot(normal(end)?.as_vector())? < 0.;
    let mut trims = Vec::with_capacity(boundary.trims().len());
    for trim in boundary.trims() {
        let edge = trim.edge().ok_or(REJECTED)?;
        let mut uv =
            cutter.try_pullback_projective_curve(result.edges()[edge].curve(), tolerance)?;
        if trim.is_reversed_3d() ^ flip {
            uv = uv.reversed()?;
        }
        let [a, b] = trim.vertices();
        trims.push(BrepTrim::try_new(
            if flip { [b, a] } else { [a, b] },
            Some(edge),
            trim.is_reversed_3d() ^ flip,
            uv,
            trim.trim_type(),
            SurfaceIso::NotIso,
            [0.; 2],
        )?);
    }
    if flip {
        trims.reverse();
    }
    let mut faces = result.faces().to_vec();
    faces[cap] = BrepFace::try_new(
        cutter.clone(),
        old.is_reversed() ^ flip,
        vec![BrepLoop::try_new(boundary.loop_type(), trims)?],
    )?;
    let remapped = Brep::try_new(
        result.vertices().to_vec(),
        result.edges().to_vec(),
        faces,
        tolerance,
    )?;
    if !remapped.is_solid() {
        return Err(REJECTED);
    }
    Ok(remapped)
}
