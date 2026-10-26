use super::*;

/// Exact straight-region extraction proves equality of complete images, even
/// for nonlinear rational clocks. Require exactly representable proxy vertices;
/// the existing polygon path separately handles rounded active vertices.
pub(super) fn linear_image_proxy(source: &NurbsCurve) -> Result<Option<NurbsCurve>, GeometryError> {
    let Some(vertices) = crate::surface_pullback::exact_linear_curve_vertices(source)? else {
        return Ok(None);
    };
    let mut points = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let [x, y, z] = vertex.each_ref().map(scalar);
        let xyz = [x?, y?, z?];
        if (0..3).any(|i| rational(xyz[i]) != vertex[i]) {
            return Ok(None);
        }
        points.push(Point3::try_from(xyz)?);
    }
    let mut knots = vec![0., 0.];
    let mut length = 0.;
    for pair in points.windows(2) {
        let next = length + pair[0].distance_to(pair[1])?;
        if !next.is_finite() || next <= length {
            return Ok(None);
        }
        length = next;
        knots.push(length);
    }
    knots.push(length);
    Ok(Some(NurbsCurve::try_new(1, points, knots)?))
}
