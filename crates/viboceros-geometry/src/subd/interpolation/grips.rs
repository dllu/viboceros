//! Restricted native limit masks move selected controls and certify their stations.
use super::*;
use std::collections::BTreeMap;

impl SubD {
    /// Match selected limit stations while retaining every unselected control
    /// point. Other surface stations may move. Indices are control ordinals;
    /// repeated targets use their last value. A failed solve leaves this source intact.
    pub fn try_with_selected_limit_points(
        &self,
        targets: &[(usize, Point3)],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let targets = targets.iter().copied().collect::<BTreeMap<_, _>>();
        if let Some(&index) = targets.keys().find(|&&i| i >= self.control_points().len()) {
            return Err(GeometryError::InvalidControlPointIndex {
                index,
                count: self.control_points().len(),
            });
        }
        if targets.iter().all(|(&i, &p)| p == self.surface_points()[i]) {
            return Ok(self.clone());
        }
        let n = targets.len();
        let indices = targets.keys().copied().collect::<Vec<_>>();
        let mut columns = vec![usize::MAX; self.control_points().len()];
        for (column, &i) in indices.iter().enumerate() {
            columns[i] = column;
        }
        let weights = viboceros_opennurbs::subd_interpolation_weights(self.archive())?;
        let mut triplets = Vec::new();
        for (row, &i) in indices.iter().enumerate() {
            for k in weights.row_offsets[i]..weights.row_offsets[i + 1] {
                let column = columns[weights.columns[k] as usize];
                if column != usize::MAX {
                    triplets.push(Triplet::new(row, column, weights.values[k]));
                }
            }
        }
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &triplets)
            .map_err(|_| GeometryError::SubDInterpolationFailed)?;
        let factor = matrix
            .sp_lu()
            .map_err(|_| GeometryError::SubDInterpolationFailed)?;
        let deltas = indices
            .iter()
            .map(|&i| {
                let a = self.surface_points()[i].to_array();
                let b = targets[&i].to_array();
                Vector3::try_new(b[0] - a[0], b[1] - a[1], b[2] - a[2]).map(|v| v.to_array())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scale = deltas
            .iter()
            .flatten()
            .map(|x| x.abs())
            .fold(0f64, f64::max);
        if scale == 0. {
            return Ok(self.clone());
        }
        let rhs = faer::Mat::from_fn(n, 3, |i, axis| deltas[i][axis] / scale);
        let mut solution = factor.solve(&rhs);
        for _ in 0..8 {
            let mut controls = self.control_points().to_vec();
            for (row, &i) in indices.iter().enumerate() {
                let p = controls[i].to_array();
                controls[i] = Point3::try_from(std::array::from_fn::<_, 3, _>(|axis| {
                    solution[(row, axis)].mul_add(scale, p[axis])
                }))?;
            }
            let edited = self.try_with_control_points(&controls)?;
            let maximum = targets.iter().try_fold(0f64, |error, (&i, &p)| {
                p.distance_to(edited.surface_points()[i])
                    .map(|e| error.max(e))
            })?;
            if maximum <= tolerance.absolute() {
                return Ok(edited);
            }
            let residual = faer::Mat::from_fn(n, 3, |row, axis| {
                (targets[&indices[row]].to_array()[axis]
                    - edited.surface_points()[indices[row]].to_array()[axis])
                    / scale
            });
            let correction = factor.solve(&residual);
            for i in 0..n {
                for axis in 0..3 {
                    solution[(i, axis)] += correction[(i, axis)];
                }
            }
        }
        Err(GeometryError::SubDInterpolationFailed)
    }
}
