use crate::nurbs::{de_boor_extended, stable_divided_difference};
use crate::{GeometryError, Point3, Real, Vector3};

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_tensor_product(
    controls: &[[Real; 4]],
    row_width: usize,
    knots_u: &[Real],
    degree_u: usize,
    span_u: usize,
    u: Real,
    knots_v: &[Real],
    degree_v: usize,
    span_v: usize,
    v: Real,
) -> Result<[Real; 4], GeometryError> {
    debug_assert_eq!(controls.len(), row_width * (degree_v + 1));
    let mut evaluated_u = Vec::with_capacity(degree_v + 1);
    for row in controls.chunks_exact(row_width) {
        evaluated_u.push(de_boor_extended(
            knots_u,
            degree_u,
            span_u,
            u,
            row.to_vec(),
        )?);
    }
    de_boor_extended(knots_v, degree_v, span_v, v, evaluated_u)
}

pub(super) fn derivative_controls_u(
    controls: &[[Real; 4]],
    degree_u: usize,
    degree_v: usize,
    span_u: usize,
    knots_u: &[Real],
) -> Result<Vec<[Real; 4]>, GeometryError> {
    let first_u = span_u - degree_u;
    let source_width = degree_u + 1;
    let mut result = Vec::with_capacity(degree_u * (degree_v + 1));
    for row in controls.chunks_exact(source_width) {
        for local_u in 0..degree_u {
            let index = first_u + local_u;
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    row[local_u + 1][coordinate],
                    row[local_u][coordinate],
                    degree_u,
                    knots_u[index + 1],
                    knots_u[index + degree_u + 1],
                )?;
            }
            result.push(derivative);
        }
    }
    Ok(result)
}

pub(super) fn derivative_controls_v(
    controls: &[[Real; 4]],
    degree_u: usize,
    degree_v: usize,
    span_v: usize,
    knots_v: &[Real],
) -> Result<Vec<[Real; 4]>, GeometryError> {
    let first_v = span_v - degree_v;
    let row_width = degree_u + 1;
    let mut result = Vec::with_capacity(row_width * degree_v);
    for local_v in 0..degree_v {
        let index = first_v + local_v;
        for local_u in 0..row_width {
            let lower = controls[local_v * row_width + local_u];
            let upper = controls[(local_v + 1) * row_width + local_u];
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    upper[coordinate],
                    lower[coordinate],
                    degree_v,
                    knots_v[index + 1],
                    knots_v[index + degree_v + 1],
                )?;
            }
            result.push(derivative);
        }
    }
    Ok(result)
}

pub(super) fn project_derivative(
    point: Point3,
    homogeneous: [Real; 4],
    derivative: [Real; 4],
) -> Result<Vector3, GeometryError> {
    let weight = homogeneous[3];
    let weight_derivative = derivative[3];
    let point = point.to_array();
    let projected = std::array::from_fn(|coordinate| {
        (-point[coordinate]).mul_add(weight_derivative, derivative[coordinate]) / weight
    });
    Vector3::try_from(projected)
}
