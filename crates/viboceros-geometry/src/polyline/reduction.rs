//! Iterative Douglas-Peucker reduction, with no recursion-depth dependency.
//! Public Rhino3dm C# behavior is retained under third_party/rhino3dm-polyline
//! (MIT, Copyright (c) 2023 Robert McNeel & Associates).
use super::*;

impl Polyline3 {
    /// Remove insignificant vertices using finite-chord distance. The existing
    /// endpoints and seam are retained; output has unit vertex parameters.
    /// Invalid collapsed curves are rejected instead of entering the model.
    pub fn try_reduced(&self, tolerance: Real) -> Result<Self, GeometryError> {
        if !tolerance.is_finite() || tolerance < 0. {
            return Err(GeometryError::InvalidCurveFitTolerance);
        }
        let points = self.vertices();
        if points.len() < 4 && points.first() == points.last() {
            return Err(GeometryError::CollapsedPolylineReduction);
        }
        // Keep native-recognized closed triangles, without changing endpoints.
        let sdk_closed = self.is_closed();
        if points.len() < 5 && sdk_closed {
            return Self::try_new(points.to_vec(), Tolerance::NUMERICAL_VALIDATION);
        }
        let mut retained = vec![false; points.len()];
        retained[0] = true;
        retained[points.len() - 1] = true;
        let mut stack = vec![(0, points.len() - 1)];
        while let Some((start, end)) = stack.pop() {
            if end - start < 2 {
                continue;
            }
            let chord = if points[start] == points[end] {
                None
            } else {
                // A derived chord can be subnormal even when every original
                // segment is regular. It still defines a projection direction.
                let line = LineSegment::from_validated(points[start], points[end], [0., 1.]);
                Some((
                    line,
                    points[start].direction_to(points[end])?.as_vector(),
                    line.length()?,
                ))
            };
            let mut maximum = tolerance;
            let mut split = None;
            for i in start + 1..end {
                let closest = match chord {
                    Some((line, direction, length)) => {
                        let parameter = (direction.dot_point_difference(points[i], points[start])
                            / length)
                            .clamp(0., 1.);
                        line.point_at(parameter)?
                    }
                    None => points[start],
                };
                let distance = points[i].distance_to(closest)?;
                if distance > maximum {
                    maximum = distance;
                    split = Some(i);
                }
            }
            if let Some(i) = split {
                retained[i] = true;
                stack.push((i, end));
                stack.push((start, i));
            }
        }
        let vertices = points
            .iter()
            .zip(retained)
            .filter_map(|(&p, keep)| keep.then_some(p))
            .collect::<Vec<_>>();
        if vertices.first() == vertices.last() && vertices.len() < 4 {
            return Err(GeometryError::CollapsedPolylineReduction);
        }
        Self::try_new(vertices, Tolerance::NUMERICAL_VALIDATION)
    }
}
