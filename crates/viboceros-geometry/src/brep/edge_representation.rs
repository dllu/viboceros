//! Preserve native arc provenance without simplifying circular NURBS edges.
use super::*;

impl BrepEdge {
    /// Recover the marked analytic form at floating-point roundoff tolerance.
    /// Unmarked circular NURBS keep their original controls and parameters.
    pub fn analytic_arc(&self) -> Result<Option<crate::CircularArc3>, GeometryError> {
        if !self.arc_representation {
            return Ok(None);
        }
        let origin = self.curve.control_points()[0].point().to_array();
        let scale = self
            .curve
            .control_points()
            .iter()
            .flat_map(|p| {
                p.point()
                    .to_array()
                    .into_iter()
                    .zip(origin)
                    .map(|(x, o)| (x - o).abs())
            })
            .fold(Real::MIN_POSITIVE, Real::max);
        let coordinates = self
            .curve
            .control_points()
            .iter()
            .flat_map(|p| p.point().to_array())
            .map(Real::abs)
            .fold(Real::MIN_POSITIVE, Real::max);
        let tolerance = Tolerance::try_new(
            (coordinates / scale).max(1.) * (256. * Real::EPSILON),
            Tolerance::DEFAULT.relative(),
            Tolerance::DEFAULT.angular(),
        )?;
        // IsArc's circle construction becomes ill-conditioned at model scale.
        // Only this private predicate copy is translated and scaled.
        let points = self
            .curve
            .control_points()
            .iter()
            .map(|p| {
                let local = std::array::from_fn::<_, 3, _>(|i| {
                    (p.point().to_array()[i] - origin[i]) / scale
                });
                WeightedPoint3::try_new(Point3::try_from(local)?, p.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let local =
            NurbsCurve::try_new_rational(self.curve.degree(), points, self.curve.knots().to_vec())?;
        Ok(match local.analytic_form(tolerance)? {
            Some(crate::CurveSegment3::Arc(arc)) => {
                let center = std::array::from_fn::<_, 3, _>(|i| {
                    arc.center().to_array()[i] * scale + origin[i]
                });
                let circle = crate::Circle3::try_from_frame(
                    Point3::try_from(center)?,
                    arc.radius() * scale,
                    arc.x_axis(),
                    arc.normal()?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                Some(
                    crate::CircularArc3::try_from_circle_angles(circle, arc.angle_domain())?
                        .try_reparameterized(self.curve.domain())?,
                )
            }
            _ => None,
        })
    }

    pub(crate) fn with_arc_representation(mut self, arc: bool) -> Result<Self, GeometryError> {
        self.arc_representation = arc;
        if arc && self.analytic_arc()?.is_none() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "analytic arc edge is not circular",
            });
        }
        Ok(self)
    }
}
