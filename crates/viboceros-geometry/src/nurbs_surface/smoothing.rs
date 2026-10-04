use super::*;
use crate::SmoothingOptions;
use crate::smoothing::{smooth_graph, validate_selection};
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
                for (direction, coordinate, unique, count) in [(0, a, gu, nu), (1, b, gv, nv)] {
                    for offset in [-1isize, 1] {
                        let adjacent = if unique < count {
                            Some((coordinate as isize + offset).rem_euclid(unique as isize) as usize)
                        } else {
                            coordinate
                                .checked_add_signed(offset)
                                .filter(|&i| i < unique)
                        };
                        if let Some(next) = adjacent {
                            neighbors[index].push(if direction == 0 {
                                self.control_index(next, b)
                            } else {
                                self.control_index(a, next)
                            });
                        }
                    }
                }
            }
        }
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
}
