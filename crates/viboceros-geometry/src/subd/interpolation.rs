//! Sparse limit-position collocation with native evaluation and certification.
use super::*;
use crate::Tolerance;
use faer::prelude::*;
use faer::sparse::{SparseColMat, Triplet};
mod grips;

impl SubD {
    /// Moves the control net so each vertex limit station meets its target.
    /// Component tags, connectivity and native fields remain in the archive.
    /// The sparse limit masks use the public OpenNURBS evaluator. All three
    /// coordinates share one pivoted factorization, followed by native residual
    /// checks and bounded iterative refinement. Failure leaves this object intact.
    pub fn try_with_limit_points(
        &self,
        targets: &[Point3],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if targets.len() != self.control_points().len() {
            return Err(GeometryError::InvalidSubDInterpolationTargets);
        }
        if targets == self.surface_points() {
            return Ok(self.clone());
        }
        let weights = viboceros_opennurbs::subd_interpolation_weights(self.archive())?;
        let n = targets.len();
        let mut triplets = Vec::with_capacity(weights.values.len());
        for row in 0..n {
            for k in weights.row_offsets[row]..weights.row_offsets[row + 1] {
                triplets.push(Triplet::new(
                    row,
                    weights.columns[k] as usize,
                    weights.values[k],
                ));
            }
        }
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &triplets)
            .map_err(|_| GeometryError::SubDInterpolationFailed)?;
        let factor = matrix
            .sp_lu()
            .map_err(|_| GeometryError::SubDInterpolationFailed)?;
        // Center before scaling, retaining small features in large coordinates.
        // Half-sums avoid overflow when the target interval spans most of f64.
        let bounds = BoundingBox3::from_points(targets.iter().copied())?;
        let min = bounds.min().to_array();
        let max = bounds.max().to_array();
        let origin: [f64; 3] = std::array::from_fn(|k| min[k] * 0.5 + max[k] * 0.5);
        let scale = targets
            .iter()
            .flat_map(|p| {
                p.to_array()
                    .into_iter()
                    .zip(origin)
                    .map(|(p, o)| (p - o).abs())
            })
            .fold(0f64, f64::max);
        let scale = if scale > 0. { scale } else { 1. };
        let rhs = faer::Mat::from_fn(n, 3, |i, k| (targets[i].to_array()[k] - origin[k]) / scale);
        let mut solution = factor.solve(&rhs);
        for iteration in 0..8 {
            let controls = (0..n)
                .map(|i| {
                    Point3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                        solution[(i, k)].mul_add(scale, origin[k])
                    }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = self.try_with_control_points(&controls)?;
            let maximum = result
                .surface_points()
                .iter()
                .zip(targets)
                .try_fold(0f64, |error, (a, b)| {
                    Ok::<_, GeometryError>(error.max(a.distance_to(*b)?))
                })?;
            if maximum <= tolerance.absolute() {
                return Ok(result);
            }
            if iteration == 7 {
                break;
            }
            let residual = faer::Mat::from_fn(n, 3, |i, k| {
                (targets[i].to_array()[k] - result.surface_points()[i].to_array()[k]) / scale
            });
            let correction = factor.solve(&residual);
            for i in 0..n {
                for k in 0..3 {
                    solution[(i, k)] += correction[(i, k)];
                }
            }
        }
        Err(GeometryError::SubDInterpolationFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn certify(subd: &SubD) {
        let weights = viboceros_opennurbs::subd_interpolation_weights(subd.archive()).unwrap();
        for row in 0..subd.control_points().len() {
            let mut value = [0.; 3];
            for k in weights.row_offsets[row]..weights.row_offsets[row + 1] {
                for (axis, x) in value.iter_mut().enumerate() {
                    *x += weights.values[k]
                        * subd.control_points()[weights.columns[k] as usize].to_array()[axis];
                }
            }
            assert!(
                Point3::try_from(value)
                    .unwrap()
                    .distance_to(subd.surface_points()[row])
                    .unwrap()
                    < 1e-12
            );
        }
        let before = subd.archive().to_vec();
        let fit = subd
            .try_with_limit_points(
                subd.control_points(),
                Tolerance::try_new(1e-10, 1e-12, 1e-8).unwrap(),
            )
            .unwrap();
        for (point, target) in fit.surface_points().iter().zip(subd.control_points()) {
            assert!(point.distance_to(*target).unwrap() < 1e-10);
        }
        assert_eq!(fit.faces(), subd.faces());
        assert_eq!(fit.vertex_tags(), subd.vertex_tags());
        assert_eq!(subd.archive(), before);
    }
    #[test]
    fn native_sparse_masks_and_interpolation_cover_interior_triangles_and_closed_surfaces() {
        let tetra = SubD::try_from_polygons(
            vec![p(0., 0., 0.), p(4., 0., 0.), p(1., 3., 0.), p(1., 1., 3.)],
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
        )
        .unwrap();
        certify(&tetra);
        let mut points = vec![p(0., 0., 1.)];
        for i in 0..5 {
            let t = i as f64 * std::f64::consts::TAU / 5.;
            points.push(p(t.cos(), t.sin(), 0.));
        }
        let fan = SubD::try_from_polygons(
            points,
            (0..5).map(|i| vec![0, i + 1, (i + 1) % 5 + 1]).collect(),
        )
        .unwrap();
        certify(&fan);
        let points = (0..4)
            .flat_map(|y| (0..4).map(move |x| p(x as f64, y as f64, ((x + y) as f64).sin())))
            .collect();
        let panel = SubD::try_from_polygons(
            points,
            (0..3)
                .flat_map(|y| {
                    (0..3).map(move |x| {
                        vec![
                            y * 4 + x,
                            y * 4 + x + 1,
                            (y + 1) * 4 + x + 1,
                            (y + 1) * 4 + x,
                        ]
                    })
                })
                .collect(),
        )
        .unwrap();
        certify(&panel);
        assert!(
            tetra
                .try_with_limit_points(&[], Tolerance::DEFAULT)
                .is_err()
        );
    }
    #[test]
    fn large_boundary_cycle_stays_sparse_and_handles_translated_scales() {
        let n = 4096usize;
        let points = (0..n)
            .map(|i| {
                let t = i as f64 * std::f64::consts::TAU / n as f64;
                p(t.cos(), t.sin(), 0.)
            })
            .collect();
        let subd = SubD::try_from_polygons(points, vec![(0..n as u32).collect()]).unwrap();
        let weights = viboceros_opennurbs::subd_interpolation_weights(subd.archive()).unwrap();
        assert_eq!(weights.values.len(), 3 * n);
        let fit = subd
            .try_with_limit_points(
                subd.control_points(),
                Tolerance::try_new(1e-10, 1e-12, 1e-8).unwrap(),
            )
            .unwrap();
        for (a, b) in fit.surface_points().iter().zip(subd.control_points()) {
            assert!(a.distance_to(*b).unwrap() < 1e-10);
        }
        for scale in [1e-6, 1., 1e6] {
            let targets = [p(0., 0., 0.), p(4., 0., 0.), p(4., 3., 2.), p(0., 3., 0.)]
                .into_iter()
                .map(|p| Point3::try_from(p.to_array().map(|x| x * scale + 1e6)).unwrap())
                .collect::<Vec<_>>();
            let subd = SubD::try_from_polygons(targets.clone(), vec![vec![0, 1, 2, 3]]).unwrap();
            let fit = subd
                .try_with_limit_points(&targets, Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap())
                .unwrap();
            for (a, b) in fit.surface_points().iter().zip(targets) {
                assert!(a.distance_to(b).unwrap() < 1e-8);
            }
        }
    }
}
