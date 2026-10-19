//! Tensor-product basis queries share the kernel evaluator.
use super::*;
impl NurbsSurface {
    /// Non-rational tensor-product basis factors in control-index order.
    /// Weights are deliberately excluded: rational point evaluation multiplies
    /// these factors by the control weights and divides by their weighted sum.
    pub fn basis_values(&self, u: Real, v: Real) -> Result<[Vec<Real>; 2], GeometryError> {
        Ok([
            crate::nurbs::bspline_basis_values(
                &self.knots_u,
                self.degree_u,
                self.control_point_count_u,
                u,
            )?,
            crate::nurbs::bspline_basis_values(
                &self.knots_v,
                self.degree_v,
                self.control_point_count_v,
                v,
            )?,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basis_reconstructs_rational_surface_and_partition_at_nonuniform_knots() {
        let controls = (0..5)
            .flat_map(|i| {
                (0..4).map(move |j| {
                    WeightedPoint3::try_new(
                        Point3::try_new(i as f64, j as f64, (i * j) as f64).unwrap(),
                        1. + (i * j) as f64 / 7.,
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let surface = NurbsSurface::try_new_rational(
            2,
            2,
            5,
            4,
            controls.clone(),
            vec![0., 0., 0., 0.1, 0.8, 1., 1., 1.],
            vec![0., 0., 0., 0.7, 1., 1., 1.],
        )
        .unwrap();
        for u in [0., 0.01, 0.1, 0.35, 0.8, 0.99, 1.] {
            for v in [0., 0.3, 0.7, 1.] {
                let [bu, bv] = surface.basis_values(u, v).unwrap();
                assert!((bu.iter().sum::<f64>() - 1.).abs() < 1e-14);
                assert!((bv.iter().sum::<f64>() - 1.).abs() < 1e-14);
                assert!(bu.iter().chain(&bv).all(|b| *b >= 0.));
                let mut homogeneous = [0.; 4];
                for (i, control) in controls.iter().enumerate() {
                    let weight = bu[i % 5] * bv[i / 5] * control.weight();
                    for (h, p) in homogeneous
                        .iter_mut()
                        .zip(control.point().to_array().into_iter().chain([1.]))
                    {
                        *h += weight * p;
                    }
                }
                let point = Point3::try_new(
                    homogeneous[0] / homogeneous[3],
                    homogeneous[1] / homogeneous[3],
                    homogeneous[2] / homogeneous[3],
                )
                .unwrap();
                assert!(point.distance_to(surface.evaluate(u, v).unwrap()).unwrap() < 1e-12);
            }
        }
        assert!(surface.basis_values(-0.01, 0.5).is_err());
        assert!(surface.basis_values(0.5, f64::NAN).is_err());
    }
}
