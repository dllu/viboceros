//! Local homogeneous span extraction and tensor subdivision for tight boxes.
//! Intermediate zero weights are valid projective controls, not curve poles.
use crate::{BoundingBox3, GeometryError, Point3, Tolerance, WeightedPoint3};
mod compose;
#[cfg(test)]
mod tests;

pub(super) const MAX_NODES: usize = 131_072;
const MAX_INITIAL_CONTROLS: usize = 1_048_576;
const MAX_WORK: usize = 33_554_432;
pub(super) const MAX_DEPTH: u8 = 64;
type H = [f64; 4];

#[derive(Default)]
pub(super) struct Budget {
    work: usize,
    initial_controls: usize,
    visited: usize,
}

impl Budget {
    pub(super) fn visit(&mut self) -> Result<(), GeometryError> {
        self.visited = self.visited.saturating_add(1);
        if self.visited > MAX_NODES {
            Err(GeometryError::BoundingBoxDidNotConverge)
        } else {
            Ok(())
        }
    }
    pub(super) fn initial(&mut self, count: usize) -> Result<(), GeometryError> {
        self.initial_controls = self.initial_controls.saturating_add(count);
        if self.initial_controls > MAX_INITIAL_CONTROLS {
            return Err(GeometryError::BoundingBoxDidNotConverge);
        }
        self.charge(count)
    }

    pub(super) fn charge(&mut self, count: usize) -> Result<(), GeometryError> {
        self.work = self.work.saturating_add(count);
        if self.work > MAX_WORK {
            Err(GeometryError::BoundingBoxDidNotConverge)
        } else {
            Ok(())
        }
    }
}

/// One native knot rectangle (or curve span when degree V is zero).
pub(super) struct Net {
    pub(super) degrees: [usize; 2],
    pub(super) origin: [f64; 3],
    pub(super) controls: Vec<H>,
    pub(super) depth: u8,
}

impl Net {
    pub(super) fn new(
        degrees: [usize; 2],
        controls: &[WeightedPoint3],
    ) -> Result<Self, GeometryError> {
        let candidate = controls[0].point().to_array();
        let origin = if controls.iter().all(|c| {
            c.point()
                .to_array()
                .into_iter()
                .zip(candidate)
                .all(|(a, b)| (a - b).is_finite())
        }) {
            candidate
        } else {
            [0.; 3]
        };
        Self::new_at_origin(degrees, controls, origin)
    }

    pub(super) fn new_at_origin(
        degrees: [usize; 2],
        controls: &[WeightedPoint3],
        origin: [f64; 3],
    ) -> Result<Self, GeometryError> {
        let scale = controls.iter().map(|c| c.weight().abs()).fold(0., f64::max);
        let controls = controls
            .iter()
            .map(|c| {
                let w = c.weight() / scale;
                if w == 0. {
                    return Err(GeometryError::BoundingBoxDidNotConverge);
                }
                let p = c.point().to_array();
                Ok([
                    (p[0] - origin[0]) * w,
                    (p[1] - origin[1]) * w,
                    (p[2] - origin[2]) * w,
                    w,
                ])
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            degrees,
            origin,
            controls,
            depth: 0,
        })
    }

