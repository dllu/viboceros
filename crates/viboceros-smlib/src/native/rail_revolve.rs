//! Rail-guided revolution with the native rational swung-surface tensor.
use super::*;
use viboceros_geometry::{AffineTransform3, Frame3, UnitVector3};

unsafe extern "C" {
    fn vb_curves_swung_surface(
        profile: *const NativeCurve,
        rail: *const NativeCurve,
        scale: f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

fn failure(message: &str) -> Error {
    Error::Kernel(format!("RailRevolve: {message}"))
}

fn local_curve(source: &NurbsCurve, frame: Frame3) -> Result<NurbsCurve, Error> {
    let reference = source
        .control_points()
        .iter()
        .map(|p| p.weight())
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
        .ok_or(Error::InvalidOutput)?;
    if source
        .control_points()
        .iter()
        .any(|p| p.weight() / reference <= 0.)
    {
        return Err(failure("requires weights with a common sign"));
    }
    let points = source
        .control_points()
        .iter()
        .map(|p| {
            Ok(WeightedPoint3::try_new(
                Point3::try_from(frame.coordinates_of(p.point())?)?,
                p.weight() / reference,
            )?)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let local = NurbsCurve::try_new_rational(source.degree(), points, source.knots().to_vec())?
        .try_reparameterized(0. ..=1.)?;
    if source
        .knots()
        .windows(2)
        .zip(local.knots().windows(2))
        .any(|(a, b)| a[0] < a[1] && b[0] >= b[1])
    {
        return Err(failure("parameter normalization loses distinct knots"));
    }
    Ok(local)
}

/// Follow a rail with radial rotation/scaling about a finite directed axis.
/// The rail's start fixes radial scale and phase; ScaleHeight follows its axial
/// variation by scaling profile heights instead of translating them.
pub fn rail_revolved_surface(
    profile: &NurbsCurve,
    rail: &NurbsCurve,
    origin: Point3,
    axis: UnitVector3,
    scale_height: bool,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let (surface, frame) = construct(profile, rail, origin, axis, scale_height, tolerance)?;
    Ok(surface.transformed(placement(frame)?)?)
}

/// Rail revolution with native continuity classification and crease splitting.
pub fn rail_revolved_brep(
    profile: &NurbsCurve,
    rail: &NurbsCurve,
    origin: Point3,
    axis: UnitVector3,
    scale_height: bool,
    tolerance: Tolerance,
) -> Result<viboceros_geometry::Brep, Error> {
    let (surface, frame) = construct(profile, rail, origin, axis, scale_height, tolerance)?;
    let source = viboceros_geometry::Brep::try_surface_face(surface, tolerance)?;
    let output = if let Some((shape, _)) =
        Shape::divide_brep_along_creases(&source, true, true, tolerance)?
    {
        shape.to_brep(tolerance)?
    } else {
        source
    };
    Ok(output.transformed(placement(frame)?, tolerance)?)
}

fn construct(
    profile: &NurbsCurve,
    rail: &NurbsCurve,
    origin: Point3,
    axis: UnitVector3,
    scale_height: bool,
    tolerance: Tolerance,
) -> Result<(NurbsSurface, Frame3), Error> {
    if scale_height
        && [profile, rail].into_iter().any(|c| {
            let n = c.control_points().len();
            let p = c.degree();
            c.knots()[1..=p].iter().any(|k| *k != *c.domain().start())
                || c.knots()[n..n + p].iter().any(|k| *k != *c.domain().end())
        })
    {
        return Err(failure(
            "ScaleHeight requires endpoint-clamped profile and rail curves",
        ));
    }
    let np = profile.control_points().len();
    let nr = rail.control_points().len();
    if np.saturating_mul(nr) > 1_000_000 {
        return Err(failure("output exceeds 1,000,000 controls"));
    }
    let start = rail.evaluate(*rail.domain().start())?;
    let delta = origin.vector_to(start)?;
    let h = axis.as_vector().dot(delta)?;
    let d = delta.to_array();
    let a = axis.as_vector().to_array();
    let radial = Vector3::try_from(std::array::from_fn(|i| (-h).mul_add(a[i], d[i])))?;
    let radius = radial.length()?;
    if radius <= tolerance.absolute() {
        return Err(failure("rail start must be away from the axis"));
    }
    let frame = Frame3::try_from_x_and_normal(
        origin,
        radial,
        axis.as_vector(),
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    let p = local_curve(profile, frame)?;
    let r = local_curve(rail, frame)?;
    let height = r.evaluate(0.)?.z();
    // Normalize radial rail coordinates before the native product, avoiding
    // overflowing reciprocal scales for small, otherwise finite models.
    let normalized_rail = NurbsCurve::try_new_rational(
        r.degree(),
        r.control_points()
            .iter()
            .map(|c| {
                let q = c.point();
                WeightedPoint3::try_new(
                    Point3::try_new(q.x() / radius, q.y() / radius, q.z())?,
                    c.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        r.knots().to_vec(),
    )?;
    let native_p = KernelCurve::from_nurbs_for_modeling(&p)?;
    let native_r = KernelCurve::from_nurbs_for_modeling(&normalized_rail)?;
    // Modeling import clamps the active intervals. Complete the tensor using
    // those exact native controls too: unclamped endpoint controls can move
    // even when the control count is unchanged.
    let p = native_p.to_nurbs()?;
    let normalized_rail = native_r.to_nurbs()?;
    let np = p.control_points().len();
    let nr = normalized_rail.control_points().len();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curves_swung_surface(
            native_p.handle.as_ptr(),
            native_r.handle.as_ptr(),
            1.,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let tensor = brep::surface_from_native(pointer)?;
    if tensor.control_point_count_u() != np || tensor.control_point_count_v() != nr {
        return Err(Error::InvalidOutput);
    }
    // NLib supplies the exact tensor knots and product weights. Complete its
    // planar profile/rail map with the remaining complex radial term and axial
    // following, which are separable products in that same native tensor basis.
    let mut controls = Vec::with_capacity(np * nr);
    for (index, c) in tensor.control_points().iter().enumerate() {
        let a = p.control_points()[index % np].point();
        let b = normalized_rail.control_points()[index / np].point();
        let q = c.point();
        // Public Rhino observations put the inclusive fallback boundary at
        // sqrt(machine epsilon), independent of the document tolerance.
        let z = if scale_height && height.abs() > 1.4901161193847656e-8 {
            a.z() * (b.z() / height)
        } else {
            a.z() + b.z() - height
        };
        controls.push(WeightedPoint3::try_new(
            Point3::try_new(q.x() - a.y() * b.y(), q.y() + a.y() * b.x(), z)?,
            c.weight(),
        )?);
    }
    let local = NurbsSurface::try_new_rational(
        tensor.degree_u(),
        tensor.degree_v(),
        np,
        nr,
        controls,
        tensor.knots_u().to_vec(),
        tensor.knots_v().to_vec(),
    )?;
    Ok((
        local.try_reparameterized(profile.domain(), rail.domain())?,
        frame,
    ))
}

fn placement(frame: Frame3) -> Result<AffineTransform3, Error> {
    let zero = Point3::try_new(0., 0., 0.)?;
    let world = Frame3::try_from_directions(
        zero,
        Vector3::try_new(1., 0., 0.)?,
        Vector3::try_new(0., 1., 0.)?,
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    Ok(AffineTransform3::try_frame_mapping(world, frame, [1.; 3])?)
}
