//! Linear-time decomposition at structurally independent knot groups.

use super::{GeometryError, NurbsCurve, Real};

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Interior knots of multiplicity `degree + 1`. These separate independent
    /// control blocks, which may or may not meet geometrically.
    pub fn full_order_knots(&self) -> impl Iterator<Item = Real> + '_ {
        self.full_order_knot_indices()
            .map(|index| self.knots[index])
    }

    /// Splits every full-order interior knot, in increasing source-domain
    /// order, including on closed curves. There is no cyclic seam relocation,
    /// fitting, weight matching, or endpoint averaging. The resulting pieces
    /// are clamped; a source without full-order knots is returned unchanged.
    pub fn try_split_at_full_order_knots(&self) -> Result<Vec<Self>, GeometryError> {
        let mut indices = self.full_order_knot_indices().peekable();
        if indices.peek().is_none() {
            return Ok(vec![self.clone()]);
        }
        let mut pieces = Vec::new();
        let mut first = 0;
        for end in indices.chain(std::iter::once(self.control_points.len())) {
            let piece = Self::try_new_rational(
                self.degree,
                self.control_points[first..end].to_vec(),
                self.knots[first..end + self.degree + 1].to_vec(),
            )?;
            pieces.push(piece.clamped_to_active_domain()?);
            first = end;
        }
        Ok(pieces)
    }

    fn full_order_knot_indices(&self) -> impl Iterator<Item = usize> + '_ {
        let domain = self.domain();
        self.knots
            .chunk_by(|a, b| a == b)
            .scan(0, move |index, group| {
                let first = *index;
                *index += group.len();
                Some(
                    (group.len() == self.degree + 1
                        && group[0] > *domain.start()
                        && group[0] < *domain.end())
                    .then_some(first),
                )
            })
            .flatten()
    }
}
