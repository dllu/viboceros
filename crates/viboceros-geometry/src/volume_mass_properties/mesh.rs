use super::*;
use crate::{
    TriangleMesh,
    binary_accumulator::monomials::{Cubics, Quartics},
};

impl TriangleMesh {
    /// Exact signed tetrahedral volume and first moments of binary64 mesh
    /// vertices. Requires a closed, consistently oriented manifold mesh.
    /// Unused vertices do not contribute. No vertex-coordinate recentering or
    /// normalization is needed: cancellation is exact across the full range.
    pub fn volume_mass_properties(&self) -> Result<VolumeMassProperties, GeometryError> {
        if !self.topology().is_solid() {
            return Err(GeometryError::InvalidVolumeMesh);
        }
        let mut volume = Cubics::default();
        let mut first: [Quartics; 3] = std::array::from_fn(|_| Quartics::default());
        for points in self.mass_triangles() {
            let points = points.map(|p| p.to_array());
            // Six determinant monomials; tetrahedron (0,a,b,c) has volume
            // det/6 and centroid (a+b+c)/4, so first moments divide by 24.
            for [i, j, k] in [
                [0, 1, 2],
                [1, 2, 0],
                [2, 0, 1],
                [0, 2, 1],
                [1, 0, 2],
                [2, 1, 0],
            ] {
                let sign = if (j + 1) % 3 == k { 1. } else { -1. };
                let factors = [sign * points[0][i], points[1][j], points[2][k]];
                volume.add(factors)?;
                for (axis, sum) in first.iter_mut().enumerate() {
                    for point in points {
                        sum.add([factors[0], factors[1], factors[2], point[axis]])?;
                    }
                }
            }
        }
        let result = VolumeMassProperties {
            volume: volume.total() / Rational::from_integer(6.into()),
            first: first.map(|m| m.total() / Rational::from_integer(24.into())),
        };
        Ok(result)
    }
}
