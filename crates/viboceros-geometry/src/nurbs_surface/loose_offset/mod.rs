//! Original-basis tensor interpolation of normal displacements.
use super::*;
use crate::ParameterSide;
use faer::Mat;
mod axis;
use axis::Layout;
#[cfg(test)]
mod tests;

fn unsupported(context: &'static str) -> GeometryError {
    GeometryError::UnsupportedLooseSurfaceOffset { context }
}

impl NurbsSurface {
    /// Offset the original Greville edit locations along their natural normals,
    /// retaining degrees, knots, weights and control counts. This approximates
    /// an offset between stations; it does not bound distance to the true offset.
    /// Sparse tensor solves avoid a dense control-net matrix. Exact periodic
    /// tails and clamped axes through degree 17 are supported within 16M local
    /// basis evaluations. Singular boundary normals use public OpenNURBS limits.
    pub fn try_loose_offset(
        &self,
        distance: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([distance], "loose surface offset distance")?;
        if distance == 0. {
            return Ok(self.clone());
        }
        if self.degree_u.max(self.degree_v) > 17
            || self.control_points.len() > 262_144
            || self
                .control_points
                .len()
                .saturating_mul((self.degree_u + 1) * (self.degree_v + 1))
                > 16 * 1024 * 1024
        {
            return Err(unsupported("degree or control-net work limit"));
        }
        let scale = self
            .control_points
            .iter()
            .map(|c| c.weight().abs())
            .fold(0., Real::max);
        let positive = self.control_points[0].weight().is_sign_positive();
        if self
            .control_points
            .iter()
            .any(|c| c.weight().is_sign_positive() != positive || c.weight() / scale == 0.)
        {
            return Err(unsupported(
                "mixed-sign or unrepresentable normalized weights",
            ));
        }
        let periodic = [self.is_periodic_u(), self.is_periodic_v()];
        let counts = [self.control_point_count_u, self.control_point_count_v];
        for axis in 0..2 {
            if periodic[axis] {
                let degree = [self.degree_u, self.degree_v][axis];
                for j in 0..counts[1 - axis] {
                    for i in 0..degree {
                        let index = |k| {
                            if axis == 0 {
                                j * counts[0] + k
                            } else {
                                k * counts[0] + j
                            }
                        };
                        if self.control_points[index(i)]
                            != self.control_points[index(counts[axis] - degree + i)]
                        {
                            return Err(unsupported("periodic tails must repeat exactly"));
                        }
                    }
                }
            }
        }
        let u = Layout::new(self.degree_u, &self.knots_u, periodic[0])?;
        let v = Layout::new(self.degree_v, &self.knots_v, periodic[1])?;
        let mut normals = Vec::with_capacity(u.len() * v.len());
        let mut missing = Vec::new();
        for sv in &v.stations {
            for su in &u.stations {
                let normal = self
                    .normal_at_on_sides(su.parameter, sv.parameter, su.side, sv.side)
                    .ok();
                if normal.is_none() {
                    missing.push(([su.parameter, sv.parameter], normals.len()));
                }
                normals.push(normal);
            }
        }
        if !missing.is_empty() {
            let parameters = missing.iter().map(|&(uv, _)| uv).collect::<Vec<_>>();
            for ((_, i), frame) in missing.into_iter().zip(self.limit_frames_at(&parameters)?) {
                normals[i] = frame.map(|f| f.z_axis());
            }
        }
        let normals = normals
            .into_iter()
            .map(|n| n.ok_or_else(|| unsupported("undefined normal at a Greville station")))
            .collect::<Result<Vec<_>, _>>()?;
        // Interpolate homogeneous displacement alone: retaining the original
        // homogeneous controls avoids subtracting/reconstructing large origins.
        let targets = Mat::from_fn(u.len(), v.len() * 3, |i, c| {
            let j = c / 3;
            let (uf, ub) = &u.rows[i];
            let (vf, vb) = &v.rows[j];
            let mut weight = 0.;
            for (b, &bv) in vb.iter().enumerate() {
                for (a, &bu) in ub.iter().enumerate() {
                    weight += bu
                        * bv
                        * (self.control_points[(vf + b) * counts[0] + uf + a].weight() / scale);
                }
            }
            weight * normals[j * u.len() + i].as_vector().to_array()[c % 3]
        });
        let first = u.solve(targets)?;
        let transposed = Mat::from_fn(v.len(), u.len() * 3, |j, c| first[(c / 3, j * 3 + c % 3)]);
        let solved = v.solve(transposed)?;
        let controls = self
            .control_points
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let ui = (i % counts[0]) % u.len();
                let vi = (i / counts[0]) % v.len();
                let w = c.weight() / scale;
                let p = c.point().to_array();
                WeightedPoint3::try_new(
                    Point3::try_from(std::array::from_fn(|k| {
                        distance.mul_add(solved[(vi, ui * 3 + k)] / w, p[k])
                    }))?,
                    c.weight(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            counts[0],
            counts[1],
            controls,
            self.knots_u.clone(),
            self.knots_v.clone(),
        )?;
        for (j, sv) in v.stations.iter().enumerate() {
            for (i, su) in u.stations.iter().enumerate() {
                let p = self.evaluate_on_sides(su.parameter, sv.parameter, su.side, sv.side)?;
                let target =
                    p.translated(normals[j * u.len() + i].as_vector().scaled(distance)?)?;
                let actual =
                    result.evaluate_on_sides(su.parameter, sv.parameter, su.side, sv.side)?;
                let rounding = 64.
                    * Real::EPSILON
                    * p.to_array()
                        .into_iter()
                        .map(Real::abs)
                        .fold(distance.abs(), Real::max);
                if target.distance_to(actual)? > tolerance.absolute() + rounding {
                    return Err(unsupported("station interpolation did not meet tolerance"));
                }
            }
        }
        Ok(result)
    }
}
