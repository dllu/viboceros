//! Lossless knot-origin removal for geometric calculations in parameter space.
use super::*;
use crate::parameter::{exact_difference, lossless_parameter_origin};
use std::borrow::Cow;

pub(crate) struct CurveParameterFrame<'a> {
    pub(crate) curve: Cow<'a, NurbsCurve>,
    pub(crate) origin: Real,
}

impl NurbsCurve {
    /// Changes only the temporary parameter origin, and only when every knot
    /// (including exterior knots) translates exactly. Stored geometry is untouched.
    pub(crate) fn local_parameter_frame(&self) -> Result<CurveParameterFrame<'_>, GeometryError> {
        let origin = lossless_parameter_origin(self.domain(), self.knots.iter().copied());
        self.parameter_frame(origin)
    }
    /// Lossless start-anchored chart for finite model-space mesh samples.
    pub(crate) fn sampling_parameter_frame(
        &self,
    ) -> Result<CurveParameterFrame<'_>, GeometryError> {
        let start = *self.domain().start();
        if start == 0.
            || self
                .knots
                .iter()
                .all(|&k| exact_difference(k, start).is_some())
        {
            self.parameter_frame(start)
        } else {
            self.local_parameter_frame()
        }
    }
    fn parameter_frame(&self, origin: Real) -> Result<CurveParameterFrame<'_>, GeometryError> {
        let curve = if origin == 0. {
            Cow::Borrowed(self)
        } else {
            Cow::Owned(Self::try_new_rational(
                self.degree,
                self.control_points.clone(),
                self.knots.iter().map(|k| k - origin).collect(),
            )?)
        };
        Ok(CurveParameterFrame { curve, origin })
    }
}