    pub(super) fn extract_axis(
        &mut self,
        axis: usize,
        knots: &[f64],
        span: usize,
        budget: &mut Budget,
    ) -> Result<(), GeometryError> {
        let degree = self.degrees[axis];
        let a = knots[span];
        let b = knots[span + 1];
        // A fully isolated Bezier span already has the required controls.
        if knots[span - degree..=span].iter().all(|x| *x == a)
            && knots[span + 1..=span + degree + 1].iter().all(|x| *x == b)
        {
            return Ok(());
        }
        let lines = self.degrees[1 - axis] + 1;
        budget.charge(lines.saturating_mul((degree + 1).saturating_pow(3)))?;
        for line in 0..lines {
            let source = (0..=degree)
                .map(|i| self.controls[self.index(axis, line, i)])
                .collect::<Vec<_>>();
            let mut work = source.clone();
            for end_arguments in 0..=degree {
                work.copy_from_slice(&source);
                // Blossom: p-i copies of the left endpoint and i of the right.
                // Keeping all four coordinates avoids projection at inserted
                // zero-weight controls and covers unclamped/full-order spans.
                for level in 1..=degree {
                    let t = if level <= degree - end_arguments {
                        a
                    } else {
                        b
                    };
                    for j in (level..=degree).rev() {
                        let k = span - degree + j;
                        let alpha = crate::nurbs::interval_fraction(
                            t,
                            knots[k],
                            knots[k + degree - level + 1],
                        )?;
                        work[j] = blend(work[j - 1], work[j], alpha);
                    }
                }
                let index = self.index(axis, line, end_arguments);
                self.controls[index] = work[degree];
            }
        }
        Ok(())
    }

    fn index(&self, axis: usize, line: usize, i: usize) -> usize {
        if axis == 0 {
            line * (self.degrees[0] + 1) + i
        } else {
            i * (self.degrees[0] + 1) + line
        }
    }

