//! Synchronous control and vertex averaging, independent of document history.
use crate::{Frame3, GeometryError, Point3, Real, require_finite};
use std::collections::BTreeSet;

/// Coordinates used to restrict an averaging displacement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SmoothingCoordinates {
    World,
    CPlane(Frame3),
    Object,
}

pub(crate) fn world_frame() -> Frame3 {
    Frame3::try_from_directions(
        Point3::try_from([0.; 3]).unwrap(),
        crate::Vector3::try_from([1., 0., 0.]).unwrap(),
        crate::Vector3::try_from([0., 1., 0.]).unwrap(),
        crate::Tolerance::MESH_VALIDATION,
    )
    .unwrap()
}

#[cfg(test)]
mod tests;

/// Point averaging settings shared by curve, surface and mesh edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmoothingOptions {
    pub factor: Real,
    pub steps: usize,
    pub axes: [bool; 3],
    pub fix_boundaries: bool,
}

impl Default for SmoothingOptions {
    fn default() -> Self {
        Self {
            factor: 0.2,
            steps: 1,
            axes: [true; 3],
            fix_boundaries: true,
        }
    }
}

impl SmoothingOptions {
    pub fn validate(self) -> Result<(), GeometryError> {
        require_finite([self.factor], "Smooth factor")?;
        if self.steps == 0 {
            return Err(GeometryError::Degenerate {
                context: "Smooth steps",
            });
        }
        Ok(())
    }
}

pub(crate) fn validate_selection(
    selected: Option<&BTreeSet<usize>>,
    count: usize,
) -> Result<(), GeometryError> {
    if let Some(&index) = selected.and_then(|s| s.last()).filter(|&&i| i >= count) {
        return Err(GeometryError::InvalidControlPointIndex { index, count });
    }
    Ok(())
}

pub(crate) fn mean(points: &[Point3], indices: &[usize]) -> Result<Point3, GeometryError> {
    if indices.is_empty() {
        return Err(GeometryError::EmptyPointSet);
    }
    if let [a, b] = indices {
        return points[*a].midpoint(points[*b]);
    }
    let mut coordinates = [0.0; 3];
    for (axis, coordinate) in coordinates.iter_mut().enumerate() {
        let mut sum = 0.0;
        let mut correction = 0.0;
        let mut absolute = 0.0;
        for &i in indices {
            let value = points[i].to_array()[axis];
            let next = sum + value;
            correction += if sum.abs() >= value.abs() {
                (sum - next) + value
            } else {
                (value - next) + sum
            };
            sum = next;
            absolute += value.abs();
        }
        sum += correction;
        let result = sum / indices.len() as Real;
        *coordinate = if absolute == 0.0 {
            0.0
        } else if sum.is_finite()
            && absolute.is_finite()
            && result.is_normal()
            && (sum.abs() >= absolute * Real::EPSILON || absolute == 0.0)
        {
            result
        } else {
            let mut exact = crate::FiniteSum::default();
            for &i in indices {
                exact.add(points[i].to_array()[axis])?;
            }
            exact.mean()?
        };
    }
    Point3::try_from(coordinates)
}

pub(crate) fn projection_matrix(axes: [bool; 3], frame: Frame3) -> [[Real; 3]; 3] {
    if axes == [true; 3] {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    } else {
        let directions = frame.axes().map(|a| a.as_vector().to_array());
        std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                (0..3)
                    .filter(|&i| axes[i])
                    .map(|i| directions[i][r] * directions[i][c])
                    .sum()
            })
        })
    }
}

pub(crate) fn projected_step(
    point: Point3,
    target: Point3,
    factor: Real,
    matrix: [[Real; 3]; 3],
) -> Result<Point3, GeometryError> {
    if factor == 0.0 || target == point {
        return Ok(point);
    }
    let p = point.to_array();
    let t = target.to_array();
    let delta = std::array::from_fn::<_, 3, _>(|i| t[i] - p[i]);
    let mut result = [0.0; 3];
    for r in 0..3 {
        if matrix[r] == std::array::from_fn(|c| if c == r { 1.0 } else { 0.0 })
            && (0.0..=1.0).contains(&factor)
        {
            result[r] = crate::interpolate_scalar([p[r], t[r]], [0.0, 1.0], factor)?;
            continue;
        }
        let projected = crate::Vector3::try_from(matrix[r])?.dot_point_difference(target, point);
        let next = factor.mul_add(projected, p[r]);
        let tiny_product = (0..3).any(|c| {
            matrix[r][c] != 0.0
                && delta[c] != 0.0
                && (matrix[r][c] * delta[c]).abs() <= 2.0 * Real::MIN_POSITIVE / Real::EPSILON
        });
        let cancellation = next.abs() < p[r].abs().max((factor * projected).abs()) * 0.125;
        result[r] = if next.is_finite()
            && !tiny_product
            && !cancellation
            && delta.iter().all(|d| d.is_finite())
        {
            next
        } else {
            // A finite final coordinate can have an overflowing displacement.
            let projection = (0..3).fold(crate::exact_scalar::rational(0.0), |sum, c| {
                sum + crate::exact_scalar::rational(matrix[r][c])
                    * (crate::exact_scalar::rational(t[c]) - crate::exact_scalar::rational(p[c]))
            });
            crate::exact_scalar::scalar(
                &(crate::exact_scalar::rational(p[r])
                    + crate::exact_scalar::rational(factor) * projection),
            )?
        };
    }
    Point3::try_from(result)
}

pub(crate) fn smooth_graph(
    mut points: Vec<Point3>,
    neighbors: &[Vec<usize>],
    movable: &[bool],
    options: SmoothingOptions,
    frame: Frame3,
) -> Result<Vec<Point3>, GeometryError> {
    options.validate()?;
    if options.factor == 0.0 || !options.axes.contains(&true) || !movable.contains(&true) {
        return Ok(points);
    }
    let matrix = projection_matrix(options.axes, frame);
    let mut next = points.clone();
    for _ in 0..options.steps {
        let mut changed = false;
        for (i, &point) in points.iter().enumerate() {
            next[i] = if movable[i] && !neighbors[i].is_empty() {
                projected_step(point, mean(&points, &neighbors[i])?, options.factor, matrix)?
            } else {
                point
            };
            changed |= next[i] != point;
        }
        if !changed {
            return Ok(next);
        }
        std::mem::swap(&mut points, &mut next);
    }
    Ok(points)
}
