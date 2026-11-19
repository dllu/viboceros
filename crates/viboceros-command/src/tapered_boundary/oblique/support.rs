//! Cover the swept stock with an auxiliary cutter on the same carrier.
use super::*;

pub(super) fn planar(
    stock: &Brep,
    plane: Plane,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let chart = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let rise = displacement.dot(plane.normal().as_vector())?;
    if rise.abs() <= tolerance.absolute() {
        return Err(REJECTED);
    }
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    // Affine projection of a positive-weight control hull covers the stock.
    for control in stock
        .faces()
        .iter()
        .flat_map(|face| face.surface().control_points())
    {
        if control.weight() <= 0. {
            return Err(REJECTED);
        }
        let point = control.point();
        let fraction = -plane.signed_distance_to(point)? / rise;
        let projected = point.translated(displacement.scaled(fraction)?)?;
        let xy = chart.projected_coordinates_of(projected)?;
        for axis in 0..2 {
            min[axis] = min[axis].min(xy[axis]);
            max[axis] = max[axis].max(xy[axis]);
        }
    }
    let pad = displacement.length()?.max(tolerance.absolute() * 16.);
    let [left, bottom] = min.map(|v| v - pad);
    let [right, top] = max.map(|v| v + pad);
    let surface = NurbsSurface::try_bilinear([
        chart.point_at([left, bottom, 0.])?,
        chart.point_at([right, bottom, 0.])?,
        chart.point_at([right, top, 0.])?,
        chart.point_at([left, top, 0.])?,
    ])?;
    Ok(Brep::try_surface_face(surface, tolerance)?)
}

pub(super) fn warped(
    stock: &Brep,
    surface: &NurbsSurface,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    if !regular_projected_patch(surface, displacement, tolerance)? {
        return Err(REJECTED);
    }
    let axis = displacement.normalized_nonzero()?.as_vector();
    let point = |u, v| surface.evaluate(surface.parameter_at_u(u)?, surface.parameter_at_v(v)?);
    let center = point(0.5, 0.5)?;
    let u = point(0., 0.5)?.vector_to(point(1., 0.5)?)?;
    let v = point(0.5, 0.)?.vector_to(point(0.5, 1.)?)?;
    let twist = point(0., 1.)?.vector_to(point(1., 1.)?)?;
    let bottom = point(0., 0.)?.vector_to(point(1., 0.)?)?;
    let twist = Vector3::try_new(
        twist.x() - bottom.x(),
        twist.y() - bottom.y(),
        twist.z() - bottom.z(),
    )?;
    let project = |a: Vector3, b| a.cross(b)?.dot(axis);
    let determinant = project(u, v)?;
    let coordinates = |offset: Vector3| {
        Ok::<_, GeometryError>([
            project(offset, v)? / determinant,
            project(u, offset)? / determinant,
        ])
    };
    let [a, b] = coordinates(twist)?;
    let mut min = [-0.5_f64; 2];
    let mut max = [0.5_f64; 2];
    for control in stock
        .faces()
        .iter()
        .flat_map(|face| face.surface().control_points())
    {
        if control.weight() <= 0. {
            return Err(REJECTED);
        }
        // In the projected tangent coordinates x = s + a*s*t and
        // y = t + b*s*t. The branch at the center has positive Jacobian.
        let [x, y] = coordinates(center.vector_to(control.point())?)?;
        let s = regular_root(b, 1. + a * y - b * x, x).ok_or(REJECTED)?;
        let t = regular_root(a, 1. + b * x - a * y, y).ok_or(REJECTED)?;
        min[0] = min[0].min(s);
        min[1] = min[1].min(t);
        max[0] = max[0].max(s);
        max[1] = max[1].max(t);
    }
    let pad = 16. * tolerance.absolute();
    let [pu, pv] = [
        pad * v.cross(axis)?.length()? / determinant.abs(),
        pad * axis.cross(u)?.length()? / determinant.abs(),
    ];
    let [left, bottom] = [min[0] - pu, min[1] - pv];
    let [right, top] = [max[0] + pu, max[1] + pv];
    let extrapolate = |s, t| {
        center
            .translated(u.scaled(s)?)?
            .translated(v.scaled(t)?)?
            .translated(twist.scaled(s * t)?)
    };
    let corners = [
        extrapolate(left, bottom)?,
        extrapolate(right, bottom)?,
        extrapolate(right, top)?,
        extrapolate(left, top)?,
    ];
    let extended = NurbsSurface::try_bilinear(corners)?;
    // The projected Jacobian stays positive once all four corners agree
    // with the center. The extended image is then a convex quadrilateral
    // containing every projected control point.
    if !regular_projected_patch(&extended, displacement, tolerance)? {
        return Err(REJECTED);
    }
    for i in 0..4 {
        let corner = corners[i];
        let edge = corner.vector_to(corners[(i + 1) % 4])?;
        let normal = edge.cross(axis)?.normalized_nonzero()?.as_vector();
        let inward = normal.scaled(corner.vector_to(center)?.dot(normal)?.signum())?;
        for control in stock
            .faces()
            .iter()
            .flat_map(|face| face.surface().control_points())
        {
            if corner.vector_to(control.point())?.dot(inward)? <= tolerance.absolute() {
                return Err(REJECTED);
            }
        }
    }
    Ok(Brep::try_surface_face(extended, tolerance)?)
}

fn regular_root(a: f64, b: f64, x: f64) -> Option<f64> {
    let derivative = (4. * a).mul_add(x, b * b).sqrt();
    let root = if b >= 0. {
        2. * x / (b + derivative)
    } else {
        (derivative - b) / (2. * a)
    };
    (derivative > 0. && root.is_finite()).then_some(root)
}
