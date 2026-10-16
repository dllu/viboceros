//! Join sampled graph outlines without bridging kinks, curvature jumps or gaps.
use super::*;
use crate::{Polyline3, Tolerance};

struct Piece {
    first: CurvatureHair,
    last: CurvatureHair,
    points: Vec<Point3>,
}
fn joins(
    a: CurvatureHair,
    b: CurvatureHair,
    factor: Real,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    Ok(a.point.is_near(b.point, tolerance)
        && a.tangent.as_vector().angle_to(b.tangent.as_vector())? <= tolerance.angular()
        && a.tip(factor)?.is_near(b.tip(factor)?, tolerance))
}
impl CurvatureComb {
    /// Convert the visible tip strips to model polylines. Weld only neighboring
    /// samples of the same source station whose positions, tangents and graph
    /// tips agree within the supplied model tolerances. A closed source may
    /// additionally join its first and last pieces across the original seam.
    /// Flat strips and outlines with fewer than two distinct points emit none.
    pub fn envelope_polylines(
        &self,
        factor: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        if !factor.is_finite() || factor < 0. {
            return Err(GeometryError::InvalidCurvatureEnvelope);
        }
        let mut pieces: Vec<Piece> = Vec::new();
        let mut adjacent = false;
        for range in &self.strips {
            let hairs = self
                .hairs
                .get(range.clone())
                .filter(|s| !s.is_empty())
                .ok_or(GeometryError::InvalidCurvatureEnvelope)?;
            if hairs.iter().all(|h| h.curvature.to_array() == [0.; 3]) {
                adjacent = false;
                continue;
            }
            let first = hairs[0];
            let last = *hairs.last().unwrap();
            let points = hairs
                .iter()
                .map(|h| h.tip(factor))
                .collect::<Result<Vec<_>, _>>()?;
            if adjacent
                && let Some(previous) = pieces.last_mut()
                && previous.last.parameter == first.parameter
                && joins(previous.last, first, factor, tolerance)?
            {
                previous.points.extend(points.into_iter().skip(1));
                previous.last = last;
            } else {
                pieces.push(Piece {
                    first,
                    last,
                    points,
                });
            }
            adjacent = true;
        }
        // The seam is an adjacency only when both natural ends survived.
        if self.closed && pieces.len() > 1 {
            let first = &pieces[0];
            let last = pieces.last().unwrap();
            if self.domain[0] == first.first.parameter
                && self.domain[1] == last.last.parameter
                && joins(last.last, first.first, factor, tolerance)?
            {
                let first = pieces.remove(0);
                let last = pieces.last_mut().unwrap();
                last.points.extend(first.points.into_iter().skip(1));
                last.last = first.last;
            }
        }
        let mut outputs = Vec::new();
        for mut piece in pieces {
            piece.points.dedup();
            if piece.points.len() < 2 {
                continue;
            }
            if self.closed
                && self.domain[0] == piece.first.parameter
                && self.domain[1] == piece.last.parameter
                && joins(piece.last, piece.first, factor, tolerance)?
            {
                let first = piece.points[0];
                *piece.points.last_mut().unwrap() = first;
                piece.points.dedup();
            }
            if piece.points.len() >= 2 {
                outputs.push(Polyline3::try_new(
                    piece.points,
                    Tolerance::NUMERICAL_VALIDATION,
                )?);
            }
        }
        Ok(outputs)
    }
}
