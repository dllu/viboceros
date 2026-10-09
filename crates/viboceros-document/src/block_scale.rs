//! Instance-frame scale reset without editing shared prototype geometry.
use super::*;
use viboceros_geometry::{AffineNormalTransform3, Frame3, Point3, Vector3};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockScaleResetMode {
    #[default]
    One,
    Automatic,
}

fn reset_placement(
    transform: AffineTransform3,
    mode: BlockScaleResetMode,
) -> Result<AffineTransform3, GeometryError> {
    let rows = transform.linear_rows();
    let columns =
        std::array::from_fn::<_, 3, _>(|j| Vector3::try_from(rows.map(|r| r[j])).unwrap());
    let scales = [
        columns[0].length()?,
        columns[1].length()?,
        columns[2].length()?,
    ];
    let common = match mode {
        BlockScaleResetMode::One => 1.,
        BlockScaleResetMode::Automatic => {
            // Absolute and relative bands calibrated against native decisions.
            let equal = |a: f64, b: f64| {
                let relative_band = 2.25e-10;
                (a - b).abs()
                    <= f64::EPSILON.sqrt() + relative_band * a.abs() + relative_band * b.abs()
            };
            if equal(scales[0], scales[1]) || equal(scales[0], scales[2]) {
                scales[0]
            } else if equal(scales[1], scales[2]) {
                scales[1]
            } else {
                Vector3::try_from(scales)?.dot(Vector3::try_from([1. / 3.; 3])?)?
            }
        }
    };
    let normal =
        AffineNormalTransform3::new(transform).transform_normal(Vector3::try_new(0., 0., 1.)?)?;
    let frame = Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.)?,
        columns[0].normalized_nonzero()?.as_vector(),
        normal.as_vector(),
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    let axes = frame.axes().map(|a| a.as_vector());
    let factors = scales.map(|scale| common / scale);
    let local =
        AffineTransform3::try_new(axes.map(Vector3::to_array), Vector3::try_from([0.; 3])?)?;
    let world = AffineTransform3::try_new(
        std::array::from_fn(|i| std::array::from_fn(|j| axes[j].to_array()[i])),
        Vector3::try_from([0.; 3])?,
    )?;
    // Scale in the orthonormal instance frame, then restore insertion exactly.
    let projected = AffineTransform3::try_new(rows, Vector3::try_from([0.; 3])?)?.then(local)?;
    let scaled = if factors.iter().all(|v| v.is_finite() && *v > 0.) {
        let scaling = AffineTransform3::try_new(
            [
                [factors[0], 0., 0.],
                [0., factors[1], 0.],
                [0., 0., factors[2]],
            ],
            Vector3::try_from([0.; 3])?,
        )?;
        projected.then(scaling)?
    } else {
        // A ratio can overflow or underflow although final coefficients are finite.
        let mut result = projected.linear_rows();
        for (row, scale) in result.iter_mut().zip(scales) {
            for coordinate in row {
                *coordinate =
                    viboceros_geometry::remap_scalar(*coordinate, [0., scale], [0., common])?;
            }
        }
        AffineTransform3::try_new(result, Vector3::try_from([0.; 3])?)?
    };
    let linear = scaled.then(world)?;
    AffineTransform3::try_new(linear.linear_rows(), transform.translation())
}

impl Document {
    pub fn reset_block_scale(
        &mut self,
        sources: impl IntoIterator<Item = ObjectId>,
        mode: BlockScaleResetMode,
    ) -> Result<usize, DocumentError> {
        let indices = self.resolve_object_indices(sources)?;
        if indices.is_empty() {
            return Err(DocumentError::EmptyBlock);
        }
        let staged = indices
            .into_iter()
            .map(|index| {
                let object = &self.objects[index];
                self.ensure_object_editable(object)?;
                let Geometry::BlockInstance(instance) = object.geometry() else {
                    return Err(DocumentError::NotBlockInstance(object.id()));
                };
                let placement = reset_placement(instance.reference().transform(), mode)?;
                let reference =
                    BlockReference::try_new(instance.reference().definition(), placement)?;
                Ok((index, self.block_instance_geometry(reference)?))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        self.commit_object_geometries(
            staged,
            "BlockResetScale",
            "Reset block scale",
            ReplacementHistory::EveryReplacement,
            true,
        )
    }
}

#[cfg(test)]
mod tests;
