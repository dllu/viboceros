use super::super::interpolation::{Axis, knots};
use super::*;
use faer::Mat;

pub(super) fn interpolate(
    point_at: &mut impl FnMut([Real; 2], [ParameterSide; 2]) -> Result<Point3, GeometryError>,
    breaks: &[Vec<Break>; 2],
) -> Result<NurbsSurface, GeometryError> {
    Grid::sample(
        Axis::cubic(knots(&breaks[0]))?,
        Axis::cubic(knots(&breaks[1]))?,
        point_at,
    )?
    .solve(None)
}

/// A cubic polynomial in XYZ composed with N/W has denominator W³ and
/// numerator degree at most three times each source degree. This is only a
/// candidate space, not an assumption that the supplied point map is cubic.
pub(super) fn rational_candidate(
    point_at: &mut impl FnMut([Real; 2], [ParameterSide; 2]) -> Result<Point3, GeometryError>,
    source: &NurbsSurface,
    maximum: usize,
) -> Result<Option<NurbsSurface>, GeometryError> {
    let Some(denominator) = super::denominator::surface_weights(source) else {
        return Ok(None);
    };
    let Some(u) = Axis::cubic_composition(
        source.degree_u(),
        source.knots_u(),
        source.domain_u(),
        maximum,
    ) else {
        return Ok(None);
    };
    let Some(v) = Axis::cubic_composition(
        source.degree_v(),
        source.knots_v(),
        source.domain_v(),
        maximum,
    ) else {
        return Ok(None);
    };
    let mut weights = Vec::with_capacity(u.stations.len() * v.stations.len());
    for v in &v.stations {
        for u in &u.stations {
            let weight = denominator
                .evaluate_on_sides(u.parameter, v.parameter, u.side, v.side)?
                .x()
                .powi(3);
            if weight == 0.0 || !weight.is_finite() {
                return Ok(None);
            }
            weights.push(weight);
        }
    }
    // Construct the optional space and check its weights before any point
    // maps. Mapping failures propagate; numerical solve failures only discard
    // this candidate. Both paths use the same cached source evaluations.
    let grid = Grid::sample(u, v, point_at)?;
    Ok(grid.solve(Some(&weights)).ok())
}

struct Grid {
    u: Axis,
    v: Axis,
    targets: Vec<Point3>,
}

impl Grid {
    fn sample(
        u: Axis,
        v: Axis,
        point_at: &mut impl FnMut([Real; 2], [ParameterSide; 2]) -> Result<Point3, GeometryError>,
    ) -> Result<Self, GeometryError> {
        let mut targets = Vec::with_capacity(u.stations.len() * v.stations.len());
        for v in &v.stations {
            for u in &u.stations {
                targets.push(point_at([u.parameter, v.parameter], [u.side, v.side])?);
            }
        }
        Ok(Self { u, v, targets })
    }

    fn solve(&self, weights: Option<&[Real]>) -> Result<NurbsSurface, GeometryError> {
        let (count_u, count_v) = (self.u.stations.len(), self.v.stations.len());
        let width = if weights.is_some() { 4 } else { 3 };
        let candidate = self.targets[0].to_array();
        let origin = if self.targets.iter().all(|p| {
            p.to_array()
                .into_iter()
                .zip(candidate)
                .all(|(a, b)| (a - b).is_finite())
        }) {
            candidate
        } else {
            [0.0; 3]
        };
        // A_u C A_v^T = P. Solve all XYZ(W)/V right-hand sides together in U,
        // then transpose and solve all XYZ(W)/U right-hand sides in V.
        let rhs_u = Mat::from_fn(count_u, count_v * width, |u, column| {
            let (v, axis) = (column / width, column % width);
            let weight = weights.map_or(1.0, |w| w[v * count_u + u]);
            if axis == 3 {
                weight
            } else {
                (self.targets[v * count_u + u].to_array()[axis] - origin[axis]) * weight
            }
        });
        let solved_u = self.u.solve(rhs_u)?;
        let rhs_v = Mat::from_fn(count_v, count_u * width, |v, column| {
            let (u, axis) = (column / width, column % width);
            solved_u[(u, v * width + axis)]
        });
        let solved_v = self.v.solve(rhs_v)?;
        let mut controls = Vec::with_capacity(count_u * count_v);
        for (v, station_v) in self.v.stations.iter().enumerate() {
            for (u, station_u) in self.u.stations.iter().enumerate() {
                let weight = if weights.is_some() {
                    solved_v[(v, u * width + 3)]
                } else {
                    1.0
                };
                if weight <= 0.0 || !weight.is_finite() {
                    return Err(GeometryError::ZeroWeightAtParameter);
                }
                let point = if station_u.fixed && station_v.fixed {
                    self.targets[v * count_u + u]
                } else {
                    Point3::try_from(std::array::from_fn(|axis| {
                        solved_v[(v, u * width + axis)] / weight + origin[axis]
                    }))?
                };
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
        }
        NurbsSurface::try_new_rational(
            self.u.degree,
            self.v.degree,
            count_u,
            count_v,
            controls,
            self.u.knots.clone(),
            self.v.knots.clone(),
        )
    }
}
