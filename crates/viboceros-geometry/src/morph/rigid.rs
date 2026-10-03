//! Forward derivative frame inferred from public Twist and attenuated Bend commands.
use crate::{
    AffineTransform3, Frame3, GeometryError, Point3, PointMorph, Real, Tolerance, Vector3,
};

pub(crate) fn forward_transform(
    morph: &(impl PointMorph + ?Sized),
    center: Point3,
) -> Result<AffineTransform3, GeometryError> {
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
        *direction = mapped
            .vector_to(morph.morph_point(center.translated(Vector3::try_from(offset)?)?)?)?
            .normalized_nonzero()?
            .as_vector();
    }
    let [x, y, z] = directions;
    let normal_x = y.cross(z)?.normalized_nonzero()?.as_vector();
    let normal_y = z.cross(x)?.normalized_nonzero()?.as_vector();
    let sum = |a: Vector3, b: Vector3| {
        let a = a.to_array();
        let b = b.to_array();
        Vector3::try_from(std::array::from_fn(|i| a[i] + b[i]))
    };
    let frame = Frame3::try_from_directions(
        mapped,
        sum(x, normal_x)?,
        sum(y, normal_y)?,
        Tolerance::try_new(1e-14, 1e-14, 1e-14)?,
    )?;
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
