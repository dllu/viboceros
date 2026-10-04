use super::*;
use crate::smoothing::{smooth_graph, validate_selection};
use crate::{Frame3, SmoothingOptions};
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
                vec![(node + unique - 1) % unique, (node + 1) % unique]
            } else if i == 0 {
                vec![1]
            } else if i + 1 == count {
                vec![i - 1]
            } else {
                vec![i - 1, i + 1]
            });
        }
        let points = smooth_graph(
            self.control_points.iter().map(|p| p.point()).collect(),
            &neighbors,
            &movable,
            options,
            frame,
        )?;
        Self::try_new_rational(
            self.degree,
            self.control_points
                .iter()
                .zip(points)
                .map(|(p, point)| WeightedPoint3::try_new(point, p.weight()))
                .collect::<Result<Vec<_>, _>>()?,
            self.knots.clone(),
        )
    }
}
