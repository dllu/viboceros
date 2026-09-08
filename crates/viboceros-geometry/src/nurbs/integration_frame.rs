//! Checked dimensionless frames shared by length, sampling, and curve area.

use crate::{GeometryError, NurbsCurve};
use std::borrow::Cow;

impl NurbsCurve {
    /// Borrows unit-domain curves, otherwise maps their full knot vector to
    /// a dimensionless frame without changing controls or weights. This
    /// avoids parameter-scale derivative overflow and sampling outside narrow
    /// translated domains. It does not fix arbitrarily ill-conditioned
    /// relative interior span widths.
    pub(crate) fn for_integration(&self) -> Result<Cow<'_, Self>, GeometryError> {
        if self.domain() == (0.0..=1.0) {
            return Ok(Cow::Borrowed(self));
        }
        let normalized = self.try_reparameterized(0.0..=1.0)?;
        // Never silently remove an interval whose relative width cannot be
        // represented in the normalized frame, including exterior knots.
        if self
            .knots()
            .windows(2)
            .zip(normalized.knots().windows(2))
            .any(|(before, after)| before[0] < before[1] && after[0] >= after[1])
        {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        Ok(Cow::Owned(normalized))
    }
}
