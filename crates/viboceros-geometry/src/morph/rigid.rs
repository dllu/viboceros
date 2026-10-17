//! Forward derivative frame inferred from public Twist and attenuated Bend commands.
use crate::{
    AffineTransform3, Frame3, GeometryError, Point3, PointMorph, Real, Tolerance, Vector3,
};

pub(crate) fn forward_transform(
    morph: &(impl PointMorph + ?Sized),
    center: Point3,
) -> Result<AffineTransform3, GeometryError> {
    fn direction_or_zero(vector: Vector3) -> Result<Vector3, GeometryError> {
        if vector.to_array() == [0.; 3] {
            Ok(vector)
        } else {
            Ok(vector.normalized_nonzero()?.as_vector())
        }
    }
    let scale = center
        .to_array()
        .into_iter()
        .map(Real::abs)
        .fold(0., Real::max);
    let step = (scale * 1.490116119385e-8 + 2.3283064365386963e-10).sqrt();
    let mapped = morph.morph_point(center)?;
    let mut directions = [Vector3::try_new(0., 0., 0.)?; 3];
    for (i, direction) in directions.iter_mut().enumerate() {
        let mut offset = [0.; 3];
        offset[i] = step;
        *direction = direction_or_zero(
            mapped.vector_to(morph.morph_point(center.translated(Vector3::try_from(offset)?)?)?)?,
        )?;
    }
    let [x, y, z] = directions;
    let sum = |a: Vector3, b: Vector3| {
        let a = a.to_array();
        let b = b.to_array();
        Vector3::try_from(std::array::from_fn(|i| a[i] + b[i]))
    };
    // Native midpoint witnesses retain the first two forward directions when
    // the sampled frame is already nearly orthogonal. Public OpenNURBS frame
    // validity supplies the observed 2^-16 dot bound and determinant check.
    let sqrt_epsilon = 1.490116119385e-8;
    let max_dot = x.dot(y)?.abs().max(y.dot(z)?.abs()).max(z.dot(x)?.abs());
    let orthogonal = max_dot <= sqrt_epsilon
        || (max_dot < 0.0000152587890625 && (x.cross(y)?.dot(z)?.abs() - 1.).abs() <= sqrt_epsilon);
    let (x, y) = if orthogonal {
        (x, y)
    } else {
        let normal_x = direction_or_zero(y.cross(z)?)?;
        let normal_y = direction_or_zero(z.cross(x)?)?;
        // A reflected derivative can cancel one averaged direction exactly.
        // Retain that forward direction while averaging the other axis.
        let averaged_x = sum(x, normal_x)?;
        let averaged_y = sum(y, normal_y)?;
        (
            if averaged_x.to_array() == [0.; 3] {
                x
            } else {
                averaged_x
            },
            if averaged_y.to_array() == [0.; 3] {
                y
            } else {
                averaged_y
            },
        )
    };
    let frame =
        Frame3::try_from_directions(mapped, x, y, Tolerance::try_new(1e-14, 1e-14, 1e-14)?)?;
    let [x, y, z] = [
        frame.x_axis().as_vector().to_array(),
        frame.y_axis().as_vector().to_array(),
        frame.z_axis().as_vector().to_array(),
    ];
    AffineTransform3::try_mapping_origins(
        std::array::from_fn(|i| [x[i], y[i], z[i]]),
        center,
        mapped,
    )
}
