//! Normalized planar charts shared by mesh and SubD region capping.
use super::projected_polygon_doubled_area;
use crate::{GeometryError, Point3, Real, require_finite};

pub(crate) struct PolygonProjection {
    origin: [Real; 3],
    global_scale: Option<Real>,
    scale: Real,
    axes: [usize; 2],
}

impl PolygonProjection {
    pub(crate) fn axes(&self) -> [usize; 2] {
        self.axes
    }

    pub(crate) fn new(points: &[Point3]) -> Result<Option<Self>, GeometryError> {
        let origin = points[0].to_array();
        let global_scale = points
            .iter()
            .flat_map(|point| point.to_array())
            .map(Real::abs)
            .fold(0.0, Real::max);
        let direct = points.iter().all(|point| {
            point
                .to_array()
                .into_iter()
                .zip(origin)
                .all(|(value, origin)| (value - origin).is_finite())
        });
        let global_scale = if direct { None } else { Some(global_scale) };
        let mut frame = Self {
            origin,
            global_scale,
            scale: 1.0,
            axes: [0, 1],
        };
        let relative = points
            .iter()
            .copied()
            .map(|point| frame.relative(point))
            .collect::<Vec<_>>();
        frame.scale = relative
            .iter()
            .flatten()
            .map(|value| value.abs())
            .fold(0.0, Real::max);
        if frame.scale == 0.0 {
            return Ok(None);
        }
        let normalized = relative
            .iter()
            .map(|point| point.map(|value| value / frame.scale))
            .collect::<Vec<_>>();
        let mut best = 0.0;
        for axes in [[0, 1], [1, 2], [2, 0]] {
            let polygon = normalized
                .iter()
                .map(|point| [point[axes[0]], point[axes[1]]])
                .collect::<Vec<_>>();
            let area = projected_polygon_doubled_area(&polygon).abs();
            if area > best {
                best = area;
                frame.axes = axes;
            }
        }
        if best <= 64.0 * Real::EPSILON * points.len() as Real {
            return Ok(None);
        }
        Ok(Some(frame))
    }

    fn relative(&self, point: Point3) -> [Real; 3] {
        let values = point.to_array();
        std::array::from_fn(|axis| {
            if let Some(scale) = self.global_scale {
                values[axis] / scale - self.origin[axis] / scale
            } else {
                values[axis] - self.origin[axis]
            }
        })
    }

    pub(crate) fn project(&self, point: Point3) -> Result<[Real; 2], GeometryError> {
        let relative = self.relative(point);
        let projected = [
            relative[self.axes[0]] / self.scale,
            relative[self.axes[1]] / self.scale,
        ];
        require_finite(projected, "mesh planar cap projection")?;
        Ok(projected)
    }
}
