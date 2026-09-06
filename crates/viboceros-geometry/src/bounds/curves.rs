//! Tight native-curve boxes from analytic extrema or adaptive rational hulls.
use crate::{BoundingBox3, CurveRef, GeometryError, NurbsCurve, Tolerance};

const MAX_NODES: usize = 131_072;
const MAX_DEPTH: u8 = 64;

impl CurveRef<'_> {
    /// Bound the complete curve, including independent limits at full-order
    /// knots. NURBS hulls are refined until each face is within the caller's
    /// absolute/relative tolerance of an attained curve coordinate. Floating
    /// point subdivision adds rounding error; this is not interval arithmetic.
    /// Unresolved poles or exhausted subdivision budgets return an error.
    pub fn tight_bounds(self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        match self {
            Self::Line(c) => BoundingBox3::from_points([c.start(), c.end()]),
            Self::Circle(c) => Ok(c.bounds()),
            Self::Arc(c) => Ok(c.bounds()),
            Self::Ellipse(c) => Ok(c.bounds()),
            Self::Polyline(c) => Ok(c.bounds()),
            Self::NurbsCurve(c) => c.tight_bounds(tolerance),
            Self::PolyCurve(c) => {
                let (first, rest) = c
                    .segments()
                    .split_first()
                    .ok_or(GeometryError::EmptyPointSet)?;
                rest.iter().try_fold(
                    first.as_ref().tight_bounds(tolerance)?,
                    |bounds, segment| bounds.union(segment.as_ref().tight_bounds(tolerance)?),
                )
            }
        }
    }
}

impl NurbsCurve {
    /// Tight, tolerance-controlled box without treating control points as
    /// extrema. Mixed-sign rational spans must acquire a same-sign weight hull
    /// under subdivision before any Euclidean hull may be accepted.
    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        if self.spans().take(MAX_NODES + 1).count() > MAX_NODES {
            return Err(GeometryError::CurveBoundsDidNotConverge);
        }
        let mut remainder = self.clamped_to_active_domain()?;
        let mixed = has_mixed_weights(self);
        if mixed && !well_conditioned_controls(&remainder, None) {
            return Err(GeometryError::CurveBoundsDidNotConverge);
        }
        let cuts = remainder
            .spans()
            .skip(1)
            .map(|(a, _)| a)
            .collect::<Vec<_>>();
        let mut nodes = Vec::with_capacity(cuts.len() + 1);
        // Do not use cyclic command splitting: both limits of every original
        // span, including a discontinuous closed seam, contribute to the box.
        for cut in cuts {
            let (left, right) = remainder.try_split(cut)?;
            if mixed && !well_conditioned_controls(&left, Some(&right)) {
                return Err(GeometryError::CurveBoundsDidNotConverge);
            }
            nodes.push((left, 0u8));
            remainder = right;
        }
        nodes.push((remainder, 0));
        if nodes.len() > MAX_NODES {
            return Err(GeometryError::CurveBoundsDidNotConverge);
        }
        let mut attained = None;
        for (curve, _) in &nodes {
            let controls = curve.control_points();
            let endpoints =
                BoundingBox3::from_points([controls[0].point(), controls.last().unwrap().point()])?;
            merge(&mut attained, endpoints)?;
        }
        let mut enclosure = None;
        let mut visited = 0;
        while let Some((curve, depth)) = nodes.pop() {
            visited += 1;
            if visited > MAX_NODES {
                return Err(GeometryError::CurveBoundsDidNotConverge);
            }
            let domain = curve.domain();
            let (a, b) = (*domain.start(), *domain.end());
            let middle = a * 0.5 + b * 0.5;
            let sample = curve.evaluate(middle)?;
            merge(&mut attained, BoundingBox3::from_points([sample])?)?;
            let controls = curve.control_points();
            let sign = controls[0].weight().is_sign_positive();
            let has_hull = controls
                .iter()
                .all(|c| c.weight().is_sign_positive() == sign);
            let hull = curve.control_point_bounds();
            if has_hull && resolved(hull, attained.unwrap(), tolerance) {
                merge(&mut enclosure, hull)?;
                continue;
            }
            if depth == MAX_DEPTH || middle <= a || middle >= b {
                return Err(GeometryError::CurveBoundsDidNotConverge);
            }
            let (left, right) = subdivide(&curve, a, b, middle)?;
            nodes.push((right, depth + 1));
            nodes.push((left, depth + 1));
        }
        enclosure.ok_or(GeometryError::EmptyPointSet)
    }
}

fn subdivide(
    curve: &NurbsCurve,
    a: f64,
    b: f64,
    middle: f64,
) -> Result<(NurbsCurve, NurbsCurve), GeometryError> {
    // A regular mixed-weight curve can have an intermediate homogeneous
    // control at infinity at this particular cut. Try bounded alternative
    // cuts before rejecting an unrepresentable subdivision; never accept its
    // Euclidean control hull merely because splitting failed.
    let mixed = has_mixed_weights(curve);
    let mut result = Err(GeometryError::CurveBoundsDidNotConverge);
    for parameter in [middle, a * (2. / 3.) + b / 3., a / 3. + b * (2. / 3.)] {
        if a < parameter && parameter < b {
            result = curve.try_split(parameter);
            if let Ok((left, right)) = &result {
                // Near-zero controls can be finite yet numerically projective:
                // subsequent Euclidean knot insertion loses their geometry.
                if !mixed || well_conditioned_controls(left, Some(right)) {
                    return result;
                }
                result = Err(GeometryError::CurveBoundsDidNotConverge);
            }
        }
    }
    result
}

fn has_mixed_weights(curve: &NurbsCurve) -> bool {
    let controls = curve.control_points();
    let sign = controls[0].weight().is_sign_positive();
    controls
        .iter()
        .any(|c| c.weight().is_sign_positive() != sign)
}

fn well_conditioned_controls(left: &NurbsCurve, right: Option<&NurbsCurve>) -> bool {
    let weights = || {
        left.control_points()
            .iter()
            .chain(right.into_iter().flat_map(|c| c.control_points()))
            .map(|c| c.weight().abs())
    };
    let scale = weights().fold(0., f64::max);
    weights().all(|w| w / scale > 64. * f64::EPSILON)
}

fn merge(bounds: &mut Option<BoundingBox3>, next: BoundingBox3) -> Result<(), GeometryError> {
    *bounds = Some(match *bounds {
        Some(previous) => previous.union(next)?,
        None => next,
    });
    Ok(())
}

fn resolved(hull: BoundingBox3, attained: BoundingBox3, tolerance: Tolerance) -> bool {
    let h_min = hull.min().to_array();
    let h_max = hull.max().to_array();
    let a_min = attained.min().to_array();
    let a_max = attained.max().to_array();
    (0..3).all(|axis| {
        let scale = h_min[axis].abs().max(h_max[axis].abs());
        let epsilon = tolerance
            .absolute()
            .max(tolerance.relative() * scale)
            .max(16. * f64::EPSILON * scale);
        a_min[axis] - h_min[axis] <= epsilon && h_max[axis] - a_max[axis] <= epsilon
    })
}

#[cfg(test)]
mod tests;
