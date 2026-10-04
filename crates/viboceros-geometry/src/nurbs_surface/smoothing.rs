use super::*;
use crate::smoothing::{
    mean, projected_step, projection_matrix, smooth_graph, validate_selection, world_frame,
};
use crate::{SmoothingCoordinates, SmoothingOptions};
use std::collections::BTreeSet;

impl NurbsSurface {
    /// Synchronously averages the Euclidean control net in the supplied axes.
    /// Selected indices use the public grip order (U major, V minor), with
    /// closed and periodic aliases addressed through their unique grip.
    pub fn try_smoothed(
        &self,
        options: SmoothingOptions,
        frame: Frame3,
        selected: Option<&BTreeSet<usize>>,
    ) -> Result<Self, GeometryError> {
        options.validate()?;
        let (neighbors, movable) = self.smoothing_graph(options, selected)?;
        let points = smooth_graph(
            self.control_points.iter().map(|p| p.point()).collect(),
            &neighbors,
            &movable,
            options,
            frame,
        )?;
        let mut result = self.clone();
        for (control, point) in result.control_points.iter_mut().zip(points) {
            *control = WeightedPoint3::try_new(point, control.weight())?;
        }
        Ok(result)
    }

    /// Object frames are evaluated at control Greville stations on the
    /// progressively edited surface, in U-major/V-minor order.
    pub fn try_smoothed_in(
        &self,
        options: SmoothingOptions,
        coordinates: SmoothingCoordinates,
        selected: Option<&BTreeSet<usize>>,
    ) -> Result<Self, GeometryError> {
        match coordinates {
            SmoothingCoordinates::World => {
                return self.try_smoothed(options, world_frame(), selected);
            }
            SmoothingCoordinates::CPlane(frame) => {
                return self.try_smoothed(options, frame, selected);
            }
            SmoothingCoordinates::Object => {}
        }
        options.validate()?;
        let (neighbors, movable) = self.smoothing_graph(options, selected)?;
        let mut result = self.clone();
        if options.factor == 0.0 || !options.axes.contains(&true) {
            return Ok(result);
        }
        for _ in 0..options.steps {
            let before = result
                .control_points
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>();
            for u in 0..self.control_point_count_u {
                for v in 0..self.control_point_count_v {
                    let i = self.control_index(u, v);
                    if !movable[i] {
                        continue;
                    }
                    let a = stable_knot_mean(&self.knots_u[u + 1..u + 1 + self.degree_u])?;
                    let b = stable_knot_mean(&self.knots_v[v + 1..v + 1 + self.degree_v])?;
                    let (_, du, dv) = result.evaluate_extended_with_derivatives(a, b)?;
                    let directions = (|| {
                        let du = du.normalized_nonzero()?.as_vector();
                        let dv = dv.normalized_nonzero()?.as_vector();
                        let normal = du.cross(dv)?.normalized_nonzero()?.as_vector();
                        Frame3::try_from_directions(
                            before[i],
                            dv.cross(normal)?,
                            dv,
                            Tolerance::MESH_VALIDATION,
                        )
                    })();
                    let frame = match directions {
                        Ok(frame) => frame,
                        Err(GeometryError::Degenerate { .. }) => world_frame(),
                        Err(e) => return Err(e),
                    };
                    let point = projected_step(
                        before[i],
                        mean(&before, &neighbors[i])?,
                        options.factor,
                        projection_matrix(options.axes, frame),
                    )?;
                    result.control_points[i] =
                        WeightedPoint3::try_new(point, result.control_points[i].weight())?;
                }
            }
            if result.control_points.iter().map(|p| p.point()).eq(before) {
                break;
            }
        }
        Ok(result)
    }

    fn smoothing_graph(
        &self,
        options: SmoothingOptions,
        selected: Option<&BTreeSet<usize>>,
    ) -> Result<(Vec<Vec<usize>>, Vec<bool>), GeometryError> {
        let [nu, nv] = [self.control_point_count_u, self.control_point_count_v];
        let [gu, gv] = self.grip_dimensions();
        validate_selection(selected, gu * gv)?;
        let mut neighbors = vec![Vec::with_capacity(4); nu * nv];
        let mut movable = vec![false; nu * nv];
        for v in 0..nv {
            for u in 0..nu {
                let index = self.control_index(u, v);
                let [a, b] = [u % gu, v % gv];
                let boundary_u = if gu < nu {
                    a < nu - gu
                } else {
                    a == 0 || a + 1 == nu
                };
                let boundary_v = if gv < nv {
                    b < nv - gv
                } else {
                    b == 0 || b + 1 == nv
                };
                let boundary = boundary_u || boundary_v;
                movable[index] = selected.is_none_or(|s| s.contains(&(a * gv + b)))
                    && !(options.fix_boundaries && boundary);
                for (direction, coordinate, unique, count) in [(0, u, gu, nu), (1, v, gv, nv)] {
                    for offset in [-1isize, 1] {
                        let adjacent = coordinate
                            .checked_add_signed(offset)
                            .filter(|&i| i < count)
                            .or_else(|| {
                                (unique < count).then_some(if offset < 0 {
                                    unique - 1
                                } else {
                                    count - unique
                                })
                            });
                        if let Some(next) = adjacent {
                            neighbors[index].push(if direction == 0 {
                                self.control_index(next, v)
                            } else {
                                self.control_index(u, next)
                            });
                        }
                    }
                }
            }
        }
        Ok((neighbors, movable))
    }
}
