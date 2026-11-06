//! Affine planar patches with editable, open uniform tensor-product bases.
use super::*;

pub const MAX_PLANE_CONTROL_POINTS: usize = 100_000;

impl NurbsSurface {
    /// Construct a rectangular plane with physical length parameter domains.
    /// Control locations at the uniform basis's Greville stations preserve the
    /// affine parameter map for arbitrary valid degrees and control counts.
    pub fn try_rectangular_plane(
        frame: Frame3,
        dimensions: [Real; 2],
        degrees: [usize; 2],
        counts: [usize; 2],
    ) -> Result<Self, GeometryError> {
        require_finite(dimensions, "rectangular plane dimensions")?;
        if dimensions.iter().any(|d| *d <= 0.) {
            return Err(GeometryError::Degenerate {
                context: "rectangular plane dimensions",
            });
        }
        if counts[0]
            .checked_mul(counts[1])
            .is_none_or(|n| n > MAX_PLANE_CONTROL_POINTS)
        {
            return Err(GeometryError::InvalidControlNet {
                context: "rectangular plane exceeds 100000 control points",
            });
        }
        let knots_u = clamped_uniform_knots(degrees[0], counts[0])?;
        let knots_v = clamped_uniform_knots(degrees[1], counts[1])?;
        // Sum the clipped arithmetic sequence analytically. Work stays linear
        // in the net size even when degree approaches the control count.
        fn stations(degree: usize, count: usize) -> Vec<Real> {
            (0..count)
                .map(|i| {
                    let a = (i + 1).max(degree + 1);
                    let b = (i + degree).min(count - 1);
                    let interior = if b < a {
                        0.
                    } else {
                        (a + b - 2 * degree) as Real * (b - a + 1) as Real
                            / (2. * (count - degree) as Real)
                    };
                    (interior + (i + degree + 1).saturating_sub(count) as Real) / degree as Real
                })
                .collect()
        }
        let us = stations(degrees[0], counts[0]);
        let vs = stations(degrees[1], counts[1]);
        let mut points = Vec::with_capacity(counts[0] * counts[1]);
        for v in vs {
            for &u in &us {
                points.push(frame.point_at([u * dimensions[0], v * dimensions[1], 0.])?);
            }
        }
        Self::try_new(
            degrees[0],
            degrees[1],
            counts[0],
            counts[1],
            points,
            knots_u.into_iter().map(|t| t * dimensions[0]).collect(),
            knots_v.into_iter().map(|t| t * dimensions[1]).collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_planar_bases_preserve_the_affine_parameter_map() {
        let frame = Frame3::try_from_directions(
            Point3::try_from([1e6, -2e5, 3e4]).unwrap(),
            Vector3::try_from([1., 2., 3.]).unwrap(),
            Vector3::try_from([-2., 4., 1.]).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        for (degrees, counts) in [([1, 1], [2, 2]), ([3, 2], [6, 5]), ([11, 9], [12, 16])] {
            let s = NurbsSurface::try_rectangular_plane(frame, [8., 5.], degrees, counts).unwrap();
            for i in 0..=8 {
                for j in 0..=8 {
                    let u = i as Real;
                    let v = j as Real * 5. / 8.;
                    assert!(
                        s.evaluate(u, v)
                            .unwrap()
                            .distance_to(frame.point_at([u, v, 0.]).unwrap())
                            .unwrap()
                            < 2e-9
                    );
                }
            }
            assert_eq!(s.domain_u(), 0. ..=8.);
            assert_eq!(s.domain_v(), 0. ..=5.);
        }
    }

    #[test]
    fn invalid_or_excessive_planar_nets_fail_before_allocation() {
        for (dimensions, degrees, counts) in [
            ([0., 5.], [1, 1], [2, 2]),
            ([8., f64::NAN], [1, 1], [2, 2]),
            ([8., 5.], [0, 1], [2, 2]),
            ([8., 5.], [3, 1], [3, 2]),
            ([8., 5.], [1, 1], [usize::MAX, 2]),
            ([8., 5.], [1, 1], [100_000, 2]),
        ] {
            let frame = Frame3::try_from_directions(
                Point3::try_from([0.; 3]).unwrap(),
                Vector3::try_from([1., 0., 0.]).unwrap(),
                Vector3::try_from([0., 1., 0.]).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            assert!(
                NurbsSurface::try_rectangular_plane(frame, dimensions, degrees, counts).is_err()
            );
        }
    }
}
