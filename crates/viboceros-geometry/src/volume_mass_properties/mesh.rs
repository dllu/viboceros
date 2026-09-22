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
        self.volume_flux_impl::<false>([0.; 3])
    }

    /// Exact signed cone volume and first moments about a caller-chosen base.
    /// Accepts open and inconsistently oriented meshes; only an enclosing,
    /// consistently oriented collection is an actual volume distribution.
    /// Use the SAME base when adding separate boundary pieces. No subtraction
    /// is rounded, even when a coordinate difference would overflow binary64.
    pub fn volume_flux(&self, base: Point3) -> Result<VolumeMassProperties, GeometryError> {
        let base = base.to_array();
        if base == [0.; 3] {
            self.volume_flux_impl::<false>(base)
        } else {
            self.volume_flux_impl::<true>(base)
        }
    }

    // Specialize the closed/origin path so reference support adds no per-term
    // tests or extra determinant columns to ordinary closed-solid integration.
    fn volume_flux_impl<const OFFSET: bool>(
        &self,
        base: [Real; 3],
    ) -> Result<VolumeMassProperties, GeometryError> {
        let mut volume = Cubics::default();
        let mut first: [Quartics; 3] = std::array::from_fn(|_| Quartics::default());
        for points in self.mass_triangles() {
            let points = points.map(|p| p.to_array());
            // det(a-o,b-o,c-o) = det(a,b,c) - det(o,b,c)
            //                  - det(a,o,c) - det(a,b,o).
            // Terms with two equal o columns cancel algebraically, before any
            // rounded arithmetic. The cone centroid is (o+a+b+c)/4.
            let terms = [
                (points, 1.),
                ([base, points[1], points[2]], -1.),
                ([points[0], base, points[2]], -1.),
                ([points[0], points[1], base], -1.),
            ];
            let count = if OFFSET { 4 } else { 1 };
            for (columns, orientation) in terms.into_iter().take(count) {
                for [i, j, k] in [
                    [0, 1, 2],
                    [1, 2, 0],
                    [2, 0, 1],
                    [0, 2, 1],
                    [1, 0, 2],
                    [2, 1, 0],
                ] {
                    let sign = orientation * if (j + 1) % 3 == k { 1. } else { -1. };
                    let factors = [sign * columns[0][i], columns[1][j], columns[2][k]];
                    volume.add(factors)?;
                    for (axis, sum) in first.iter_mut().enumerate() {
                        for point in points.into_iter().chain(OFFSET.then_some(base)) {
                            sum.add([factors[0], factors[1], factors[2], point[axis]])?;
                        }
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
