//! Degree-one rational spans have straight geometry and projective speed.
use crate::{GeometryError, NurbsCurve, Real, WeightedPoint3};

pub(crate) struct RationalLineSpan {
    pub(crate) domain: [Real; 2],
    pub(crate) controls: [WeightedPoint3; 2],
    pub(crate) length: Real,
}

impl NurbsCurve {
    pub(crate) fn rational_line_spans(
        &self,
    ) -> Result<Option<Vec<RationalLineSpan>>, GeometryError> {
        if self.degree() != 1 {
            return Ok(None);
        }
        self.try_bezier_spans()?
            .into_iter()
            .map(|span| {
                let controls = [span.control_points()[0], span.control_points()[1]];
                if controls[0].weight().is_sign_positive()
                    != controls[1].weight().is_sign_positive()
                {
                    return Err(GeometryError::ZeroWeightAtParameter);
                }
                Ok(RationalLineSpan {
                    domain: [*span.domain().start(), *span.domain().end()],
                    length: controls[0].point().distance_to(controls[1].point())?,
                    controls,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}