    pub(super) fn project(&self, h: H) -> Result<Point3, GeometryError> {
        if h[3] == 0. {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        Point3::try_from(std::array::from_fn(|i| {
            let coordinate = h[i] / h[3] + self.origin[i];
            if coordinate.is_finite() {
                coordinate
            } else {
                // A signed rational curve can leave its control hull. Its
                // local offset may overflow while the final world coordinate
                // remains finite; combine in homogeneous space in that case.
                self.origin[i].mul_add(h[3], h[i]) / h[3]
            }
        }))
    }

    fn corners(&self) -> Result<BoundingBox3, GeometryError> {
        let [p, q] = self.degrees;
        let indices = [0, p, q * (p + 1), (q + 1) * (p + 1) - 1];
        let points = indices.map(|i| self.project(self.controls[i]));
        BoundingBox3::from_points(points.into_iter().collect::<Result<Vec<_>, _>>()?)
    }

    pub(super) fn center(&self) -> Result<Point3, GeometryError> {
        let width = self.degrees[0] + 1;
        let mut rows = self
            .controls
            .chunks_exact(width)
            .map(midpoint)
            .collect::<Vec<_>>();
        self.project(midpoint_in_place(&mut rows))
    }

    pub(super) fn hull(&self) -> Option<BoundingBox3> {
        let first = self.controls[0][3];
        if first == 0.
            || self
                .controls
                .iter()
                .any(|h| h[3] == 0. || h[3].is_sign_positive() != first.is_sign_positive())
        {
            return None;
        }
        let points = self
            .controls
            .iter()
            .map(|h| self.project(*h))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        BoundingBox3::from_points(points).ok()
    }

    fn split_axis(
        &self,
        hull: Option<BoundingBox3>,
        attained: BoundingBox3,
        tolerance: Tolerance,
    ) -> usize {
        if self.degrees[1] == 0 {
            return 0;
        }
        let mut score = [0_f64; 2];
        for (axis, axis_score) in score.iter_mut().enumerate() {
            for line in 0..=self.degrees[1 - axis] {
                for i in 0..self.degrees[axis] {
                    let a = self.controls[self.index(axis, line, i)];
                    let b = self.controls[self.index(axis, line, i + 1)];
                    if let Some(hull) = hull {
                        for j in 0..3 {
                            if !resolved_axis(hull, attained, tolerance, j) {
                                *axis_score = axis_score.max((a[j] / a[3] - b[j] / b[3]).abs());
                            }
                        }
                    } else {
                        // First resolve denominator signs. This also avoids
                        // subdividing a flat extrusion in the constant axis.
                        *axis_score = axis_score.max((a[3] - b[3]).abs());
                    }
                }
            }
        }
        if score[0] == score[1] {
            usize::from(self.depth % 2)
        } else {
            usize::from(score[1] > score[0])
        }
    }

    pub(super) fn split(self, axis: usize) -> (Self, Self) {
        let mut left = self.controls.clone();
        let mut right = self.controls.clone();
        let degree = self.degrees[axis];
        let mut work = vec![[0.; 4]; degree + 1];
        for line in 0..=self.degrees[1 - axis] {
            for (i, h) in work.iter_mut().enumerate() {
                *h = self.controls[self.index(axis, line, i)];
            }
            for level in 1..=degree {
                for i in 0..=degree - level {
                    work[i] = blend(work[i], work[i + 1], 0.5);
                }
                left[self.index(axis, line, level)] = work[0];
                right[self.index(axis, line, degree - level)] = work[degree - level];
            }
        }
        (
            Self {
                controls: left,
                depth: self.depth + 1,
                ..self
            },
            Self {
                controls: right,
                depth: self.depth + 1,
                ..self
            },
        )
    }
}

fn blend(a: H, b: H, t: f64) -> H {
    if t == 0. {
        return a;
    }
    if t == 1. {
        return b;
    }
    std::array::from_fn(|i| a[i].mul_add(1. - t, b[i] * t))
}

fn midpoint(controls: &[H]) -> H {
    midpoint_in_place(&mut controls.to_vec())
}

fn midpoint_in_place(work: &mut [H]) -> H {
    for level in 1..work.len() {
        for i in 0..work.len() - level {
            work[i] = blend(work[i], work[i + 1], 0.5);
        }
    }
    work[0]
}

pub(super) fn bounds(
    mut nodes: Vec<Net>,
    budget: &mut Budget,
    tolerance: Tolerance,
) -> Result<BoundingBox3, GeometryError> {
    if nodes.len() > MAX_NODES {
        return Err(GeometryError::BoundingBoxDidNotConverge);
    }
    let mut attained = None;
    for node in &nodes {
        merge(&mut attained, node.corners()?)?;
    }
    let mut enclosure = None;
    while let Some(node) = nodes.pop() {
        budget.visit()?;
        merge(&mut attained, node.corners()?)?;
        let hull = node.hull();
        if let Some(hull) = hull
            && resolved(hull, attained.unwrap(), tolerance)
        {
            merge(&mut enclosure, hull)?;
            continue;
        }
        let work = node.controls.len().saturating_mul(
            node.degrees[0]
                .saturating_add(node.degrees[1])
                .saturating_add(1),
        );
        budget.charge(work)?;
        merge(&mut attained, BoundingBox3::from_points([node.center()?])?)?;
        if let Some(hull) = hull
            && resolved(hull, attained.unwrap(), tolerance)
        {
            merge(&mut enclosure, hull)?;
            continue;
        }
        if node.depth == MAX_DEPTH {
            return Err(GeometryError::BoundingBoxDidNotConverge);
        }
        budget.charge(work)?;
        let axis = node.split_axis(hull, attained.unwrap(), tolerance);
        let (left, right) = node.split(axis);
        nodes.push(right);
        nodes.push(left);
    }
    enclosure.ok_or(GeometryError::EmptyPointSet)
}

pub(super) fn merge(
    bounds: &mut Option<BoundingBox3>,
    next: BoundingBox3,
) -> Result<(), GeometryError> {
    *bounds = Some(match *bounds {
        Some(b) => b.union(next)?,
        None => next,
    });
    Ok(())
}

pub(super) fn resolved(hull: BoundingBox3, attained: BoundingBox3, tolerance: Tolerance) -> bool {
    (0..3).all(|i| resolved_axis(hull, attained, tolerance, i))
}

fn resolved_axis(
    hull: BoundingBox3,
    attained: BoundingBox3,
    tolerance: Tolerance,
    axis: usize,
) -> bool {
    let min = hull.min().to_array()[axis];
    let max = hull.max().to_array()[axis];
    let scale = min.abs().max(max.abs());
    let epsilon = tolerance
        .absolute()
        .max(tolerance.relative() * scale)
        .max(16. * f64::EPSILON * scale);
    attained.min().to_array()[axis] - min <= epsilon
        && max - attained.max().to_array()[axis] <= epsilon
}
