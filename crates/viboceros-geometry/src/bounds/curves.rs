//! Tight native-curve boxes from analytic extrema or adaptive rational hulls.
use crate::{BoundingBox3, CurveRef, GeometryError, NurbsCurve, Tolerance};

use super::bezier::{self, Budget, Net};

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
    /// Tight, tolerance-controlled box, retaining homogeneous intermediate
    /// controls until a same-sign rational hull can be projected safely.
    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        let mut budget = Budget::default();
        let mut nodes = Vec::new();
        let degree = self.degree();
        for span in degree..self.control_points().len() {
            if self.knots()[span] == self.knots()[span + 1] {
                continue;
            }
            budget.initial(degree + 1)?;
            let mut net = Net::new([degree, 0], &self.control_points()[span - degree..=span])?;
            net.extract_axis(0, self.knots(), span, &mut budget)?;
            nodes.push(net);
        }
        bezier::bounds(nodes, &mut budget, tolerance)
    }
}

#[cfg(test)]
mod tests;
