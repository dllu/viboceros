use super::*;
use crate::smoothing::{
    mean, projected_step, projection_matrix, smooth_graph, validate_selection, world_frame,
};
use crate::{Frame3, SmoothingCoordinates, SmoothingOptions};
use std::collections::BTreeSet;

impl NurbsCurve {
    /// Averages the Euclidean control polygon in a fixed orthonormal frame.
    /// Retains weights, knots, degree and native parameter intervals.
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
        for (p, point) in result.control_points.iter_mut().zip(points) {
            *p = WeightedPoint3::try_new(point, p.weight())?;
        }
        Ok(result)
    }

    /// Averages controls using fixed World/CPlane or progressively evaluated
    /// Object axes. Object axes address raw controls, including seam aliases.
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
            for i in 0..before.len() {
                if !movable[i] {
                    continue;
                }
                let target = mean(&before, &neighbors[i])?;
                let frame = if result.degree == 1 {
                    world_frame()
                } else {
                    let t = result.control_greville_parameter(i)?;
                    let (_, first, second) = result.evaluate_extended_with_second_derivative(t)?;
                    let directions = (|| {
                        let u = first.normalized_nonzero()?.as_vector();
                        let second = second.normalized_nonzero()?.as_vector();
                        let binormal = u.cross(second)?.normalized_nonzero()?.as_vector();
                        Frame3::try_from_directions(
                            before[i],
                            u,
                            binormal,
                            Tolerance::MESH_VALIDATION,
                        )
                    })();
                    match directions {
                        Ok(frame) => frame,
                        Err(GeometryError::Degenerate { .. }) => world_frame(),
                        Err(e) => return Err(e),
                    }
                };
                let point = projected_step(
                    before[i],
                    target,
                    options.factor,
                    projection_matrix(options.axes, frame),
                )?;
                result.control_points[i] =
                    WeightedPoint3::try_new(point, result.control_points[i].weight())?;
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
        let unique = self.grip_count()?;
        validate_selection(selected, unique)?;
        let count = self.control_points.len();
        let cyclic = unique < count;
        let mut movable = Vec::with_capacity(count);
        let mut neighbors = Vec::with_capacity(count);
        for i in 0..count {
            let node = i % unique;
            let boundary = if cyclic {
                node < count - unique
            } else {
                i == 0 || i + 1 == count
            };
            movable.push(
                selected.is_none_or(|s| s.contains(&node)) && !(options.fix_boundaries && boundary),
            );
            neighbors.push(if cyclic {
                // Keep raw interior adjacency: Object edits can separate aliases.
                // Only the two ends wrap through the original unique polygon.
                vec![
                    if i == 0 { unique - 1 } else { i - 1 },
                    if i + 1 == count {
                        count - unique
                    } else {
                        i + 1
                    },
                ]
            } else if i == 0 {
                vec![1]
            } else if i + 1 == count {
                vec![i - 1]
            } else {
                vec![i - 1, i + 1]
            });
        }
        Ok((neighbors, movable))
    }
}
