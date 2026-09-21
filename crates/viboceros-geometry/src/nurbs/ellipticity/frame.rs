//! Affine coefficient coordinates, not a model-space distance metric.
use super::*;

pub(super) struct CoefficientFrame {
    pub(super) origin: Point3,
    pub(super) axes: [Vector3; 2],
    pub(super) normal: Vector3,
    pub(super) scales: [Real; 2],
}

impl CoefficientFrame {
    pub(super) fn from_controls(
        controls: &[&WeightedPoint3],
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let mut sums: [crate::FiniteSum; 3] = std::array::from_fn(|_| crate::FiniteSum::default());
        for control in controls {
            for (sum, coordinate) in sums.iter_mut().zip(control.point().to_array()) {
                sum.add(coordinate)?;
            }
        }
        let origin = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
        let offsets = controls
            .iter()
            .map(|p| origin.vector_to(p.point()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut scale = 0.;
        let mut x = offsets[0];
        for &v in &offsets {
            let length = v.length()?;
            if length > scale {
                (scale, x) = (length, v);
            }
        }
        if scale <= tolerance.absolute() {
            return Ok(None);
        }
        let x = x.normalized_nonzero()?.as_vector();
        let mut normal = offsets[0];
        let mut height = 0.;
        for &v in &offsets {
            let cross = x.cross(v.scaled(1. / scale)?)?;
            let length = cross.length()?;
            if length > height {
                (height, normal) = (length, cross);
            }
        }
        if height <= 128. * Real::EPSILON {
            return Ok(None);
        }
        let normal = normal.normalized_nonzero()?.as_vector();
        let y = normal.cross(x)?.normalized_nonzero()?.as_vector();
        let mut matrix = Mat::<Real>::zeros(offsets.len(), 2);
        for (i, v) in offsets.iter().enumerate() {
            let v = v.scaled(1. / scale)?;
            matrix[(i, 0)] = v.dot(x)?;
            matrix[(i, 1)] = v.dot(y)?;
        }
        let Ok(svd) = matrix.thin_svd() else {
            return Ok(None);
        };
        let axis = |i| {
            Vector3::try_from(std::array::from_fn(|j| {
                x.to_array()[j].mul_add(svd.V()[(0, i)], y.to_array()[j] * svd.V()[(1, i)])
            }))?
            .normalized_nonzero()
            .map(|v| v.as_vector())
        };
        let axes = [axis(0)?, axis(1)?];
        let mut scales = [0.0_f64; 2];
        for v in offsets {
            for i in 0..2 {
                scales[i] = scales[i].max(v.dot(axes[i])?.abs());
            }
        }
        if scales.iter().any(|&s| s <= tolerance.absolute()) {
            return Ok(None);
        }
        Ok(Some(Self {
            origin,
            axes,
            normal,
            scales,
        }))
    }

    pub(super) fn vector(&self, coordinates: [Real; 2]) -> Result<Vector3, GeometryError> {
        Vector3::try_from(std::array::from_fn(|i| {
            self.axes[0].to_array()[i]
                .mul_add(coordinates[0], self.axes[1].to_array()[i] * coordinates[1])
        }))
    }
}
