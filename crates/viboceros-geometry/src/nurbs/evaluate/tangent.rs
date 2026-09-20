//! Limiting tangents, with exact recovery after range loss or float failure.
use super::*;

impl NurbsCurve {
    /// The oriented limiting tangent, including stationary points with a
    /// nonzero higher derivative. A locally constant span has no tangent.
    pub fn tangent_at_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<UnitVector3, GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
        let domain = self.domain();
        let incoming = parameter == *domain.end()
            || (side == ParameterSide::Left && parameter > *domain.start());
        self.with_evaluation_controls(
            span,
            |origin, active| self.floating_tangent(span, parameter, origin, active),
            || self.exact_tangent(span, parameter, incoming),
        )
    }

    fn floating_tangent(
        &self,
        span: usize,
        parameter: Real,
        origin: Point3,
        active: Vec<[Real; 4]>,
    ) -> Result<UnitVector3, GeometryError> {
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
        let point = project_homogeneous(homogeneous)?;
        self.restore_evaluated_point(span, parameter, point, origin)?;
        let point = point.to_array();
        let derivative = de_boor(
            &self.knots[1..self.knots.len() - 1],
            self.degree - 1,
            span - 1,
            parameter,
            self.derivative_controls(span, 1, &active)?,
        )?;
        let first = Vector3::try_from(std::array::from_fn(|i| {
            (-point[i]).mul_add(derivative[3], derivative[i]) / homogeneous[3]
        }))?;
        if first.to_array() != [0.; 3] {
            return first.normalized_nonzero();
        }
        // A zero rounded derivative is not an exact stationarity predicate.
        // The dispatcher resolves higher-order limits with exact arithmetic.
        Err(GeometryError::Degenerate {
            context: "unresolved NURBS tangent",
        })
    }
}
